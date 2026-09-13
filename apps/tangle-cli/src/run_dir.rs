//! Immutable per-run output directory.
//!
//! A completed run directory is the durable record of one run:
//!
//! - `manifest.json` carries the provenance a rerun needs — scenario source
//!   path and content hash, seed, step and fidelity profile, the scenario
//!   schema version, [`EVENT_VERSION`], the model version, the build revision,
//!   and the declared sampling policy — and a descriptor of the event stream;
//! - `summary.json` carries the run's aggregate counts, tied back to the
//!   manifest by its SHA-256;
//! - `events.jsonl.gz` carries the canonical JSON Lines event stream,
//!   gzip-compressed. It is the canonical trace's exact bytes, so the trace
//!   golden and the trace hash golden pin this artifact too.
//!
//! The directory is immutable once complete. `manifest.json` is written last
//! and is the completion marker, so its presence means every artifact it names
//! is present and final; a second run targeting a completed directory fails
//! with [`RunDirectoryError::Completed`] and mutates nothing. A batch can
//! therefore be stopped and resumed without changing completed run artifacts.
//!
//! Nothing here reads the wall clock or the environment, so the same inputs
//! produce byte-identical artifacts: the gzip header is fixed (no file name,
//! `mtime` zero, unknown operating system) and the manifest carries no
//! capture timestamp.
//!
//! Full per-tick trajectories are opt-in and are not written here. The declared
//! [`SamplingPolicy`] is what bounds the directory's size: the sparse typed
//! event stream keeps every event, and trajectory sampling is `off`, so no
//! per-step agent state exists to grow with agents x ticks.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use tangle_model::MODEL_VERSION;
use tangle_sim::{EVENT_VERSION, RunSummary as KernelRunSummary};

use crate::baseline::{PRESETS, ScenarioProvenance};
use crate::trace::{Trace, sha256_hex};

/// Version of the run manifest format.
pub const RUN_MANIFEST_VERSION: u32 = 1;

/// Version of the run summary format.
pub const RUN_SUMMARY_VERSION: u32 = 1;

/// Version of the declared sampling policy.
pub const SAMPLING_POLICY_VERSION: u32 = 1;

/// Completion marker of a run directory; written after every other artifact.
pub const MANIFEST_FILE: &str = "manifest.json";

/// The run's aggregate summary inside a run directory.
pub const SUMMARY_FILE: &str = "summary.json";

/// The compressed canonical event stream inside a run directory.
pub const EVENT_STREAM_FILE: &str = "events.jsonl.gz";

/// Compression [`EVENT_STREAM_FILE`] uses.
pub const EVENT_STREAM_COMPRESSION: &str = "gzip";

/// Trajectory samples per `stride_ticks` steps the declared policy keeps.
///
/// Ten steps is 0.5 s at the Standard 50 ms step, which samples a trajectory
/// finely enough to plot while staying two orders of magnitude below the
/// per-step stream.
pub const DEFAULT_TRAJECTORY_STRIDE_TICKS: u64 = 10;

/// Hard ceiling on trajectory samples in one run directory.
///
/// The stride bounds ordinary runs; the ceiling is the backstop that keeps a
/// long run's trajectory output bounded as well.
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
}

/// Which sparse typed events the event stream keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventRetention {
    /// Every event the kernel emits is a record: the stream is complete.
    #[default]
    All,
}

/// Whether full per-step trajectory state is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryRetention {
    /// No trajectory state is written; the caller must opt in.
    #[default]
    Off,
}

/// How full trajectories would be sampled when a caller opts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrajectorySampling {
    /// Whether trajectory state is written at all.
    pub retention: TrajectoryRetention,
    /// Steps between trajectory samples: one sample every `stride_ticks` steps.
    pub stride_ticks: u64,
    /// Most trajectory samples one run directory may hold.
    pub max_samples: u64,
}

/// The sampling policy a run manifest declares.
///
/// Sparse state transitions and safety events are always event records, so the
/// typed event stream is never sampled; only the high-volume per-step
/// trajectory state is subject to sampling, and it is off unless a caller opts
/// in. That declaration is what bounds a run directory's output size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplingPolicy {
    /// Policy format version.
    pub policy_version: u32,
    /// Retention of the sparse typed event stream.
    pub events: EventRetention,
    /// Sampling of full per-step trajectories.
    pub trajectories: TrajectorySampling,
}

