//! Selectively sampled agent trajectories, written as Parquet.
//!
//! The run directory's trajectory artifact is the per-agent motion record a
//! consumer plots or aggregates: one row per sampled agent frame, holding the
//! agent's mode, body kind, world position, heading, longitudinal speed, and
//! ordered body-segment poses at one completed tick.
//!
//! Sampling is what bounds the artifact. The declared
//! [`TrajectorySampling`] policy fixes the stride between sampled ticks and the
//! most rows the artifact may hold, the default policy keeps a bounded subset,
//! and full per-tick trajectories are opt-in. Rows are written in canonical
//! order — ascending tick, then ascending agent — and the writer reads no
//! clock, so the same rows produce byte-identical bytes.
//!
//! The Parquet dependency lives here, in the CLI/output layer: `tangle-model`
//! and `tangle-sim` never depend on it.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::builder::{Float64Builder, ListBuilder, StructBuilder};
use arrow_array::{
    Array, ArrayRef, Float64Array, ListArray, RecordBatch, StringArray, StructArray, UInt32Array,
    UInt64Array,
};
use arrow_schema::{ArrowError, DataType, Field, Fields, Schema, SchemaRef};
use glam::DVec2;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::errors::ParquetError;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use tangle_model::{BodyKind, MovementDirection, PermissionEffect};
use tangle_sim::{BodySegmentSample, ManeuverState, Simulation, SnapshotDetail};

use crate::run_dir::TrajectorySampling;
use crate::trace::sha256_hex;

/// File name of the sampled-trajectory artifact inside a run directory.
pub const TRAJECTORY_FILE: &str = "trajectories.parquet";

/// File format [`TRAJECTORY_FILE`] uses.
pub const TRAJECTORY_FORMAT: &str = "parquet";

/// Version of the sampled-trajectory artifact's column shape.
///
/// Version 3 appends the optional route-relative tactical columns
/// `route_s_m`, `route_d_m`, `target_offset_m`, `maneuver_state`,
/// `predicted_min_clearance_m`, `perceived_rule`, and `opposing_direction`, so a
/// run directory's trajectories describe a steering body's route coordinates,
/// maneuver state, and wrong-way rule state where it has them and stay absent
/// (`null`) where it does not. They are additive: no existing column changes
/// meaning or position, and a run that authors no Increment 2 policy writes
/// `null` in each. The whole additive column union lands under this one version,
/// so the rule-state columns add no second bump. Version 2 added `body_kind` and
/// the ordered `segments` pose list; version 1 was the seven columns `tick`,
/// `agent`, `mode`, `x_m`, `y_m`, `heading_rad`, and `speed_mps`.
pub const TRAJECTORY_FORMAT_VERSION: u32 = 3;

/// The trajectory columns, in schema and declaration order.
///
/// One row is one sampled agent frame. The order is part of the artifact
/// contract: a consumer that reads by position and one that reads by name see
/// the same record. Units are spelled into the names, as the event records do.
/// `segments` is an ordered list of `{x_m, y_m, heading_rad}` poses, empty for a
/// Phase 1 single-envelope body and never null. The final seven columns are the
/// optional route-relative tactical state: they are `null` for a pedestrian and
/// a legacy version-1 path-following agent, which carry no route coordinates.
/// `perceived_rule` and `opposing_direction` are the wrong-way rule state, so
/// they are null in every row that is not on an opposing traversal: the state is
/// sparse by nature, and a rule-state transition is an event record, never an
/// extra row here. The third tuple field is the column's nullability.
fn trajectory_columns() -> Vec<(&'static str, DataType, bool)> {
    vec![
        ("tick", DataType::UInt64, false),
        ("agent", DataType::UInt32, false),
        ("mode", DataType::Utf8, false),
        ("body_kind", DataType::Utf8, false),
        ("x_m", DataType::Float64, false),
        ("y_m", DataType::Float64, false),
        ("heading_rad", DataType::Float64, false),
        ("speed_mps", DataType::Float64, false),
        ("segments", segments_data_type(), false),
        ("route_s_m", DataType::Float64, true),
        ("route_d_m", DataType::Float64, true),
        ("target_offset_m", DataType::Float64, true),
        ("maneuver_state", DataType::Utf8, true),
        ("predicted_min_clearance_m", DataType::Float64, true),
        ("perceived_rule", DataType::Utf8, true),
        ("opposing_direction", DataType::Utf8, true),
    ]
}

/// The Arrow type of one row's ordered body-segment poses.
fn segments_data_type() -> DataType {
    DataType::List(Arc::new(segment_item_field()))
}

/// The item field of the `segments` list: one body-segment pose.
fn segment_item_field() -> Field {
    Field::new("item", DataType::Struct(segment_fields()), false)
}

