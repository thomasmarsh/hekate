//! Immutable per-run output directory.
//!
//! A completed run directory is the durable record of one run:
//!
//! - `manifest.json` carries the provenance a rerun needs — scenario source
//!   path and content hash, seed, step and fidelity profile, the scenario
//!   schema version, [`EVENT_VERSION`], the model version, the build revision,
//!   and the declared sampling policy — and a descriptor of the event stream;
//! - `summary.json` carries the run's aggregate counts and the metric
//!   definition revision its `metrics.json` reports, tied back to the manifest
//!   by its SHA-256;
//! - `events.jsonl.gz` carries the canonical JSON Lines event stream,
//!   gzip-compressed. It is the canonical trace's exact bytes, so the trace
//!   golden and the trace hash golden pin this artifact too;
//! - `trajectories.parquet` carries the selectively sampled agent trajectories
//!   the declared sampling policy retains, with the manifest's descriptor
//!   naming the file, its row count, and its SHA-256.
//!
//! The directory is immutable once complete. `manifest.json` is written last
//! and is the completion marker, so its presence means every artifact it names
//! is present and final; it is written through [`MANIFEST_TEMP_FILE`] and
//! renamed into place, so an interrupted write leaves no truncated marker and
//! the directory still reads as incomplete. A second run targeting a completed
//! directory fails with [`RunDirectoryError::Completed`] and mutates nothing. A
//! batch can therefore be stopped and resumed without changing completed run
//! artifacts.
//!
//! Nothing here reads the wall clock or the environment, so the same inputs
//! produce byte-identical artifacts: the gzip header is fixed (no file name,
//! `mtime` zero, unknown operating system) and the manifest carries no
//! capture timestamp.
//!
//! The declared [`SamplingPolicy`] is what bounds the directory's size: the
//! sparse typed event stream keeps every event, while the high-volume per-step
//! trajectory state is sampled at a declared stride and capped at a declared
//! row count, so the artifact cannot grow with agents x ticks. Full per-tick
//! trajectories are opt-in through a policy that keeps every tick.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use hekate_model::MODEL_VERSION;
use hekate_sim::{EVENT_VERSION, RunSummary as KernelRunSummary};
use serde::{Deserialize, Serialize};

use crate::baseline::{PRESETS, ScenarioProvenance};
use crate::run_metrics::{METRIC_DEFINITION_VERSION, METRICS_FILE, RunMetrics, RunMetricsArtifact};
use crate::trace::{Trace, sha256_hex};
use crate::trajectories::{
    TrajectoryArtifact, TrajectoryError, TrajectorySample, write_trajectories,
};

/// Version of the run manifest format.
///
/// Version 3 records the trajectory artifact's column-shape version in its
/// descriptor, so a reader knows which trajectory columns to expect before it
/// opens the Parquet file. Version 2 adds the normalized version-2 hash and the
/// migration version to the scenario provenance ([`ScenarioProvenance`]), so a
/// manifest attributes a run to a source, a migration, or the kernel. Version 1
/// manifests omit all of those fields.
pub const RUN_MANIFEST_VERSION: u32 = 3;

/// Version of the run summary format.
pub const RUN_SUMMARY_VERSION: u32 = 1;

/// Version of the declared sampling policy.
pub const SAMPLING_POLICY_VERSION: u32 = 1;

/// Completion marker of a run directory; written after every other artifact.
pub const MANIFEST_FILE: &str = "manifest.json";

/// File the completion marker is staged in before it is renamed into place.
///
/// The marker's presence is what makes a run directory complete, so it is
/// written atomically: a crash mid-write leaves this file rather than a
/// truncated `manifest.json`, which would read as a complete run no consumer
/// can parse. A leftover staging file is not a marker, so a directory holding
/// one reads as a partial run that a batch clears and re-runs.
pub const MANIFEST_TEMP_FILE: &str = "manifest.json.tmp";

/// The run's aggregate summary inside a run directory.
pub const SUMMARY_FILE: &str = "summary.json";

