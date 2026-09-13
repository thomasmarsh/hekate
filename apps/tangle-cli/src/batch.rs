//! A resumable, optionally parallel batch of independent runs.
//!
//! A batch runs one scenario once per seed. Each seed produces its own
//! immutable run directory under one output root, written by the same
//! [`write_run_directory`] the `run` command uses, and the batch writes one
//! `batch.json` recording the specification, the seed list, and each run's
//! directory and trace hash in ascending seed order.
//!
//! Two properties make a batch a durable experiment record:
//!
//! - **Resumable.** Resume is detected from the artifacts on disk, never from
//!   in-memory state: a run directory whose `manifest.json` completion marker
//!   is present is skipped and never touched, while a directory without the
//!   marker is cleared and re-run from scratch — including a directory whose
//!   marker write was interrupted, which holds the staging file and no marker.
//!   Re-invoking a completed batch rewrites `batch.json` byte for byte and
//!   changes no completed artifact.
//! - **Parallel equals serial.** `jobs` is an outer loop over whole
//!   single-threaded runs, each owning a private simulation, so a parallel
//!   batch and a serial batch produce identical per-run trace hashes and
//!   identical event streams. The kernel tick is never parallelized.
//!
//! Ordering is deterministic: the seed list and the manifest's `runs` are
//! sorted ascending, so `batch.json` does not depend on the order seeds were
//! given, on how many workers ran, or on the order runs finished.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tangle_model::{CompiledScenario, MODEL_VERSION};
use tangle_sim::{EVENT_VERSION, InitError, RunConfig};

use crate::baseline::ScenarioProvenance;
use crate::run_dir::{
    EVENT_STREAM_FILE, MANIFEST_FILE, MANIFEST_TEMP_FILE, RunDirectoryError, RunDirectoryRequest,
    RunManifest, SUMMARY_FILE, SamplingPolicy, fidelity, write_run_directory,
};
use crate::run_metrics::METRICS_FILE;
use crate::seed_bank::SeedBankReference;
use crate::trace::{canonical_run_captured, sha256_hex};
use crate::trajectories::TRAJECTORY_FILE;

/// Version of the batch manifest format.
pub const BATCH_MANIFEST_VERSION: u32 = 1;

/// Batch-level manifest file inside the output root.
pub const BATCH_MANIFEST_FILE: &str = "batch.json";

/// Prefix of a per-run directory inside a batch output root.
const RUN_DIRECTORY_PREFIX: &str = "seed-";

/// Failure to run a batch or to read back what it wrote.
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    /// The batch was asked to run no seeds.
    #[error("a batch needs at least one seed")]
    NoSeeds,
    /// A batch artifact could not be inspected or written.
    #[error("cannot {action} batch artifact '{path}': {source}")]
    Io {
        /// The path the operation targeted.
        path: PathBuf,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// A seed's directory holds content that is neither a completed run nor a
    /// partial run this batch may re-run.
    #[error(
        "run directory '{path}' for seed {seed} holds content that is neither a completed run nor a partial run; remove it or choose another output root"
    )]
    ForeignContent {
        /// The directory that holds foreign content.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
    },
    /// A completed run directory exists but was not produced by this batch's
    /// specification, so resuming would record the wrong provenance.
    #[error(
        "completed run directory '{path}' for seed {seed} does not match the batch specification ({field}); remove it or choose another output root"
    )]
    Mismatch {
        /// The completed run directory that disagrees.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The specification field that disagrees.
        field: &'static str,
    },
    /// A run directory's manifest could not be read back as JSON.
    #[error("run directory '{path}' for seed {seed} holds no readable manifest: {source}")]
    Manifest {
        /// The run directory whose manifest failed to parse.
        path: PathBuf,
        /// The seed whose directory it is.
        seed: u64,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// A run's simulation could not be initialized.
    #[error("seed {seed} could not be simulated: {source}")]
    Init {
        /// The seed whose run failed to build.
        seed: u64,
        /// The kernel's initialization failure.
        #[source]
        source: InitError,
    },
    /// A run directory could not be written.
    #[error(transparent)]
    RunDirectory(#[from] RunDirectoryError),
}

/// The specification every run of a batch shares.
///
/// This is the run manifest's provenance and policy without the per-run stream
/// descriptor, so a consumer can tell what the batch ran and reproduce any run
/// from the specification in `batch.json` alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchSpec {
    /// Authored scenario provenance: id, source path, schema version, content hash.
    pub scenario: ScenarioProvenance,
    /// Fixed steps each run advances.
    pub ticks: u64,
    /// Phase 1 fidelity preset the fixed step corresponds to.
    pub fidelity: String,
    /// Fixed physics step in seconds.
    pub step_s: f64,
    /// Typed event schema version of every run's records.
    pub event_version: u32,
    /// Kernel model version every run used.
    pub model_version: String,
    /// Crate version of the command that wrote the batch.
    pub build_revision: String,
    /// Sampling policy every run applied.
    pub sampling: SamplingPolicy,
}