/// The fields of one body-segment pose, in declaration order.
fn segment_fields() -> Fields {
    Fields::from(vec![
        Field::new("x_m", DataType::Float64, false),
        Field::new("y_m", DataType::Float64, false),
        Field::new("heading_rad", DataType::Float64, false),
    ])
}

/// One sampled trajectory row: one agent observed at one completed step.
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectorySample {
    /// Completed fixed step the sample was taken at.
    pub tick: u64,
    /// Stable agent identifier, the same value the event records carry.
    pub agent: u32,
    /// Agent mode label, matching the `spawned` event's `mode` field.
    pub mode: String,
    /// Envelope kind of the body, the same kind the snapshot and scene carry.
    pub body_kind: BodyKind,
    /// World position east of the origin in metres.
    pub x_m: f64,
    /// World position north of the origin in metres.
    pub y_m: f64,
    /// World heading in radians.
    pub heading_rad: f64,
    /// Longitudinal speed in metres per second.
    pub speed_mps: f64,
    /// Ordered body segments front to back, each with its own world pose. Empty
    /// for a Phase 1 single-envelope body.
    pub segments: Vec<BodySegmentSample>,
    /// Arc length along the compiled facility reference in metres, present for a
    /// steering body that carries route state.
    pub route_s_m: Option<f64>,
    /// Signed lateral offset in metres, positive to the left of travel, present
    /// for a steering body that carries route state.
    pub route_d_m: Option<f64>,
    /// The active maneuver's target signed offset, once one is fixed.
    pub target_offset_m: Option<f64>,
    /// The active maneuver lifecycle state, present with the route coordinates.
    pub maneuver_state: Option<ManeuverState>,
    /// Predicted minimum clearance over the maneuver horizon, once predicted.
    pub predicted_min_clearance_m: Option<f64>,
    /// The applicable `nominal_direction` permission statement the agent
    /// perceived, present exactly with `opposing_direction` and absent when no
    /// statement binds the pair.
    pub perceived_rule: Option<PermissionEffect>,
    /// The direction the agent travels on its opposing traversal of the object
    /// whose rule direction it is against, present only while it is on such a
    /// traversal: a nominal traversal, an `either` object, and an agent without
    /// route state all stay absent.
    pub opposing_direction: Option<MovementDirection>,
}

/// The Parquet sampled-trajectory artifact a manifest describes.
///
/// The descriptor is how a consumer traces the trajectory file to its run: it
/// names the file inside the run directory, its row count, and the SHA-256 of
/// its exact bytes, and it lives in the manifest that carries the scenario,
/// seed, and sampling policy that produced them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectoryArtifact {
    /// File name inside the run directory.
    pub path: String,
    /// File format.
    pub format: String,
    /// Column-shape version of the artifact, [`TRAJECTORY_FORMAT_VERSION`].
    pub format_version: u32,
    /// Rows the artifact holds, one per sampled agent frame.
    pub rows: u64,
    /// SHA-256 of the file's exact bytes.
    pub sha256: String,
}

/// Failure to write or read the sampled-trajectory artifact.
#[derive(Debug, thiserror::Error)]
pub enum TrajectoryError {
    /// The artifact could not be written or read.
    #[error("cannot {action} trajectory artifact '{path}': {source}")]
    Io {
        /// The file the operation targeted.
        path: PathBuf,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The file is not readable Parquet.
    #[error("trajectory artifact '{path}' is not readable Parquet: {source}")]
    Parquet {
        /// The file that failed to decode.
        path: PathBuf,
        /// The underlying Parquet failure.
        #[source]
        source: ParquetError,
    },
    /// The file's Arrow record batches could not be decoded.
    #[error("trajectory artifact '{path}' does not decode as Arrow record batches: {source}")]
    Arrow {
        /// The file that failed to decode.
        path: PathBuf,
        /// The underlying Arrow failure.
        #[source]
        source: ArrowError,
    },
    /// The file does not hold the declared trajectory columns.
    #[error("trajectory artifact '{path}' does not hold the trajectory columns: {detail}")]
    Schema {
        /// The file that failed the schema check.
        path: PathBuf,
        /// The first mismatch the check found.
        detail: String,
    },
}

/// Collects the trajectory rows one declared policy keeps.
///
/// The recorder samples the step the simulation last completed and appends its
/// live agents in stable spawn order, so the rows come out in canonical order —
/// ascending tick, then ascending agent — and stops as soon as the policy's
/// `max_samples` cap is reached. Sampling during the run is what bounds the
/// artifact: both the collected rows and the written file are bounded by the
/// declared policy before any bytes are produced.
pub struct TrajectoryRecorder {
    sampling: TrajectorySampling,
    samples: Vec<TrajectorySample>,
}

impl TrajectoryRecorder {
    /// A recorder that keeps exactly the rows `sampling` retains.
    pub fn new(sampling: TrajectorySampling) -> Self {
        Self {
            sampling,
            samples: Vec::new(),
        }
    }