/// The compressed canonical event stream inside a run directory.
pub const EVENT_STREAM_FILE: &str = "events.jsonl.gz";

/// Compression [`EVENT_STREAM_FILE`] uses.
pub const EVENT_STREAM_COMPRESSION: &str = "gzip";

/// Steps between the trajectory samples the default policy keeps.
///
/// Ten steps is 0.5 s at the Standard 50 ms step, which samples a trajectory
/// finely enough to plot while staying two orders of magnitude below the
/// per-step stream.
pub const DEFAULT_TRAJECTORY_STRIDE_TICKS: u64 = 10;

/// Most trajectory rows the default policy keeps in one run directory.
///
/// The stride bounds an ordinary run; the cap is the backstop that keeps a very
/// long run's trajectory artifact bounded as well.
pub const DEFAULT_MAX_TRAJECTORY_SAMPLES: u64 = 100_000;

/// Fidelity label for a step that is not one of the Phase 1 presets.
const FIDELITY_CUSTOM: &str = "custom";

/// Failure to write an immutable run directory.
#[derive(Debug, thiserror::Error)]
pub enum RunDirectoryError {
    /// The target already holds a completed run, which is never rewritten.
    #[error(
        "run directory '{path}' already holds a completed run; a completed run directory is never rewritten"
    )]
    Completed {
        /// The directory that holds the completed run.
        path: PathBuf,
    },
    /// The target exists and holds content that is not a completed run.
    #[error(
        "run directory '{path}' exists and is not empty; remove it or choose another destination"
    )]
    NotEmpty {
        /// The directory that holds foreign content.
        path: PathBuf,
    },
    /// The directory could not be inspected or written.
    #[error("cannot {action} run directory '{path}': {source}")]
    Io {
        /// The directory the operation targeted.
        path: PathBuf,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The sampled-trajectory artifact could not be written.
    #[error(transparent)]
    Trajectory(#[from] TrajectoryError),
}

/// Which sparse typed events the event stream keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventRetention {
    /// Every event the kernel emits is a record: the stream is complete.
    #[default]
    All,
}

/// How much per-step trajectory state is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryRetention {
    /// No trajectory artifact is written at all.
    Off,
    /// The bounded sample the policy's stride and cap retain.
    #[default]
    Sampled,
    /// Every tick, with no cap: full trajectories, which a caller opts into.
    Full,
}

/// How per-step trajectory state is sampled.
///
/// A trajectory sample is one agent observed at one tick, which is one row of
/// the run directory's Parquet artifact. The cap therefore bounds the
/// artifact's row count and, with it, its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectorySampling {
    /// Whether trajectory state is written at all, and how fully.
    pub retention: TrajectoryRetention,
    /// Steps between sampled ticks: a tick is sampled when it is a multiple of
    /// `stride_ticks`. A declared stride of zero is read as one step.
    pub stride_ticks: u64,
    /// Most rows one run directory's trajectory artifact may hold, in canonical
    /// order. Rows past the cap are dropped rather than written; a full
    /// trajectory declares `u64::MAX`, so its cap cannot bind.
    pub max_samples: u64,
}

impl Default for TrajectorySampling {
    fn default() -> Self {
        Self {
            retention: TrajectoryRetention::default(),
            stride_ticks: DEFAULT_TRAJECTORY_STRIDE_TICKS,
            max_samples: DEFAULT_MAX_TRAJECTORY_SAMPLES,
        }
    }
}

impl TrajectorySampling {
    /// The policy that writes no trajectory artifact.
    pub const fn off() -> Self {
        Self {
            retention: TrajectoryRetention::Off,
            stride_ticks: DEFAULT_TRAJECTORY_STRIDE_TICKS,
            max_samples: DEFAULT_MAX_TRAJECTORY_SAMPLES,
        }
    }