/// One run of a batch: the seed, its run directory, and its trace hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchRun {
    /// Root seed the run used.
    pub seed: u64,
    /// Run directory path relative to the batch output root.
    pub directory: String,
    /// SHA-256 of the run's canonical trace; equals the run manifest's
    /// `stream.uncompressed_sha256`, which is the link from the batch to the run.
    pub trace_sha256: String,
    /// SHA-256 of the run directory's `manifest.json` bytes; equals the run
    /// summary's `manifest_sha256`.
    pub manifest_sha256: String,
}

/// `batch.json`: the specification, seed list, and per-run links of one batch.
///
/// `runs` is ordered by ascending seed — not by the order seeds were given or
/// by the order runs finished — so the manifest is identical for serial and
/// parallel execution and across resume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchManifest {
    /// Manifest format version.
    pub batch_manifest_version: u32,
    /// The specification every run shares.
    pub spec: BatchSpec,
    /// The seed bank the batch ran from, when it ran from one; `None` when it
    /// took an explicit seed list. A batch that names a bank records the bank's
    /// path and content hash, so a comparison can prove both sides consumed one
    /// bank artifact, and `seeds` is exactly the bank's ordered seed list.
    ///
    /// The field is omitted when absent, so a `--seeds` batch serializes
    /// exactly as it did before the field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_bank: Option<SeedBankReference>,
    /// The seeds the batch ran, ascending and deduplicated.
    pub seeds: Vec<u64>,
    /// One entry per seed, ordered by ascending seed.
    pub runs: Vec<BatchRun>,
}

/// Inputs needed to run one batch.
#[derive(Debug, Clone)]
pub struct BatchRequest {
    /// Output root that holds `batch.json` and one run directory per seed.
    pub root: PathBuf,
    /// Compiled scenario every run uses; cloned per run so each is private.
    pub scenario: CompiledScenario,
    /// Authored provenance of `scenario`, recorded in the specification.
    pub provenance: ScenarioProvenance,
    /// Fixed steps each run advances.
    pub ticks: u64,
    /// Fixed step in seconds.
    pub step_s: f64,
    /// Sampling policy every run applies.
    pub sampling: SamplingPolicy,
    /// Seeds to run; sorted and deduplicated before any run starts. When
    /// `seed_bank` is set these are the bank's ordered seeds.
    pub seeds: Vec<u64>,
    /// The seed bank `seeds` came from, recorded in the manifest verbatim, or
    /// `None` for a batch run from an explicit seed list.
    pub seed_bank: Option<SeedBankReference>,
    /// Most whole runs to execute at once. `1` is serial; a larger value runs
    /// that many independent single-threaded runs concurrently, never a
    /// parallel tick.
    pub jobs: u64,
}

/// Run a batch and write its `batch.json` manifest.
///
/// Resume happens before any run starts: a seed whose directory already holds a
/// completed run matching the specification is skipped, and one whose directory
/// lacks the completion marker is cleared and re-run. If every run succeeds the
/// manifest is rewritten last, so its presence means every run it names is
/// complete.
pub fn run_batch(request: BatchRequest) -> Result<BatchManifest, BatchError> {
    let seeds = normalize_seeds(&request.seeds)?;
    fs::create_dir_all(&request.root).map_err(|source| BatchError::Io {
        path: request.root.clone(),
        action: "create",
        source,
    })?;

    // Inspect the artifacts on disk before spending any work, so resume never
    // depends on in-memory state.
    let mut records: Vec<Option<BatchRun>> = vec![None; seeds.len()];
    let mut pending: Vec<PendingRun> = Vec::new();
    for (index, seed) in seeds.iter().enumerate() {
        match inspect(&request, *seed)? {
            Some(record) => records[index] = Some(record),
            None => pending.push(PendingRun { index, seed: *seed }),
        }
    }

    for (index, record) in execute(&request, pending)? {
        records[index] = Some(record);
    }

    let runs: Vec<BatchRun> = records
        .into_iter()
        .map(|record| record.expect("every seed has exactly one run record"))
        .collect();
    let manifest = BatchManifest {
        batch_manifest_version: BATCH_MANIFEST_VERSION,
        spec: BatchSpec {
            scenario: request.provenance.clone(),
            ticks: request.ticks,
            fidelity: fidelity(request.step_s).to_owned(),
            step_s: request.step_s,
            event_version: EVENT_VERSION,
            model_version: MODEL_VERSION.to_owned(),
            build_revision: env!("CARGO_PKG_VERSION").to_owned(),
            sampling: request.sampling,
        },
        seed_bank: request.seed_bank.clone(),
        seeds,
        runs,
    };
    write_batch_manifest(&request.root, &manifest)?;
    Ok(manifest)
}