    /// Observe the agents the simulation's last completed step left alive.
    ///
    /// The tick is the one the step completed, the same tick the canonical
    /// trace attributes that step's events to, so a trajectory row and an event
    /// record with the same tick describe the same instant.
    pub fn observe(&mut self, sim: &Simulation) {
        let tick = sim.time().tick();
        if !self.sampling.samples_tick(tick) {
            return;
        }
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            if !self.sampling.below_cap(self.samples.len() as u64) {
                return;
            }
            let motion = sample
                .motion
                .as_ref()
                .expect("a full snapshot detail carries motion");
            let route = motion.route_state;
            self.samples.push(TrajectorySample {
                tick,
                agent: sample.id.get(),
                mode: motion.mode.label().to_owned(),
                body_kind: motion.body_kind,
                x_m: sample.position.x,
                y_m: sample.position.y,
                heading_rad: sample.heading_rad,
                speed_mps: motion.speed_mps,
                segments: motion.segments.clone(),
                route_s_m: route.map(|route| route.s_m),
                route_d_m: route.map(|route| route.d_m),
                target_offset_m: route.and_then(|route| route.target_offset_m),
                maneuver_state: route.map(|route| route.maneuver_state),
                predicted_min_clearance_m: route.and_then(|route| route.predicted_min_clearance_m),
                perceived_rule: route.and_then(|route| route.perceived_rule),
                opposing_direction: route.and_then(|route| route.opposing_direction),
            });
        }
    }

    /// The rows kept so far, in canonical order.
    pub fn samples(&self) -> &[TrajectorySample] {
        &self.samples
    }

    /// Finish the capture, returning the rows the policy kept.
    pub fn finish(self) -> Vec<TrajectorySample> {
        self.samples
    }
}

/// Write the sampled trajectories into a run directory and describe the file.
///
/// The file is a pure function of the rows: the schema is fixed, the pages use
/// a fixed compression, and nothing reads the clock or the environment, so the
/// same rows reproduce byte-identical bytes. The returned descriptor carries
/// the row count and the file's SHA-256, which is what lets a manifest link the
/// artifact back to its run.
pub fn write_trajectories(
    directory: &Path,
    samples: &[TrajectorySample],
) -> Result<TrajectoryArtifact, TrajectoryError> {
    let path = directory.join(TRAJECTORY_FILE);
    let schema = trajectory_schema();
    let mut bytes = Vec::new();
    {
        let mut writer =
            ArrowWriter::try_new(&mut bytes, schema.clone(), Some(writer_properties()))
                .map_err(|source| parquet_error(&path, source))?;
        if !samples.is_empty() {
            let batch = record_batch(&schema, samples);
            writer
                .write(&batch)
                .map_err(|source| parquet_error(&path, source))?;
        }
        writer
            .close()
            .map_err(|source| parquet_error(&path, source))?;
    }
    fs::write(&path, &bytes).map_err(|source| TrajectoryError::Io {
        path: path.clone(),
        action: "write",
        source,
    })?;
    Ok(TrajectoryArtifact {
        path: TRAJECTORY_FILE.to_owned(),
        format: TRAJECTORY_FORMAT.to_owned(),
        format_version: TRAJECTORY_FORMAT_VERSION,
        rows: samples.len() as u64,
        sha256: sha256_hex(&bytes),
    })
}

/// Read a run directory's sampled trajectories back, in canonical order.
///
/// The artifact is self-describing, so a consumer reads rows without the run
/// that produced them; the manifest's descriptor is what ties the rows back to
/// that run.
pub fn read_trajectories(directory: &Path) -> Result<Vec<TrajectorySample>, TrajectoryError> {
    let path = directory.join(TRAJECTORY_FILE);
    let file = File::open(&path).map_err(|source| TrajectoryError::Io {
        path: path.clone(),
        action: "read",
        source,
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|source| parquet_error(&path, source))?
        .build()
        .map_err(|source| parquet_error(&path, source))?;

    let mut samples = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|source| TrajectoryError::Arrow {
            path: path.clone(),
            source,
        })?;
        let columns = TrajectoryColumns::new(&batch, &path)?;
        for row in 0..batch.num_rows() {
            samples.push(columns.sample(row));
        }
    }
    Ok(samples)
}

/// The artifact's Arrow schema, built from [`trajectory_columns`].
fn trajectory_schema() -> SchemaRef {
    Arc::new(Schema::new(
        trajectory_columns()
            .into_iter()
            .map(|(name, data_type, nullable)| Field::new(name, data_type, nullable))
            .collect::<Vec<_>>(),
    ))
}