/// The policy every run directory declares: every event, no trajectories.
impl Default for SamplingPolicy {
    fn default() -> Self {
        Self {
            policy_version: SAMPLING_POLICY_VERSION,
            events: EventRetention::default(),
            trajectories: TrajectorySampling {
                retention: TrajectoryRetention::default(),
                stride_ticks: DEFAULT_TRAJECTORY_STRIDE_TICKS,
                max_samples: DEFAULT_MAX_TRAJECTORY_SAMPLES,
            },
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
    /// Authored scenario provenance: id, source path, schema version, content hash.
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
    /// Declared sampling policy that bounds the directory's output size.
    pub sampling: SamplingPolicy,
    /// The compressed event stream this manifest describes.
    pub stream: EventStream,
}

/// `summary.json`: the run's aggregate output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Summary format version.
    pub summary_version: u32,
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
    /// The completed run's canonical trace.
    pub trace: &'a Trace,
    /// The kernel's summary of the completed run.
    pub summary: &'a KernelRunSummary,
}

/// Write the immutable run directory for a completed run.
///
/// The directory is created when missing and must be empty: a completed run
/// (`manifest.json` present) is never rewritten and unknown pre-existing
/// content is never clobbered, so both fail with a [`RunDirectoryError`] and
/// leave the target untouched. Inside a fresh directory the event stream and
/// the summary are written first and `manifest.json` last, so a directory only
/// reads as completed once every artifact the manifest names is present.
pub fn write_run_directory(
    directory: &Path,
    request: RunDirectoryRequest<'_>,
) -> Result<(), RunDirectoryError> {
    ensure_writable(directory)?;

    let manifest = RunManifest::new(&request);
    let manifest_json = to_pretty_json(&manifest);
    let summary = RunSummary::new(&manifest_json, request.summary);
    let summary_json = to_pretty_json(&summary);
    let stream = gzip(request.trace.bytes());

    fs::create_dir_all(directory).map_err(|source| RunDirectoryError::Io {
        path: directory.to_path_buf(),
        action: "create",
        source,
    })?;
    write_file(&directory.join(EVENT_STREAM_FILE), &stream)?;
    write_file(&directory.join(SUMMARY_FILE), summary_json.as_bytes())?;
    write_file(&directory.join(MANIFEST_FILE), manifest_json.as_bytes())?;
    Ok(())
}

impl RunManifest {
    /// The manifest for a completed run.
    fn new(request: &RunDirectoryRequest<'_>) -> Self {
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
            sampling: SamplingPolicy::default(),
            stream: EventStream {
                path: EVENT_STREAM_FILE.to_owned(),
                compression: EVENT_STREAM_COMPRESSION.to_owned(),
                records: count_records(request.trace.bytes()),
                uncompressed_bytes: request.trace.bytes().len(),
                uncompressed_sha256: request.trace.hash().to_owned(),
            },
        }
    }
}

impl RunSummary {
    /// The summary of a completed run, tied to the manifest bytes it describes.
    fn new(manifest_json: &str, summary: &KernelRunSummary) -> Self {
        Self {
            summary_version: RUN_SUMMARY_VERSION,
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
/// reproducible either way because it also records the numeric `step_s`.
fn fidelity(step_s: f64) -> &'static str {
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

    /// The declared policy keeps every event and writes no trajectory state, so
    /// the directory cannot grow with agents x ticks.
    #[test]
    fn the_declared_policy_retains_every_event_and_no_trajectory_state() {
        let policy = SamplingPolicy::default();
        assert_eq!(policy.policy_version, SAMPLING_POLICY_VERSION);
        assert_eq!(policy.events, EventRetention::All);
        assert_eq!(policy.trajectories.retention, TrajectoryRetention::Off);
        assert_eq!(
            policy.trajectories.stride_ticks,
            DEFAULT_TRAJECTORY_STRIDE_TICKS
        );
        assert_eq!(
            policy.trajectories.max_samples,
            DEFAULT_MAX_TRAJECTORY_SAMPLES
        );
        assert!(policy.trajectories.stride_ticks >= 1);
        assert!(policy.trajectories.max_samples >= 1);
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
