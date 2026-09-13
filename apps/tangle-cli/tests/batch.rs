//! Contract tests for the resumable, optionally parallel `batch` command.
//!
//! A batch is the durable experiment record: one immutable run directory per
//! seed plus a `batch.json` linking them. These tests pin the three properties
//! that make it dependable — the manifest orders runs by ascending seed and
//! links each run to its run manifest; a stopped batch resumes from the
//! artifacts on disk without mutating a completed run while completing an
//! incomplete one; and a parallel batch produces exactly the bytes a serial
//! batch produces.
//!
//! The resume test compares content hashes before and after, and the
//! parallel-equals-serial test compares the whole batch manifest and every
//! decompressed event stream, so a difference in completion order or worker
//! count fails here rather than passing because the test reimplemented the
//! writer.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tangle_cli::{
    BATCH_MANIFEST_FILE, BATCH_MANIFEST_VERSION, BatchError, BatchManifest, BatchRequest,
    EVENT_STREAM_FILE, MANIFEST_FILE, METRICS_FILE, RunManifest, SUMMARY_FILE, SamplingPolicy,
    ScenarioProvenance, TRAJECTORY_FILE, load_scenario_hashed, run_batch,
};
use tangle_model::CompiledScenario;
use tangle_sim::{EVENT_VERSION, RunConfig};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
/// Few enough ticks that a multi-seed batch stays fast; every run still
/// produces events and sampled trajectories.
const TICKS: u64 = 60;

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tangle-cli-batch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// Load the checked-in walking scenario and its source content hash.
fn walking() -> (CompiledScenario, String) {
    load_scenario_hashed(&repo_path(WALKING)).expect("walking scenario loads")
}

fn provenance(content_sha256: &str) -> ScenarioProvenance {
    ScenarioProvenance {
        id: "walking_guide_v1".to_owned(),
        source_path: WALKING.to_owned(),
        schema_version: 1,
        content_sha256: content_sha256.to_owned(),
    }
}

/// The request a test hands to [`run_batch`].
fn request(root: &Path, seeds: &[u64], ticks: u64, jobs: u64) -> BatchRequest {
    let (scenario, content_sha256) = walking();
    BatchRequest {
        root: root.to_path_buf(),
        scenario,
        provenance: provenance(&content_sha256),
        ticks,
        step_s: RunConfig::new(0).step().as_secs(),
        sampling: SamplingPolicy::default(),
        seeds: seeds.to_vec(),
        seed_bank: None,
        jobs,
    }
}