/// One Arrow record batch holding every row, in canonical order.
fn record_batch(schema: &SchemaRef, samples: &[TrajectorySample]) -> RecordBatch {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(
            samples.iter().map(|sample| sample.tick).collect::<Vec<_>>(),
        )),
        Arc::new(UInt32Array::from(
            samples
                .iter()
                .map(|sample| sample.agent)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from_iter_values(
            samples.iter().map(|sample| sample.mode.as_str()),
        )),
        Arc::new(StringArray::from_iter_values(
            samples.iter().map(|sample| sample.body_kind.label()),
        )),
        Arc::new(Float64Array::from(
            samples.iter().map(|sample| sample.x_m).collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            samples.iter().map(|sample| sample.y_m).collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            samples
                .iter()
                .map(|sample| sample.heading_rad)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            samples
                .iter()
                .map(|sample| sample.speed_mps)
                .collect::<Vec<_>>(),
        )),
        Arc::new(segments_array(samples)),
        Arc::new(Float64Array::from(
            samples
                .iter()
                .map(|sample| sample.route_s_m)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            samples
                .iter()
                .map(|sample| sample.route_d_m)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float64Array::from(
            samples
                .iter()
                .map(|sample| sample.target_offset_m)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from_iter(
            samples
                .iter()
                .map(|sample| sample.maneuver_state.map(ManeuverState::label)),
        )),
        Arc::new(Float64Array::from(
            samples
                .iter()
                .map(|sample| sample.predicted_min_clearance_m)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from_iter(samples.iter().map(|sample| {
            sample.perceived_rule.map(PermissionEffect::label)
        }))),
        Arc::new(StringArray::from_iter(samples.iter().map(|sample| {
            sample.opposing_direction.map(MovementDirection::label)
        }))),
    ];
    RecordBatch::try_new(schema.clone(), columns)
        .expect("every column of the trajectory schema has the row count")
}

/// The ordered body-segment poses of every row, as one list column.
fn segments_array(samples: &[TrajectorySample]) -> ListArray {
    let mut builder = ListBuilder::new(StructBuilder::from_fields(
        segment_fields(),
        samples.iter().map(|sample| sample.segments.len()).sum(),
    ))
    .with_field(segment_item_field());
    for sample in samples {
        for segment in &sample.segments {
            let values = builder.values();
            values
                .field_builder::<Float64Builder>(0)
                .expect("the segment struct declares an x field")
                .append_value(segment.position.x);
            values
                .field_builder::<Float64Builder>(1)
                .expect("the segment struct declares a y field")
                .append_value(segment.position.y);
            values
                .field_builder::<Float64Builder>(2)
                .expect("the segment struct declares a heading field")
                .append_value(segment.heading_rad);
            values.append(true);
        }
        builder.append(true);
    }
    builder.finish()
}

/// Writing properties of the artifact.
///
/// Snappy pages keep the artifact small, and the writer version is fixed by the
/// pinned crate, so compression cannot make the bytes depend on the machine.
fn writer_properties() -> WriterProperties {
    WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build()
}

fn parquet_error(path: &Path, source: ParquetError) -> TrajectoryError {
    TrajectoryError::Parquet {
        path: path.to_path_buf(),
        source,
    }
}

/// The trajectory columns of one Arrow batch, after checking its schema.
struct TrajectoryColumns<'a> {
    ticks: &'a UInt64Array,
    agents: &'a UInt32Array,
    modes: &'a StringArray,
    body_kinds: Vec<BodyKind>,
    xs: &'a Float64Array,
    ys: &'a Float64Array,
    headings: &'a Float64Array,
    speeds: &'a Float64Array,
    segments: &'a ListArray,
    route_s: &'a Float64Array,
    route_d: &'a Float64Array,
    target_offset: &'a Float64Array,
    maneuver_states: Vec<Option<ManeuverState>>,
    predicted_min_clearances: &'a Float64Array,
    perceived_rules: Vec<Option<PermissionEffect>>,
    opposing_directions: Vec<Option<MovementDirection>>,
}

impl<'a> TrajectoryColumns<'a> {
    /// Check the batch's schema and bind its columns.
    fn new(batch: &'a RecordBatch, path: &Path) -> Result<Self, TrajectoryError> {
        let schema = batch.schema();
        let columns = trajectory_columns();
        for (index, (name, data_type, nullable)) in columns.iter().enumerate() {
            let Some(field) = schema.fields().get(index) else {
                return Err(schema_error(
                    path,
                    format!("column {index} ('{name}') is absent"),
                ));
            };
            if field.name() != name
                || field.data_type() != data_type
                || field.is_nullable() != *nullable
            {
                return Err(schema_error(
                    path,
                    format!(
                        "column {index} is '{}' of type {} (nullable {}), not '{name}' of type \
                         {data_type} (nullable {nullable})",
                        field.name(),
                        field.data_type(),
                        field.is_nullable(),
                    ),
                ));
            }
        }
        if schema.fields().len() != columns.len() {
            return Err(schema_error(
                path,
                format!(
                    "the artifact holds {} columns, not {}",
                    schema.fields().len(),
                    columns.len()
                ),
            ));
        }
        let body_kinds: Vec<BodyKind> = downcast::<StringArray>(batch, 3)
            .iter()
            .map(|label| {
                let label = label.expect("the body-kind column is never null");
                body_kind_from_label(label).ok_or_else(|| {
                    schema_error(
                        path,
                        format!("the body-kind column holds unknown kind '{label}'"),
                    )
                })
            })
            .collect::<Result<_, _>>()?;
        let maneuver_states = labels(batch, path, 12, "maneuver-state", ManeuverState::from_label)?;
        let perceived_rules = labels(
            batch,
            path,
            14,
            "perceived-rule",
            permission_effect_from_label,
        )?;
        let opposing_directions = labels(
            batch,
            path,
            15,
            "opposing-direction",
            movement_direction_from_label,
        )?;
        // The schema check above proves each column's type, so the downcasts
        // cannot fail.
        Ok(Self {
            ticks: downcast(batch, 0),
            agents: downcast(batch, 1),
            modes: downcast(batch, 2),
            body_kinds,
            xs: downcast(batch, 4),
            ys: downcast(batch, 5),
            headings: downcast(batch, 6),
            speeds: downcast(batch, 7),
            segments: downcast(batch, 8),
            route_s: downcast(batch, 9),
            route_d: downcast(batch, 10),
            target_offset: downcast(batch, 11),
            maneuver_states,
            predicted_min_clearances: downcast(batch, 13),
            perceived_rules,
            opposing_directions,
        })
    }

    /// The row at `row` as a trajectory sample.
    fn sample(&self, row: usize) -> TrajectorySample {
        TrajectorySample {
            tick: self.ticks.value(row),
            agent: self.agents.value(row),
            mode: self.modes.value(row).to_owned(),
            body_kind: self.body_kinds[row],
            x_m: self.xs.value(row),
            y_m: self.ys.value(row),
            heading_rad: self.headings.value(row),
            speed_mps: self.speeds.value(row),
            segments: segment_samples(self.segments.value(row)),
            route_s_m: optional_float(self.route_s, row),
            route_d_m: optional_float(self.route_d, row),
            target_offset_m: optional_float(self.target_offset, row),
            maneuver_state: self.maneuver_states[row],
            predicted_min_clearance_m: optional_float(self.predicted_min_clearances, row),
            perceived_rule: self.perceived_rules[row],
            opposing_direction: self.opposing_directions[row],
        }
    }
}

/// One nullable float column cell: `None` when the cell is null.
fn optional_float(column: &Float64Array, row: usize) -> Option<f64> {
    (!column.is_null(row)).then(|| column.value(row))
}

/// Reconstruct the ordered body-segment poses of one row from its list cell.
fn segment_samples(cell: ArrayRef) -> Vec<BodySegmentSample> {
    let structs = cell
        .as_any()
        .downcast_ref::<StructArray>()
        .expect("a checked segments cell holds structs");
    let xs = struct_column(structs, "x_m");
    let ys = struct_column(structs, "y_m");
    let headings = struct_column(structs, "heading_rad");
    (0..structs.len())
        .map(|index| BodySegmentSample {
            position: DVec2::new(xs.value(index), ys.value(index)),
            heading_rad: headings.value(index),
        })
        .collect()
}

/// One `Float64` field of a checked segment struct.
fn struct_column<'a>(structs: &'a StructArray, name: &str) -> &'a Float64Array {
    structs
        .column_by_name(name)
        .unwrap_or_else(|| panic!("the segment struct declares a {name} field"))
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap_or_else(|| panic!("the {name} field is a float column"))
}