/// A seed whose run still has to be executed, with its position in the sorted
/// seed list so the manifest can be restored to ascending order afterwards.
struct PendingRun {
    index: usize,
    seed: u64,
}

/// Sort and deduplicate the seed list, rejecting an empty one.
fn normalize_seeds(seeds: &[u64]) -> Result<Vec<u64>, BatchError> {
    if seeds.is_empty() {
        return Err(BatchError::NoSeeds);
    }
    let mut seeds = seeds.to_vec();
    seeds.sort_unstable();
    seeds.dedup();
    Ok(seeds)
}

/// The run a seed's directory holds, or `None` when it still has to run.
///
/// A completed run (completion marker present) is read back and checked against
/// the specification. A directory without the marker that holds only run
/// artifacts is a partial run this batch may re-run; anything else is foreign
/// content the batch refuses to touch.
fn inspect(request: &BatchRequest, seed: u64) -> Result<Option<BatchRun>, BatchError> {
    let directory = run_directory_path(&request.root, seed);
    if !directory.exists() {
        return Ok(None);
    }
    if directory.join(MANIFEST_FILE).exists() {
        let manifest = read_run_manifest(&directory, seed)?;
        verify_matches(&directory, seed, &manifest, request)?;
        return read_run(&directory, seed).map(Some);
    }
    if holds_only_run_artifacts(&directory)? {
        return Ok(None);
    }
    Err(BatchError::ForeignContent {
        path: directory,
        seed,
    })
}

/// Whether a directory without a completion marker holds only run artifacts,
/// so it is a partial run rather than foreign content.
///
/// [`MANIFEST_TEMP_FILE`] counts as a run artifact: the marker is written through
/// it and renamed into place, so a crash mid-write leaves the staging file and
/// no marker, which is a partial run the batch completes.
fn holds_only_run_artifacts(directory: &Path) -> Result<bool, BatchError> {
    let entries = fs::read_dir(directory).map_err(|source| BatchError::Io {
        path: directory.to_path_buf(),
        action: "read",
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| BatchError::Io {
            path: directory.to_path_buf(),
            action: "read",
            source,
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let known = name == EVENT_STREAM_FILE
            || name == SUMMARY_FILE
            || name == TRAJECTORY_FILE
            || name == MANIFEST_TEMP_FILE
            || name == METRICS_FILE;
        if !known {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Every specification field a resumed run must agree with.
fn verify_matches(
    directory: &Path,
    seed: u64,
    manifest: &RunManifest,
    request: &BatchRequest,
) -> Result<(), BatchError> {
    let field = if manifest.scenario.content_sha256 != request.provenance.content_sha256 {
        Some("scenario source content hash")
    } else if manifest.seed != seed {
        Some("seed")
    } else if manifest.ticks != request.ticks {
        Some("tick count")
    } else if manifest.step_s != request.step_s {
        Some("fixed step")
    } else if manifest.sampling != request.sampling {
        Some("sampling policy")
    } else {
        None
    };
    match field {
        None => Ok(()),
        Some(field) => Err(BatchError::Mismatch {
            path: directory.to_path_buf(),
            seed,
            field,
        }),
    }
}

/// Execute the pending runs, `jobs` at a time, and return their records.
///
/// Each worker takes one whole run at a time, so the kernel tick is never
/// parallelized and every run's `Simulation` is private to its worker. The
/// returned records are ordered by ascending seed; a failure is reported for
/// the lowest seed so a batch's error is deterministic.
fn execute(
    request: &BatchRequest,
    pending: Vec<PendingRun>,
) -> Result<Vec<(usize, BatchRun)>, BatchError> {
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = usize::try_from(request.jobs)
        .unwrap_or(usize::MAX)
        .clamp(1, pending.len());
    let queue = Mutex::new(pending);
    let outcomes = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| drain_queue(request, &queue, &outcomes));
        }
    });

    let mut completed = Vec::new();
    let mut failures = Vec::new();
    for (index, outcome) in outcomes.into_inner().expect("no run worker panicked") {
        match outcome {
            Ok(record) => completed.push((index, record)),
            Err(error) => failures.push((index, error)),
        }
    }
    if let Some((_, error)) = failures.into_iter().min_by_key(|(index, _)| *index) {
        return Err(error);
    }
    completed.sort_by_key(|(index, _)| *index);
    Ok(completed)
}

/// One worker: take pending runs until the queue is empty.
fn drain_queue(
    request: &BatchRequest,
    queue: &Mutex<Vec<PendingRun>>,
    outcomes: &Mutex<Vec<(usize, Result<BatchRun, BatchError>)>>,
) {
    loop {
        let Some(run) = queue
            .lock()
            .expect("the batch work queue is never poisoned")
            .pop()
        else {
            return;
        };
        let outcome = execute_run(request, run.seed);
        outcomes
            .lock()
            .expect("the batch outcome list is never poisoned")
            .push((run.index, outcome));
    }
}

/// Run one seed into its directory and read the artifact back.
fn execute_run(request: &BatchRequest, seed: u64) -> Result<BatchRun, BatchError> {
    let directory = run_directory_path(&request.root, seed);
    clear_partial_run(&directory)?;
    let (trace, summary, trajectories, metrics) = canonical_run_captured(
        request.scenario.clone(),
        RunConfig::new(seed),
        request.ticks,
        &request.sampling.trajectories,
    )
    .map_err(|source| BatchError::Init { seed, source })?;
    write_run_directory(
        &directory,
        RunDirectoryRequest {
            scenario: &request.provenance,
            seed,
            step_s: request.step_s,
            sampling: request.sampling,
            trace: &trace,
            trajectories: &trajectories,
            summary: &summary,
            metrics: &metrics,
        },
    )?;
    read_run(&directory, seed)
}

/// Clear a partial run directory so the writer can create it from scratch.
///
/// Only a directory the inspection step found without the completion marker
/// reaches here, so a completed run directory is never removed.
fn clear_partial_run(directory: &Path) -> Result<(), BatchError> {
    match fs::remove_dir_all(directory) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(BatchError::Io {
            path: directory.to_path_buf(),
            action: "clear a partial run directory",
            source,
        }),
    }
}

