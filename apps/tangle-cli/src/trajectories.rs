//! Selectively sampled agent trajectories, written as Parquet.
//!
//! The run directory's trajectory artifact is the per-agent motion record a
//! consumer plots or aggregates: one row per sampled agent frame, holding the
//! agent's mode, world position, heading, and longitudinal speed at one
//! completed tick.
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

use arrow_array::{
    Array, ArrayRef, Float64Array, RecordBatch, StringArray, UInt32Array, UInt64Array,
};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::errors::ParquetError;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use tangle_sim::{Simulation, SnapshotDetail};

use crate::run_dir::TrajectorySampling;
use crate::trace::sha256_hex;

/// File name of the sampled-trajectory artifact inside a run directory.
pub const TRAJECTORY_FILE: &str = "trajectories.parquet";

/// File format [`TRAJECTORY_FILE`] uses.
pub const TRAJECTORY_FORMAT: &str = "parquet";

/// The trajectory columns, in schema and declaration order.
///
/// One row is one sampled agent frame. The order is part of the artifact
/// contract: a consumer that reads by position and one that reads by name see
/// the same record. Units are spelled into the names, as the event records do.
const TRAJECTORY_COLUMNS: [(&str, DataType); 7] = [
    ("tick", DataType::UInt64),
    ("agent", DataType::UInt32),
    ("mode", DataType::Utf8),
    ("x_m", DataType::Float64),
    ("y_m", DataType::Float64),
    ("heading_rad", DataType::Float64),
    ("speed_mps", DataType::Float64),
];

/// One sampled trajectory row: one agent observed at one completed step.
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectorySample {
    /// Completed fixed step the sample was taken at.
    pub tick: u64,
    /// Stable agent identifier, the same value the event records carry.
    pub agent: u32,
    /// Agent mode label, matching the `spawned` event's `mode` field.
    pub mode: String,
    /// World position east of the origin in metres.
    pub x_m: f64,
    /// World position north of the origin in metres.
    pub y_m: f64,
    /// World heading in radians.
    pub heading_rad: f64,
    /// Longitudinal speed in metres per second.
    pub speed_mps: f64,
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
                .expect("a full snapshot detail carries motion");
            self.samples.push(TrajectorySample {
                tick,
                agent: sample.id.get(),
                mode: motion.mode.label().to_owned(),
                x_m: sample.position.x,
                y_m: sample.position.y,
                heading_rad: sample.heading_rad,
                speed_mps: motion.speed_mps,
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

/// The artifact's Arrow schema, built from [`TRAJECTORY_COLUMNS`].
fn trajectory_schema() -> SchemaRef {
    Arc::new(Schema::new(
        TRAJECTORY_COLUMNS
            .iter()
            .map(|(name, data_type)| Field::new(*name, data_type.clone(), false))
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
    ];
    RecordBatch::try_new(schema.clone(), columns)
        .expect("every column of the trajectory schema has the row count")
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
    xs: &'a Float64Array,
    ys: &'a Float64Array,
    headings: &'a Float64Array,
    speeds: &'a Float64Array,
}

impl<'a> TrajectoryColumns<'a> {
    /// Check the batch's schema and bind its columns.
    fn new(batch: &'a RecordBatch, path: &Path) -> Result<Self, TrajectoryError> {
        let schema = batch.schema();
        for (index, (name, data_type)) in TRAJECTORY_COLUMNS.iter().enumerate() {
            let Some(field) = schema.fields().get(index) else {
                return Err(schema_error(
                    path,
                    format!("column {index} ('{name}') is absent"),
                ));
            };
            if field.name() != name || field.data_type() != data_type {
                return Err(schema_error(
                    path,
                    format!(
                        "column {index} is '{}' of type {}, not '{name}' of type {data_type}",
                        field.name(),
                        field.data_type()
                    ),
                ));
            }
        }
        if schema.fields().len() != TRAJECTORY_COLUMNS.len() {
            return Err(schema_error(
                path,
                format!(
                    "the artifact holds {} columns, not {}",
                    schema.fields().len(),
                    TRAJECTORY_COLUMNS.len()
                ),
            ));
        }
        // The schema check above proves each column's type, so the downcasts
        // cannot fail.
        Ok(Self {
            ticks: downcast(batch, 0),
            agents: downcast(batch, 1),
            modes: downcast(batch, 2),
            xs: downcast(batch, 3),
            ys: downcast(batch, 4),
            headings: downcast(batch, 5),
            speeds: downcast(batch, 6),
        })
    }

    /// The row at `row` as a trajectory sample.
    fn sample(&self, row: usize) -> TrajectorySample {
        TrajectorySample {
            tick: self.ticks.value(row),
            agent: self.agents.value(row),
            mode: self.modes.value(row).to_owned(),
            x_m: self.xs.value(row),
            y_m: self.ys.value(row),
            heading_rad: self.headings.value(row),
            speed_mps: self.speeds.value(row),
        }
    }
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

    /// The declared columns are the schema: names, types, order, and no
    /// nullability, so a reader can trust either names or positions.
    #[test]
    fn the_declared_columns_are_the_written_schema() {
        let schema = trajectory_schema();
        assert_eq!(schema.fields().len(), TRAJECTORY_COLUMNS.len());
        for (field, (name, data_type)) in schema.fields().iter().zip(TRAJECTORY_COLUMNS) {
            assert_eq!(field.name(), name);
            assert_eq!(field.data_type(), &data_type);
            assert!(!field.is_nullable());
        }
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
        let motion = agent.motion.expect("full detail carries motion");
        assert_eq!(
            recorder.samples(),
            [TrajectorySample {
                tick: 1,
                agent: agent.id.get(),
                mode: motion.mode.label().to_owned(),
                x_m: agent.position.x,
                y_m: agent.position.y,
                heading_rad: agent.heading_rad,
                speed_mps: motion.speed_mps,
            }]
        );
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
}