/// Parse a body-kind label back, the inverse of [`BodyKind::label`].
fn body_kind_from_label(label: &str) -> Option<BodyKind> {
    [
        BodyKind::Circle,
        BodyKind::Box,
        BodyKind::Capsule,
        BodyKind::ArticulatedChain,
    ]
    .into_iter()
    .find(|kind| kind.label() == label)
}

/// Parse a permission-effect label back, the inverse of
/// [`PermissionEffect::label`].
fn permission_effect_from_label(label: &str) -> Option<PermissionEffect> {
    [
        PermissionEffect::Permit,
        PermissionEffect::Prohibit,
        PermissionEffect::Obligate,
    ]
    .into_iter()
    .find(|effect| effect.label() == label)
}

/// Parse a movement-direction label back, the inverse of
/// [`MovementDirection::label`].
fn movement_direction_from_label(label: &str) -> Option<MovementDirection> {
    [MovementDirection::Forward, MovementDirection::Reverse]
        .into_iter()
        .find(|direction| direction.label() == label)
}

/// One checked label column of an Arrow batch, decoded through `parse`.
///
/// A cell that holds no label is an absent value; an unknown label is a schema
/// error rather than a silently dropped state.
fn labels<T>(
    batch: &RecordBatch,
    path: &Path,
    index: usize,
    name: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Vec<Option<T>>, TrajectoryError> {
    downcast::<StringArray>(batch, index)
        .iter()
        .map(|label| {
            label
                .map(|label| {
                    parse(label).ok_or_else(|| {
                        schema_error(
                            path,
                            format!("the {name} column holds unknown label '{label}'"),
                        )
                    })
                })
                .transpose()
        })
        .collect()
}

/// One checked column of an Arrow batch.
fn downcast<T: 'static>(batch: &RecordBatch, index: usize) -> &T {
    batch
        .column(index)
        .as_any()
        .downcast_ref::<T>()
        .expect("the checked schema declares this column's type")
}