    /// The opt-in policy that keeps every tick, with no cap.
    ///
    /// The stride is one step and the cap is `u64::MAX`, so the declared policy
    /// is the policy applied: a full artifact is never truncated.
    pub const fn full() -> Self {
        Self {
            retention: TrajectoryRetention::Full,
            stride_ticks: 1,
            max_samples: u64::MAX,
        }
    }

    /// Whether this policy writes a trajectory artifact at all.
    pub const fn writes_artifact(self) -> bool {
        !matches!(self.retention, TrajectoryRetention::Off)
    }

    /// Whether the completed step `tick` is sampled.
    pub const fn samples_tick(self, tick: u64) -> bool {
        if !self.writes_artifact() {
            return false;
        }
        let stride = if self.stride_ticks == 0 {
            1
        } else {
            self.stride_ticks
        };
        tick.is_multiple_of(stride)
    }

    /// Whether a row set of `rows` rows may still grow under the declared cap.
    pub const fn below_cap(self, rows: u64) -> bool {
        rows < self.max_samples
    }
}

/// The sampling policy a run manifest declares.
///
/// Sparse state transitions and safety events are always event records, so the
/// typed event stream is never sampled; only the high-volume per-step
/// trajectory state is sampled, at a declared stride and cap that bound a run
/// directory's output size. The policy a manifest records is the policy the run
/// applied, so a consumer can tell a bounded sample from a full trajectory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplingPolicy {
    /// Policy format version.
    pub policy_version: u32,
    /// Retention of the sparse typed event stream.
    pub events: EventRetention,
    /// Sampling of full per-step trajectories.
    pub trajectories: TrajectorySampling,
}

/// The policy every run directory declares: every event, a bounded trajectory
/// sample.
impl Default for SamplingPolicy {
    fn default() -> Self {
        Self {
            policy_version: SAMPLING_POLICY_VERSION,
            events: EventRetention::default(),
            trajectories: TrajectorySampling::default(),
        }
    }
}

impl SamplingPolicy {
    /// The declared policy for a run that keeps full per-tick trajectories.
    ///
    /// Full trajectories are opt-in: the default samples a bounded subset of
    /// ticks, and only a policy that keeps every tick writes an unreduced
    /// trajectory artifact.
    pub const fn full_trajectories() -> Self {
        Self {
            policy_version: SAMPLING_POLICY_VERSION,
            events: EventRetention::All,
            trajectories: TrajectorySampling::full(),
        }
    }
}

/// The compressed event stream a manifest describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventStream {
    /// File name inside the run directory.
    pub path: String,
    /// Compression the file uses.
    pub compression: String,
    /// Records in the uncompressed stream, one per line.
    pub records: u64,
    /// Size of the uncompressed stream in bytes.
    pub uncompressed_bytes: usize,
    /// SHA-256 of the uncompressed stream: the canonical trace hash.
    pub uncompressed_sha256: String,
}

/// `manifest.json`: the provenance and policy that make a run reproducible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunManifest {
    /// Manifest format version.
    pub manifest_version: u32,
    /// Authored scenario provenance: id, source path, schema version, source
    /// content hash, normalized version-2 hash, and migration version.
    pub scenario: ScenarioProvenance,
    /// Root seed the run used.
    pub seed: u64,
    /// Fixed steps advanced.
    pub ticks: u64,
    /// Phase 1 fidelity preset the fixed step corresponds to.
    pub fidelity: String,
    /// Fixed physics step in seconds.
    pub step_s: f64,
    /// Typed event schema version of the stream's records.
    pub event_version: u32,
    /// Kernel model version the run used.
    pub model_version: String,
    /// Crate version of the command that wrote the directory.
    ///
    /// The workspace stamps no source revision into a build, so the producing
    /// binary's crate version is the available build revision.
    pub build_revision: String,
    /// Sampling policy the run applied, which bounds the directory's output
    /// size.
    pub sampling: SamplingPolicy,
    /// The compressed event stream this manifest describes.
    pub stream: EventStream,
    /// The sampled-trajectory artifact this manifest describes, absent when the
    /// applied policy retains no trajectory state.
    pub trajectories: Option<TrajectoryArtifact>,
}