/// Read a run directory's record from the artifacts on disk.
///
/// Both a fresh run and a resumed one go through this path, so the recorded
/// trace and manifest hashes always describe the bytes that are actually
/// present, not the values an in-memory writer intended.
fn read_run(directory: &Path, seed: u64) -> Result<BatchRun, BatchError> {
    let manifest = read_run_manifest(directory, seed)?;
    let bytes = fs::read(directory.join(MANIFEST_FILE)).map_err(|source| BatchError::Io {
        path: directory.join(MANIFEST_FILE),
        action: "read",
        source,
    })?;
    Ok(BatchRun {
        seed,
        directory: run_directory_name(seed),
        trace_sha256: manifest.stream.uncompressed_sha256,
        manifest_sha256: sha256_hex(&bytes),
    })
}

/// Read and parse a run directory's manifest.
fn read_run_manifest(directory: &Path, seed: u64) -> Result<RunManifest, BatchError> {
    let path = directory.join(MANIFEST_FILE);
    let json = fs::read_to_string(&path).map_err(|source| BatchError::Io {
        path: path.clone(),
        action: "read",
        source,
    })?;
    serde_json::from_str(&json).map_err(|source| BatchError::Manifest {
        path: directory.to_path_buf(),
        seed,
        source,
    })
}

/// Write `batch.json` into the output root.
fn write_batch_manifest(root: &Path, manifest: &BatchManifest) -> Result<(), BatchError> {
    let mut json = serde_json::to_string_pretty(manifest).expect("batch manifest serializes");
    json.push('\n');
    let path = root.join(BATCH_MANIFEST_FILE);
    fs::write(&path, json).map_err(|source| BatchError::Io {
        path,
        action: "write",
        source,
    })
}

/// The directory one seed's run occupies inside the batch root.
fn run_directory_path(root: &Path, seed: u64) -> PathBuf {
    root.join(run_directory_name(seed))
}

/// The name of a seed's run directory, relative to the batch root.
fn run_directory_name(seed: u64) -> String {
    format!("{RUN_DIRECTORY_PREFIX}{seed}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_are_sorted_and_deduplicated() {
        assert_eq!(
            normalize_seeds(&[2, 0, 1, 2, 0]).expect("seeds normalize"),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn an_empty_seed_list_is_rejected() {
        assert!(matches!(normalize_seeds(&[]), Err(BatchError::NoSeeds)));
    }

    /// A run directory name is a pure function of the seed, so a resumed batch
    /// addresses the same directory a fresh one wrote.
    #[test]
    fn a_run_directory_name_names_the_seed() {
        assert_eq!(run_directory_name(0), "seed-0");
        assert_eq!(run_directory_name(42), "seed-42");
    }
}