fn schema_error(path: &Path, detail: String) -> TrajectoryError {
    TrajectoryError::Schema {
        path: path.to_path_buf(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declared columns are the schema: names, types, order, and
    /// nullability, so a reader can trust either names or positions.
    #[test]
    fn the_declared_columns_are_the_written_schema() {
        let schema = trajectory_schema();
        let columns = trajectory_columns();
        assert_eq!(schema.fields().len(), columns.len());
        for (field, (name, data_type, nullable)) in schema.fields().iter().zip(columns) {
            assert_eq!(field.name(), name);
            assert_eq!(field.data_type(), &data_type);
            assert_eq!(field.is_nullable(), nullable, "column '{name}' nullability");
        }
        // The artifact declares its own column-shape version.
        assert_eq!(TRAJECTORY_FORMAT_VERSION, 3);
    }

    /// A recorded sample carries the kernel's own values, so the artifact
    /// cannot silently disagree with the run that produced it.
    #[test]
    fn a_recorded_sample_carries_the_frames_the_kernel_reports() {
        let policy = TrajectorySampling {
            retention: crate::run_dir::TrajectoryRetention::Full,
            stride_ticks: 1,
            max_samples: 1,
        };
        let source = tangle_model::parse_scenario_source(WALKING).expect("scenario parses");
        let scenario = tangle_model::CompiledScenario::compile(source).expect("scenario compiles");
        let mut sim = Simulation::new(scenario, tangle_sim::RunConfig::new(0)).expect("builds");
        let mut recorder = TrajectoryRecorder::new(policy);

        sim.step();
        recorder.observe(&sim);

        let frame = sim.snapshot(SnapshotDetail::Full);
        let agent = &frame.agents()[0];
        let motion = agent.motion.as_ref().expect("full detail carries motion");
        assert_eq!(
            recorder.samples(),
            [TrajectorySample {
                tick: 1,
                agent: agent.id.get(),
                mode: motion.mode.label().to_owned(),
                body_kind: motion.body_kind,
                x_m: agent.position.x,
                y_m: agent.position.y,
                heading_rad: agent.heading_rad,
                speed_mps: motion.speed_mps,
                segments: motion.segments.clone(),
                route_s_m: None,
                route_d_m: None,
                target_offset_m: None,
                maneuver_state: None,
                predicted_min_clearance_m: None,
                perceived_rule: None,
                opposing_direction: None,
            }]
        );
    }

    /// The artifact round-trips the optional route-relative tactical columns and
    /// the wrong-way rule state, including their absence, so a consumer reads
    /// the same route state the kernel reported and a legacy row stays absent
    /// rather than zero.
    #[test]
    fn the_artifact_round_trips_optional_route_state() {
        let present = TrajectorySample {
            tick: 1,
            agent: 0,
            mode: "vehicle".to_owned(),
            body_kind: tangle_model::BodyKind::Capsule,
            x_m: 1.0,
            y_m: 2.0,
            heading_rad: 0.5,
            speed_mps: 3.0,
            segments: Vec::new(),
            route_s_m: Some(12.5),
            route_d_m: Some(-0.25),
            target_offset_m: Some(1.75),
            maneuver_state: Some(ManeuverState::Returning),
            predicted_min_clearance_m: Some(0.6),
            perceived_rule: Some(PermissionEffect::Permit),
            opposing_direction: Some(MovementDirection::Reverse),
        };
        let absent = TrajectorySample {
            route_s_m: None,
            route_d_m: None,
            target_offset_m: None,
            maneuver_state: None,
            predicted_min_clearance_m: None,
            perceived_rule: None,
            opposing_direction: None,
            ..present.clone()
        };
        let rows = vec![present, absent];

        let directory = std::env::temp_dir().join(format!(
            "tangle-trajectory-route-state-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch directory is created");
        write_trajectories(&directory, &rows).expect("trajectories write");
        let read = read_trajectories(&directory).expect("trajectories read back");
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(read, rows);
    }

    /// The artifact round-trips the body kind and the ordered segment poses, so
    /// a consumer reads the same envelope the kernel reported.
    #[test]
    fn the_artifact_round_trips_body_kind_and_segments() {
        use tangle_model::BodyKind;

        let sample =
            |agent: u32, body_kind: BodyKind, segments: Vec<BodySegmentSample>| TrajectorySample {
                tick: 1,
                agent,
                mode: "vehicle".to_owned(),
                body_kind,
                x_m: 1.0,
                y_m: 2.0,
                heading_rad: 0.5,
                speed_mps: 3.0,
                segments,
                route_s_m: None,
                route_d_m: None,
                target_offset_m: None,
                maneuver_state: None,
                predicted_min_clearance_m: None,
                perceived_rule: None,
                opposing_direction: None,
            };
        let rows = vec![
            sample(0, BodyKind::Box, Vec::new()),
            sample(
                1,
                BodyKind::ArticulatedChain,
                vec![
                    BodySegmentSample {
                        position: DVec2::new(4.0, 0.0),
                        heading_rad: 0.25,
                    },
                    BodySegmentSample {
                        position: DVec2::new(-6.0, 0.5),
                        heading_rad: 0.5,
                    },
                ],
            ),
        ];

        let directory = std::env::temp_dir().join(format!(
            "tangle-trajectory-round-trip-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("scratch directory is created");
        let artifact = write_trajectories(&directory, &rows).expect("trajectories write");
        assert_eq!(artifact.format_version, TRAJECTORY_FORMAT_VERSION);
        let read = read_trajectories(&directory).expect("trajectories read back");
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(read, rows);
    }

    const WALKING: &str = r#"
    {
      schema_version: 1,
      id: 'walking_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] } ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      population: {
        vehicle_count: 2,
        vehicle_speed_mps: 12.0,
        vehicle_spacing_m: 20.0,
        vehicle_length_m: 4.5,
        vehicle_width_m: 1.8,
      },
    }
    "#;

    /// A version-2 narrow mode on a compiled facility whose reference path is
    /// the movement's path, so the recorder observes route state on a real run.
    const ROUTE_STATE_V2: &str = r#"
    {
      schema_version: 2,
      id: 'route_state_v2',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
      portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      boundaries: [ { id: 'world', points: [
        { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
        { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
      ] } ],
      regions: [ { id: 'band', points: [
        { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
        { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
      ] } ],
      facilities: [
        { id: 'bikeway', region: 'band', reference_path: 'guide',
          width_m: 3.0, nominal_direction: 'forward',
          access: { modes: [ 'rider' ] }, lateral_use: 'shared',
          speed_policy: { limit_mps: null } },
      ],
      movements: [
        { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
          direction: 'forward' },
      ],
      mode_templates: [
        {
          id: 'rider',
          body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
            radius_m: { min: 0.35, max: 0.35 } },
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield' ],
          access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
          occupancy: 'operator_only',
          profiles: {
            speed_mps: { min: 6.0, max: 6.0 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
            lateral_clearance_m: { min: 0.3, max: 0.3 },
            compliance: { min: 1.0, max: 1.0 },
          },
        },
      ],
      permissions: [],
      demand: [
        { id: 'rider_inflow', mode: 'rider',
          spawn: { rate: {
            portal: 'entry',
            rate_per_hour: 900.0,
            interval_s: { start_s: 0.0, end_s: null },
            choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
          } } },
      ],
    }
    "#;

    /// A version-2 narrow mode whose demand selects a compiled facility whose
    /// authored nominal direction is forward, travelling the facility's reverse
    /// traversal: a `nominal_direction` `permit` statement binds the pair, so
    /// every rode row carries the rule state.
    const OPPOSING_V2: &str = r#"
    {
      schema_version: 2,
      id: 'opposing_state_v2',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
      portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      boundaries: [ { id: 'world', points: [
        { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
        { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
      ] } ],
      regions: [ { id: 'band', points: [
        { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
        { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
      ] } ],
      facilities: [
        { id: 'bikeway', region: 'band', reference_path: 'guide',
          width_m: 3.0, nominal_direction: 'forward',
          access: { modes: [ 'rider' ] }, lateral_use: 'shared',
          speed_policy: { limit_mps: null } },
      ],
      movements: [
        { id: 'against', from: 'exit', to: 'entry', path: 'guide', priority: 0,
          direction: 'reverse' },
      ],
      mode_templates: [
        {
          id: 'rider',
          body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
            radius_m: { min: 0.35, max: 0.35 } },
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield' ],
          access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
          occupancy: 'operator_only',
          profiles: {
            speed_mps: { min: 6.0, max: 6.0 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
            lateral_clearance_m: { min: 0.3, max: 0.3 },
            compliance: { min: 1.0, max: 1.0 },
          },
        },
      ],
      permissions: [
        { id: 'contraflow_bikeway', kind: 'nominal_direction', holder: 'rider',
          target: 'bikeway', effect: 'permit' },
      ],
      demand: [
        { id: 'rider_inflow', mode: 'rider',
          spawn: { rate: {
            portal: 'exit',
            rate_per_hour: 900.0,
            interval_s: { start_s: 0.0, end_s: null },
            choice: { movements: [ { movement: 'against', weight: 1.0 } ] },
          } } },
      ],
    }
    "#;

    /// The recorder reads a real run's route state into the artifact: a
    /// steering body on a compiled facility writes non-null `route_s_m`,
    /// `route_d_m`, and `maneuver_state`, while the maneuver targets it has not
    /// fixed stay absent.
    #[test]
    fn a_narrow_run_records_route_coordinates_into_the_artifact() {
        use tangle_model::{CompiledScenario, parse_scenario_source_v2};

        let source = parse_scenario_source_v2(ROUTE_STATE_V2).expect("the document is version 2");
        let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
        let policy = TrajectorySampling {
            retention: crate::run_dir::TrajectoryRetention::Full,
            stride_ticks: 1,
            max_samples: 400,
        };
        let (_, _, samples) = crate::trace::canonical_run_sampled(
            scenario,
            tangle_sim::RunConfig::new(0),
            400,
            &policy,
        )
        .expect("the run completes");

        let recorded: Vec<&TrajectorySample> = samples
            .iter()
            .filter(|sample| sample.maneuver_state.is_some())
            .collect();
        assert!(
            !recorded.is_empty(),
            "a narrow run must record route state for its steering bodies"
        );
        for sample in recorded {
            assert!(sample.route_s_m.is_some(), "route_s_m must be present");
            assert!(sample.route_d_m.is_some(), "route_d_m must be present");
            assert_eq!(sample.maneuver_state, Some(ManeuverState::Following));
            assert_eq!(sample.target_offset_m, None);
            assert_eq!(sample.predicted_min_clearance_m, None);
            assert_eq!(
                sample.opposing_direction, None,
                "a forward-nominal facility's riders are never on an opposing traversal"
            );
            assert_eq!(sample.perceived_rule, None);
        }
    }

    /// The recorder reads a real run's wrong-way rule state into the artifact: a
    /// rider whose authored traversal is against its facility's rule direction
    /// writes a non-null `opposing_direction` and the `perceived_rule` the
    /// compiled statement binds. The state changes no row: the artifact holds
    /// exactly the live agent frames of the same run replayed straight through
    /// the kernel.
    #[test]
    fn an_opposing_run_records_its_rule_state_into_the_artifact() {
        use tangle_model::{CompiledScenario, parse_scenario_source_v2};

        let policy = TrajectorySampling {
            retention: crate::run_dir::TrajectoryRetention::Full,
            stride_ticks: 1,
            max_samples: 400,
        };
        let compile = || {
            let source = parse_scenario_source_v2(OPPOSING_V2).expect("the document is version 2");
            CompiledScenario::compile_v2(source).expect("the scenario compiles")
        };
        let (_, _, samples) = crate::trace::canonical_run_sampled(
            compile(),
            tangle_sim::RunConfig::new(0),
            400,
            &policy,
        )
        .expect("the run completes");

        let opposing: Vec<&TrajectorySample> = samples
            .iter()
            .filter(|sample| sample.opposing_direction.is_some())
            .collect();
        assert!(
            !opposing.is_empty(),
            "a rider travelling against its facility's rule direction carries the rule state"
        );
        for sample in &opposing {
            assert_eq!(sample.opposing_direction, Some(MovementDirection::Reverse));
            assert_eq!(
                sample.perceived_rule,
                Some(PermissionEffect::Permit),
                "the applicable statement is the perceived rule"
            );
            assert!(sample.route_s_m.is_some(), "the state rides route state");
        }

        // No row is added or duplicated for a rule-state transition: the rows are
        // exactly one per live agent frame of the same run replayed directly.
        let mut sim = Simulation::new(compile(), tangle_sim::RunConfig::new(0))
            .expect("the simulation builds");
        let mut frames = Vec::new();
        for _ in 0..400 {
            sim.step();
            let tick = sim.time().tick();
            for agent in sim.snapshot(SnapshotDetail::Full).agents() {
                frames.push((tick, agent.id.get()));
            }
        }
        let recorded: Vec<(u64, u32)> = samples
            .iter()
            .map(|sample| (sample.tick, sample.agent))
            .collect();
        assert_eq!(
            recorded, frames,
            "the rule state must add no sampled row and drop none"
        );
    }
}