/// `summary.json`: the run's aggregate output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Summary format version.
    pub summary_version: u32,
    /// The metric definition revision the run's `metrics.json` reports, so a
    /// reader can attribute the run's numbers to the definition that fixed them.
    pub metric_definition_version: u32,
    /// SHA-256 of the manifest's exact bytes, which ties every value here back
    /// to the provenance that produced it.
    pub manifest_sha256: String,
    /// Completed fixed steps.
    pub ticks: u64,
    /// Agents spawned over the run.
    pub spawned: u64,
    /// Agents despawned over the run.
    pub despawned: u64,
    /// Agents still alive at the end.
    pub remaining: usize,
    /// Simulated duration in seconds.
    pub elapsed_s: f64,
}

/// Inputs needed to write one immutable run directory.
#[derive(Debug, Clone, Copy)]
pub struct RunDirectoryRequest<'a> {
    /// Authored scenario provenance, as the loader reports it.
    pub scenario: &'a ScenarioProvenance,
    /// Root seed the run used.
    pub seed: u64,
    /// Fixed step in seconds the run used.
    pub step_s: f64,
    /// Sampling policy the run applied, recorded as applied in the manifest.
    pub sampling: SamplingPolicy,
    /// The completed run's canonical trace.
    pub trace: &'a Trace,
    /// The trajectory rows the applied policy kept, in canonical order.
    pub trajectories: &'a [TrajectorySample],
    /// The kernel's summary of the completed run.
    pub summary: &'a KernelRunSummary,
    /// The run's captured metric values, written as `metrics.json`.
    pub metrics: &'a RunMetrics,
}

/// Write the immutable run directory for a completed run.
///
/// The directory is created when missing and must be empty: a completed run
/// (`manifest.json` present) is never rewritten and unknown pre-existing
/// content is never clobbered, so both fail with a [`RunDirectoryError`] and
/// leave the target untouched. Inside a fresh directory the artifacts the
/// manifest describes are written first and `manifest.json` last, so a
/// directory only reads as completed once every artifact the manifest names is
/// present.
pub fn write_run_directory(
    directory: &Path,
    request: RunDirectoryRequest<'_>,
) -> Result<(), RunDirectoryError> {
    ensure_writable(directory)?;
    fs::create_dir_all(directory).map_err(|source| RunDirectoryError::Io {
        path: directory.to_path_buf(),
        action: "create",
        source,
    })?;

    let trajectory = match request.sampling.trajectories.writes_artifact() {
        true => Some(write_trajectories(directory, request.trajectories)?),
        false => None,
    };
    let manifest = RunManifest::new(&request, trajectory);
    let manifest_json = to_pretty_json(&manifest);
    let summary = RunSummary::new(&manifest_json, request.summary);
    let summary_json = to_pretty_json(&summary);
    let metrics = RunMetricsArtifact::new(&manifest_json, request.metrics);
    let metrics_json = to_pretty_json(&metrics);
    let stream = gzip(request.trace.bytes());

    write_file(&directory.join(EVENT_STREAM_FILE), &stream)?;
    write_file(&directory.join(SUMMARY_FILE), summary_json.as_bytes())?;
    write_file(&directory.join(METRICS_FILE), metrics_json.as_bytes())?;
    write_manifest(directory, manifest_json.as_bytes())?;
    Ok(())
}

impl RunManifest {
    /// The manifest for a completed run, describing the artifacts on disk.
    fn new(request: &RunDirectoryRequest<'_>, trajectories: Option<TrajectoryArtifact>) -> Self {
        Self {
            manifest_version: RUN_MANIFEST_VERSION,
            scenario: request.scenario.clone(),
            seed: request.seed,
            ticks: request.summary.ticks(),
            fidelity: fidelity(request.step_s).to_owned(),
            step_s: request.step_s,
            event_version: EVENT_VERSION,
            model_version: MODEL_VERSION.to_owned(),
            build_revision: env!("CARGO_PKG_VERSION").to_owned(),
            sampling: request.sampling,
            stream: EventStream {
                path: EVENT_STREAM_FILE.to_owned(),
                compression: EVENT_STREAM_COMPRESSION.to_owned(),
                records: count_records(request.trace.bytes()),
                uncompressed_bytes: request.trace.bytes().len(),
                uncompressed_sha256: request.trace.hash().to_owned(),
            },
            trajectories,
        }
    }
}