/// Sorted file names a directory holds.
fn entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", directory.display()))
        .map(|entry| {
            entry
                .expect("directory entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// SHA-256 of a file's exact bytes.
fn file_sha256(path: &Path) -> String {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()));
    format!("{:x}", Sha256::digest(&bytes))
}

/// The content hash of every file a directory holds, sorted by file name.
fn content_hashes(directory: &Path) -> Vec<(String, String)> {
    entries(directory)
        .into_iter()
        .map(|name| {
            let hash = file_sha256(&directory.join(&name));
            (name, hash)
        })
        .collect()
}

fn read_batch_manifest(root: &Path) -> BatchManifest {
    let json = std::fs::read_to_string(root.join(BATCH_MANIFEST_FILE)).expect("batch.json is read");
    serde_json::from_str(&json).expect("batch.json is JSON")
}

fn read_run_manifest(directory: &Path) -> RunManifest {
    let json = std::fs::read_to_string(directory.join(MANIFEST_FILE)).expect("manifest is written");
    serde_json::from_str(&json).expect("manifest is JSON")
}

/// The canonical event stream, decompressed from a run directory.
fn decompressed_stream(directory: &Path) -> Vec<u8> {
    let compressed =
        std::fs::read(directory.join(EVENT_STREAM_FILE)).expect("event stream is written");
    let mut decoded = Vec::new();
    GzDecoder::new(&compressed[..])
        .read_to_end(&mut decoded)
        .expect("event stream decompresses");
    decoded
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Run the `batch` command with `--jobs jobs` into `out_root`.
fn batch_command(scratch: &Scratch, out_root: &str, seeds: &str, jobs: &str) -> Output {
    let ticks = TICKS.to_string();
    Command::new(CLI)
        .arg("batch")
        .arg(repo_path(WALKING))
        .args(["--seeds", seeds])
        .args(["--ticks", &ticks])
        .args(["--out-root", out_root])
        .args(["--jobs", jobs])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs")
}

/// The four artifacts every completed run directory holds.
fn run_artifacts() -> Vec<String> {
    vec![
        EVENT_STREAM_FILE.to_owned(),
        MANIFEST_FILE.to_owned(),
        METRICS_FILE.to_owned(),
        SUMMARY_FILE.to_owned(),
        TRAJECTORY_FILE.to_owned(),
    ]
}

/// `batch --seeds 2,0,1` writes one run directory per seed and a manifest whose
/// runs are ordered by ascending seed, not by the order the seeds were given.
#[test]
fn batch_writes_a_run_directory_per_seed_and_an_ascending_manifest() {
    let scratch = Scratch::new("layout");
    let out_root = scratch.path("batch");

    let output = batch_command(&scratch, "batch", "2,0,1", "1");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "", "a batch prints no report on stdout");
    assert!(
        stderr(&output).contains("batch manifest:"),
        "the batch must report where its manifest is: {}",
        stderr(&output)
    );

    let (_, content_sha256) = walking();
    let manifest = read_batch_manifest(&out_root);
    assert_eq!(manifest.batch_manifest_version, BATCH_MANIFEST_VERSION);
    assert_eq!(manifest.spec.scenario.id, "walking_guide_v1");
    assert_eq!(manifest.spec.scenario.content_sha256, content_sha256);
    assert_eq!(manifest.spec.ticks, TICKS);
    assert_eq!(manifest.spec.fidelity, "standard");
    assert_eq!(manifest.spec.step_s, 0.05);
    assert_eq!(manifest.spec.event_version, EVENT_VERSION);
    assert_eq!(manifest.spec.sampling, SamplingPolicy::default());

    // Ascending seed order, independent of the order the seeds were given.
    assert_eq!(manifest.seeds, vec![0, 1, 2]);
    let ordered: Vec<u64> = manifest.runs.iter().map(|run| run.seed).collect();
    assert_eq!(ordered, vec![0, 1, 2]);
    for run in &manifest.runs {
        assert_eq!(run.directory, format!("seed-{}", run.seed));
        let directory = out_root.join(&run.directory);
        assert_eq!(entries(&directory), run_artifacts());
    }
}

/// The batch manifest links every run to its run manifest: the trace hash is
/// the run manifest's own stream hash, and the manifest hash is the summary's
/// link back to the same bytes.
#[test]
fn the_batch_manifest_links_each_run_to_its_run_manifest() {
    let scratch = Scratch::new("links");
    let out_root = scratch.path("batch");

    let output = batch_command(&scratch, "batch", "0,1", "2");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let manifest = read_batch_manifest(&out_root);
    for run in &manifest.runs {
        let directory = out_root.join(&run.directory);
        let run_manifest = read_run_manifest(&directory);
        assert_eq!(run_manifest.seed, run.seed);
        assert_eq!(
            run.trace_sha256, run_manifest.stream.uncompressed_sha256,
            "the batch must name the run's own trace hash"
        );
        let summary: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(directory.join(SUMMARY_FILE)).expect("summary is written"),
        )
        .expect("summary is JSON");
        assert_eq!(
            run.manifest_sha256,
            file_sha256(&directory.join(MANIFEST_FILE)),
            "the batch must name the run manifest's exact bytes"
        );
        assert_eq!(
            summary["manifest_sha256"].as_str(),
            Some(run.manifest_sha256.as_str()),
            "the run summary must trace back to the same manifest"
        );
    }
}

/// A stopped batch resumes from the artifacts on disk: completed runs are
/// skipped and their bytes are untouched, while a run directory missing the
/// completion marker is re-run and completed.
#[test]
fn resume_skips_completed_runs_and_completes_an_incomplete_one() {
    let scratch = Scratch::new("resume");
    let out_root = scratch.path("batch");

    let first = run_batch(request(&out_root, &[0, 1, 2], TICKS, 2)).expect("batch runs");
    let batch_bytes = std::fs::read(out_root.join(BATCH_MANIFEST_FILE)).expect("batch.json");
    let before: Vec<Vec<(String, String)>> = first
        .runs
        .iter()
        .map(|run| content_hashes(&out_root.join(&run.directory)))
        .collect();

    // Simulate an interrupted run: its directory keeps the artifacts written
    // before the completion marker but loses the marker itself.
    let interrupted = out_root.join(&first.runs[1].directory);
    std::fs::remove_file(interrupted.join(MANIFEST_FILE)).expect("the marker is removed");
    assert!(
        !interrupted.join(MANIFEST_FILE).exists(),
        "the interrupted run must lack its completion marker"
    );

    let second = run_batch(request(&out_root, &[0, 1, 2], TICKS, 2)).expect("batch resumes");

    assert_eq!(second, first, "resume must reproduce the same manifest");
    assert_eq!(
        std::fs::read(out_root.join(BATCH_MANIFEST_FILE)).expect("batch.json"),
        batch_bytes,
        "resuming rewrote batch.json"
    );
    for (index, run) in second.runs.iter().enumerate() {
        assert_eq!(
            content_hashes(&out_root.join(&run.directory)),
            before[index],
            "resume changed the bytes of run seed {}",
            run.seed
        );
    }
    assert!(
        interrupted.join(MANIFEST_FILE).exists(),
        "the incomplete run must be completed"
    );
}