impl RunSummary {
    /// The summary of a completed run, tied to the manifest bytes it describes.
    fn new(manifest_json: &str, summary: &KernelRunSummary) -> Self {
        Self {
            summary_version: RUN_SUMMARY_VERSION,
            metric_definition_version: METRIC_DEFINITION_VERSION,
            manifest_sha256: sha256_hex(manifest_json.as_bytes()),
            ticks: summary.ticks(),
            spawned: summary.spawned(),
            despawned: summary.despawned(),
            remaining: summary.remaining(),
            elapsed_s: summary.elapsed().as_secs(),
        }
    }
}

/// Reject a target that must not be written.
fn ensure_writable(directory: &Path) -> Result<(), RunDirectoryError> {
    let mut entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(RunDirectoryError::Io {
                path: directory.to_path_buf(),
                action: "read",
                source,
            });
        }
    };
    if directory.join(MANIFEST_FILE).exists() {
        return Err(RunDirectoryError::Completed {
            path: directory.to_path_buf(),
        });
    }
    if entries.next().is_some() {
        return Err(RunDirectoryError::NotEmpty {
            path: directory.to_path_buf(),
        });
    }
    Ok(())
}

/// Gzip the canonical event stream.
///
/// The encoder writes a fixed header — no file name, `mtime` zero, unknown
/// operating system — so the compressed bytes are a pure function of the
/// uncompressed bytes.
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .expect("writing to memory cannot fail");
    encoder.finish().expect("writing to memory cannot fail")
}

/// Records in a canonical JSON Lines stream.
///
/// The canonical trace terminates every line with `\n`, so counting newlines
/// counts records.
fn count_records(bytes: &[u8]) -> u64 {
    bytes.iter().filter(|byte| **byte == b'\n').count() as u64
}

/// The Phase 1 fidelity preset a fixed step corresponds to.
///
/// `run` advances the Standard step, so the label normally reads `standard`. A
/// step no preset uses is labelled [`FIDELITY_CUSTOM`]; the manifest stays
/// reproducible either way because it also records the numeric `step_s`. The
/// batch manifest labels its specification the same way, so both manifests name
/// a step identically.
pub(crate) fn fidelity(step_s: f64) -> &'static str {
    PRESETS
        .iter()
        .find(|preset| preset.step_s == step_s)
        .map_or(FIDELITY_CUSTOM, |preset| preset.name)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), RunDirectoryError> {
    fs::write(path, bytes).map_err(|source| RunDirectoryError::Io {
        path: path.to_path_buf(),
        action: "write",
        source,
    })
}

/// Write the completion marker through [`MANIFEST_TEMP_FILE`] and rename it into
/// place, so `manifest.json` is either absent or complete and never truncated.
fn write_manifest(directory: &Path, bytes: &[u8]) -> Result<(), RunDirectoryError> {
    let temporary = directory.join(MANIFEST_TEMP_FILE);
    write_file(&temporary, bytes)?;
    let marker = directory.join(MANIFEST_FILE);
    fs::rename(&temporary, &marker).map_err(|source| RunDirectoryError::Io {
        path: marker,
        action: "write",
        source,
    })
}

/// Pretty JSON with a trailing newline: the on-disk form of a run artifact.
fn to_pretty_json<T: Serialize>(value: &T) -> String {
    let mut json = serde_json::to_string_pretty(value).expect("run artifacts serialize");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_step_carries_the_standard_fidelity_label() {
        assert_eq!(fidelity(0.05), "standard");
        assert_eq!(fidelity(0.1), "fast");
        assert_eq!(fidelity(0.02), "fine");
    }

    /// A step outside the Phase 1 presets is still reproducible: the numeric
    /// step is recorded, and the label says the step is not a preset.
    #[test]
    fn a_step_outside_the_presets_is_labelled_custom() {
        assert_eq!(fidelity(0.03), FIDELITY_CUSTOM);
    }

    /// The declared policy keeps every event and a bounded trajectory sample,
    /// so the directory cannot grow with agents x ticks.
    #[test]
    fn the_declared_policy_keeps_every_event_and_a_bounded_sample() {
        let policy = SamplingPolicy::default();
        assert_eq!(policy.policy_version, SAMPLING_POLICY_VERSION);
        assert_eq!(policy.events, EventRetention::All);
        assert_eq!(policy.trajectories.retention, TrajectoryRetention::Sampled);
        assert_eq!(
            policy.trajectories.stride_ticks,
            DEFAULT_TRAJECTORY_STRIDE_TICKS
        );
        assert_eq!(
            policy.trajectories.max_samples,
            DEFAULT_MAX_TRAJECTORY_SAMPLES
        );
        assert!(policy.trajectories.writes_artifact());
        assert!(policy.trajectories.stride_ticks >= 1);
        assert!(policy.trajectories.max_samples >= 1);
        assert!(
            policy
                .trajectories
                .samples_tick(DEFAULT_TRAJECTORY_STRIDE_TICKS)
        );
        assert!(!policy.trajectories.samples_tick(1));
        assert!(
            policy
                .trajectories
                .below_cap(DEFAULT_MAX_TRAJECTORY_SAMPLES - 1)
        );
        assert!(
            !policy
                .trajectories
                .below_cap(DEFAULT_MAX_TRAJECTORY_SAMPLES)
        );
    }

    /// Full trajectories are the opt-in policy, and the policy they declare is
    /// the policy applied: every tick, no cap.
    #[test]
    fn the_full_policy_keeps_every_tick_with_no_cap() {
        let policy = SamplingPolicy::full_trajectories();
        assert_eq!(policy.events, EventRetention::All);
        assert_eq!(policy.trajectories.retention, TrajectoryRetention::Full);
        assert_eq!(policy.trajectories.stride_ticks, 1);
        assert_eq!(policy.trajectories.max_samples, u64::MAX);
        assert!(policy.trajectories.samples_tick(1));
        assert!(policy.trajectories.samples_tick(250));
        assert!(policy.trajectories.below_cap(u64::MAX - 1));
    }

    /// A policy that writes no trajectories samples nothing, whatever its
    /// stride says; a declared stride of zero is read as one step.
    #[test]
    fn a_policy_samples_exactly_the_ticks_it_keeps() {
        let off = TrajectorySampling::off();
        assert!(!off.writes_artifact());
        assert!(!off.samples_tick(10));
        let zero_stride = TrajectorySampling {
            stride_ticks: 0,
            ..TrajectorySampling::default()
        };
        assert!(zero_stride.samples_tick(1));
        assert!(zero_stride.samples_tick(2));
    }

    #[test]
    fn records_are_counted_one_per_line() {
        assert_eq!(count_records(b""), 0);
        assert_eq!(count_records(b"{}\n"), 1);
        assert_eq!(count_records(b"{}\n{}\n"), 2);
    }

    /// The gzip header is fixed, so compression is a pure function of its input.
    #[test]
    fn compression_is_deterministic_and_reversible() {
        use std::io::Read;

        let original = b"{\"kind\":\"event\"}\n{\"kind\":\"summary\"}\n";
        let first = gzip(original);
        assert_eq!(first, gzip(original));
        let mut decoded = Vec::new();
        flate2::read::GzDecoder::new(&first[..])
            .read_to_end(&mut decoded)
            .expect("stream decompresses");
        assert_eq!(decoded, original);
    }
}