/// A completed run directory that does not match the specification is refused
/// rather than recorded under the wrong provenance, and nothing is mutated.
#[test]
fn a_completed_run_that_disagrees_with_the_batch_is_refused() {
    let scratch = Scratch::new("mismatch");
    let out_root = scratch.path("batch");

    let manifest = run_batch(request(&out_root, &[0], TICKS, 1)).expect("batch runs");
    let directory = out_root.join(&manifest.runs[0].directory);
    let before = content_hashes(&directory);
    let batch_bytes = std::fs::read(out_root.join(BATCH_MANIFEST_FILE)).expect("batch.json");

    let error = run_batch(request(&out_root, &[0], TICKS + 1, 1))
        .expect_err("a mismatched rerun is refused");
    assert!(
        matches!(
            &error,
            BatchError::Mismatch { seed: 0, field, .. } if *field == "tick count"
        ),
        "unexpected error: {error}"
    );
    assert_eq!(
        content_hashes(&directory),
        before,
        "the refused rerun mutated a completed run"
    );
    assert_eq!(
        std::fs::read(out_root.join(BATCH_MANIFEST_FILE)).expect("batch.json"),
        batch_bytes,
        "the refused rerun rewrote batch.json"
    );
}

/// Parallel and serial execution agree: `--jobs 4` and `--jobs 1` write the
/// same batch manifest and byte-identical per-run event streams and trace
/// hashes, so worker count cannot change an experiment's output.
#[test]
fn parallel_and_serial_batches_produce_identical_trace_hashes_and_streams() {
    let scratch = Scratch::new("parallel");
    let serial = scratch.path("serial");
    let parallel = scratch.path("parallel");

    let serial_output = batch_command(&scratch, "serial", "0,1,2,3", "1");
    assert_eq!(
        serial_output.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&serial_output)
    );
    let parallel_output = batch_command(&scratch, "parallel", "0,1,2,3", "4");
    assert_eq!(
        parallel_output.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&parallel_output)
    );

    assert_eq!(
        std::fs::read(serial.join(BATCH_MANIFEST_FILE)).expect("serial batch.json"),
        std::fs::read(parallel.join(BATCH_MANIFEST_FILE)).expect("parallel batch.json"),
        "the batch manifest must not depend on --jobs"
    );

    let serial_manifest = read_batch_manifest(&serial);
    let parallel_manifest = read_batch_manifest(&parallel);
    assert_eq!(serial_manifest.runs.len(), 4);
    for (serial_run, parallel_run) in serial_manifest.runs.iter().zip(&parallel_manifest.runs) {
        assert_eq!(serial_run.seed, parallel_run.seed);
        assert_eq!(
            serial_run.trace_sha256, parallel_run.trace_sha256,
            "seed {} has a different trace hash under --jobs 4",
            serial_run.seed
        );
        let serial_dir = serial.join(&serial_run.directory);
        let parallel_dir = parallel.join(&parallel_run.directory);
        assert_eq!(
            decompressed_stream(&serial_dir),
            decompressed_stream(&parallel_dir),
            "seed {} has a different canonical event stream under --jobs 4",
            serial_run.seed
        );
    }
}

/// `--jobs` counts whole runs, so zero is a usage error rather than a batch
/// that silently runs nothing.
#[test]
fn zero_jobs_is_a_usage_error() {
    let scratch = Scratch::new("zero-jobs");

    let output = batch_command(&scratch, "batch", "0", "0");

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(
        !scratch.path("batch").exists(),
        "a usage error wrote a batch root"
    );
}
