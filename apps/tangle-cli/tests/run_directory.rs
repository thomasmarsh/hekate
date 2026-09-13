//! Contract tests for the immutable run directory `run` writes.
//!
//! The library tests cover the on-disk contract — manifest provenance, the
//! compressed event stream's round-trip to the canonical trace, the summary's
//! tie to the manifest, and immutability — because that is what a `batch`,
//! `replay`, or aggregation consumer reads. The command tests cover the wiring:
//! `--run-dir` adds the directory without changing what `run` already writes.
//!
//! The stream is the canonical trace's exact bytes, so these tests compare it
//! against `tests/golden/walking_guide_v1.trace.jsonl` and its hash golden. A
//! failure there means canonical serialization changed, not that a fixture is
//! stale.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tangle_cli::{
    DEFAULT_MAX_TRAJECTORY_SAMPLES, DEFAULT_TRAJECTORY_STRIDE_TICKS, EVENT_STREAM_COMPRESSION,
    EVENT_STREAM_FILE, EventRetention, MANIFEST_FILE, RUN_MANIFEST_VERSION, RUN_SUMMARY_VERSION,
    RunDirectoryError, RunDirectoryRequest, RunManifest, RunSummary, SAMPLING_POLICY_VERSION,
    SUMMARY_FILE, ScenarioProvenance, TrajectoryRetention, canonical_run, load_scenario_hashed,
    write_run_directory,
};
use tangle_sim::{EVENT_VERSION, RunConfig};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
const GOLDEN_SEED: u64 = 0;
const GOLDEN_TICKS: u64 = 250;

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tangle-cli-run-dir-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory is created");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Sorted entries the scratch directory holds.
    fn entries(&self) -> Vec<String> {
        entries(&self.dir)
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
    let mut hashes: Vec<(String, String)> = entries(directory)
        .into_iter()
        .map(|name| {
            let hash = file_sha256(&directory.join(&name));
            (name, hash)
        })
        .collect();
    hashes.sort();
    hashes
}

fn golden(extension: &str) -> String {
    let path = repo_path(&format!("tests/golden/walking_guide_v1.trace.{extension}"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read golden file '{}': {error}", path.display()))
}

/// Load the checked-in walking scenario and its source content hash.
fn walking() -> (tangle_model::CompiledScenario, String) {
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

fn request<'a>(
    scenario: &'a ScenarioProvenance,
    trace: &'a tangle_cli::Trace,
    summary: &'a tangle_sim::RunSummary,
) -> RunDirectoryRequest<'a> {
    RunDirectoryRequest {
        scenario,
        seed: GOLDEN_SEED,
        step_s: RunConfig::new(GOLDEN_SEED).step().as_secs(),
        trace,
        summary,
    }
}

/// Write the golden walking run into `directory` and return the run directory.
fn write_golden_run(directory: &Path) {
    let (scenario, content_sha256) = walking();
    let provenance = provenance(&content_sha256);
    let (trace, summary) =
        canonical_run(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS).expect("run completes");
    write_run_directory(directory, request(&provenance, &trace, &summary))
        .expect("run directory is written");
}

fn read_manifest(directory: &Path) -> RunManifest {
    let json = std::fs::read_to_string(directory.join(MANIFEST_FILE)).expect("manifest is written");
    serde_json::from_str(&json).expect("manifest is JSON")
}

fn read_summary(directory: &Path) -> RunSummary {
    let json = std::fs::read_to_string(directory.join(SUMMARY_FILE)).expect("summary is written");
    serde_json::from_str(&json).expect("summary is JSON")
}

/// The canonical event stream, decompressed from the run directory.
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

#[test]
fn run_writes_the_three_artifacts_and_no_trajectory_stream() {
    let scratch = Scratch::new("artifacts");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir);

    // Full trajectories stay opt-in, so the directory holds exactly the
    // manifest, the summary, and the sparse event stream.
    assert_eq!(
        entries(&run_dir),
        vec![
            EVENT_STREAM_FILE.to_owned(),
            MANIFEST_FILE.to_owned(),
            SUMMARY_FILE.to_owned()
        ]
    );
}

/// The manifest is the provenance a rerun needs: what was run, from which
/// bytes, at which seed, step, fidelity, and schema versions, and under which
/// sampling policy.
#[test]
fn manifest_records_the_provenance_a_rerun_needs() {
    let scratch = Scratch::new("manifest");
    let run_dir = scratch.path("run");
    let (_, content_sha256) = walking();

    write_golden_run(&run_dir);
    let manifest = read_manifest(&run_dir);

    assert_eq!(manifest.manifest_version, RUN_MANIFEST_VERSION);
    assert_eq!(manifest.scenario.id, "walking_guide_v1");
    assert_eq!(manifest.scenario.source_path, WALKING);
    assert_eq!(manifest.scenario.schema_version, 1);
    // The recorded hash is the loader's own content hash of the authored bytes,
    // so a consumer can verify the exact source it must rerun.
    assert_eq!(manifest.scenario.content_sha256, content_sha256);
    assert_eq!(manifest.scenario.content_sha256.len(), 64);

    assert_eq!(manifest.seed, GOLDEN_SEED);
    assert_eq!(manifest.ticks, GOLDEN_TICKS);
    assert_eq!(manifest.fidelity, "standard");
    assert_eq!(manifest.step_s, 0.05);
    assert_eq!(manifest.event_version, EVENT_VERSION);
    assert_eq!(manifest.model_version, tangle_model::MODEL_VERSION);
    assert_eq!(manifest.build_revision, env!("CARGO_PKG_VERSION"));
    assert!(!manifest.build_revision.is_empty());

    assert_eq!(manifest.sampling.policy_version, SAMPLING_POLICY_VERSION);
    assert_eq!(manifest.sampling.events, EventRetention::All);
    assert_eq!(
        manifest.sampling.trajectories.retention,
        TrajectoryRetention::Off
    );
    assert_eq!(
        manifest.sampling.trajectories.stride_ticks,
        DEFAULT_TRAJECTORY_STRIDE_TICKS
    );
    assert_eq!(
        manifest.sampling.trajectories.max_samples,
        DEFAULT_MAX_TRAJECTORY_SAMPLES
    );

    assert_eq!(manifest.stream.path, EVENT_STREAM_FILE);
    assert_eq!(manifest.stream.compression, EVENT_STREAM_COMPRESSION);
    assert_eq!(manifest.stream.records, 14);
    assert_eq!(manifest.stream.uncompressed_bytes, 1317);

    // Run artifacts are pretty JSON with a trailing newline.
    let json = std::fs::read_to_string(run_dir.join(MANIFEST_FILE)).expect("manifest is written");
    assert!(json.ends_with("}\n"), "manifest lacks a trailing newline");
}

#[test]
fn event_stream_round_trips_to_the_canonical_records() {
    let scratch = Scratch::new("stream");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir);

    let (scenario, _) = walking();
    let expected = canonical_run(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS)
        .expect("run completes")
        .0;

    let decoded = decompressed_stream(&run_dir);
    assert_eq!(
        decoded,
        expected.bytes(),
        "the compressed stream must hold the canonical trace bytes"
    );

    let text = String::from_utf8(decoded).expect("the stream is UTF-8");
    assert_eq!(
        text,
        golden("jsonl"),
        "the run directory's stream disagrees with the checked-in trace golden"
    );
    // One typed record per line, in canonical order, and every line is JSON.
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 14);
    assert_eq!(
        lines[0],
        r#"{"kind":"run","scenario_id":"walking_guide_v1","schema_version":1,"event_version":2,"seed":0,"step_s":0.05,"ticks":250}"#
    );
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).expect("each record is JSON");
    }

    let manifest = read_manifest(&run_dir);
    assert_eq!(manifest.stream.records, lines.len() as u64);
    assert_eq!(manifest.stream.uncompressed_sha256, expected.hash());
    assert_eq!(
        manifest.stream.uncompressed_sha256,
        golden("sha256").trim(),
        "the run directory's stream hash disagrees with the checked-in hash golden"
    );
}

#[test]
fn summary_reports_the_run_and_traces_back_to_the_manifest() {
    let scratch = Scratch::new("summary");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir);

    let (scenario, _) = walking();
    let (_, kernel_summary) =
        canonical_run(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS).expect("run completes");
    let summary = read_summary(&run_dir);

    assert_eq!(summary.summary_version, RUN_SUMMARY_VERSION);
    assert_eq!(summary.ticks, kernel_summary.ticks());
    assert_eq!(summary.spawned, kernel_summary.spawned());
    assert_eq!(summary.despawned, kernel_summary.despawned());
    assert_eq!(summary.remaining, kernel_summary.remaining());
    // 250 fixed steps at the Standard 50 ms step.
    assert_eq!(summary.elapsed_s, 12.5);
    assert_eq!(
        summary.manifest_sha256,
        file_sha256(&run_dir.join(MANIFEST_FILE)),
        "every summary value must be traceable to the manifest it names"
    );
    assert_eq!(summary.manifest_sha256.len(), 64);
}

/// A completed run directory is immutable: the second write fails and leaves
/// every artifact byte-identical.
#[test]
fn a_completed_run_directory_is_never_rewritten() {
    let scratch = Scratch::new("immutable");
    let run_dir = scratch.path("run");

    write_golden_run(&run_dir);
    let before = content_hashes(&run_dir);

    let (scenario, content_sha256) = walking();
    let provenance = provenance(&content_sha256);
    let (trace, summary) =
        canonical_run(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS).expect("run completes");
    let error = write_run_directory(&run_dir, request(&provenance, &trace, &summary))
        .expect_err("a completed run directory rejects a rerun");

    assert!(
        matches!(&error, RunDirectoryError::Completed { path } if path == &run_dir),
        "unexpected error: {error}"
    );
    assert_eq!(
        content_hashes(&run_dir),
        before,
        "the rejected rerun mutated a completed artifact"
    );
}

/// A directory that holds anything but a completed run is not clobbered either:
/// the writer refuses rather than guessing what the content is.
#[test]
fn a_run_directory_holding_foreign_content_is_not_clobbered() {
    let scratch = Scratch::new("foreign");
    let run_dir = scratch.path("run");
    std::fs::create_dir_all(&run_dir).expect("directory is created");
    std::fs::write(run_dir.join("notes.txt"), "hand-written\n").expect("file is written");

    let (scenario, content_sha256) = walking();
    let provenance = provenance(&content_sha256);
    let (trace, summary) =
        canonical_run(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS).expect("run completes");
    let error = write_run_directory(&run_dir, request(&provenance, &trace, &summary))
        .expect_err("a non-empty directory rejects the write");

    assert!(
        matches!(&error, RunDirectoryError::NotEmpty { path } if path == &run_dir),
        "unexpected error: {error}"
    );
    assert_eq!(entries(&run_dir), vec!["notes.txt".to_owned()]);
}

/// Nothing in a run directory depends on the clock or the environment, so the
/// same run reproduces every artifact byte for byte.
#[test]
fn run_directories_are_reproducible_byte_for_byte() {
    let scratch = Scratch::new("reproducible");
    let first = scratch.path("first");
    let second = scratch.path("second");

    write_golden_run(&first);
    write_golden_run(&second);

    assert_eq!(entries(&first), entries(&second));
    for name in entries(&first) {
        assert_eq!(
            std::fs::read(first.join(&name)).expect("artifact is readable"),
            std::fs::read(second.join(&name)).expect("artifact is readable"),
            "run directory artifact '{name}' is not reproducible"
        );
    }
}

#[test]
fn run_writes_the_run_directory_and_keeps_the_trace_output() {
    let scratch = Scratch::new("cli-additive");
    let scenario = repo_path(WALKING);

    let output = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args(["--seed", "0", "--ticks", "250"])
        .args(["--output", "trace.jsonl", "--hash-file", "trace.sha256"])
        .args(["--run-dir", "run"])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "", "a file destination prints no trace");
    assert_eq!(
        stderr(&output),
        format!("trace hash: {}\n", golden("sha256").trim())
    );

    // The existing outputs are unchanged.
    let trace = std::fs::read_to_string(scratch.path("trace.jsonl")).expect("trace is written");
    assert_eq!(trace, golden("jsonl"));
    let hash = std::fs::read_to_string(scratch.path("trace.sha256")).expect("hash is written");
    assert_eq!(hash, format!("{}\n", golden("sha256").trim()));

    // The added directory holds the same run, and its manifest names the source
    // path exactly as the command received it.
    let run_dir = scratch.path("run");
    assert_eq!(
        entries(&run_dir),
        vec![
            EVENT_STREAM_FILE.to_owned(),
            MANIFEST_FILE.to_owned(),
            SUMMARY_FILE.to_owned()
        ]
    );
    let manifest = read_manifest(&run_dir);
    assert_eq!(
        manifest.scenario.source_path,
        scenario.to_string_lossy().into_owned()
    );
    assert_eq!(
        String::from_utf8(decompressed_stream(&run_dir)).unwrap(),
        trace
    );
}

/// A repeat run against a completed directory fails before writing anything
/// else, so the completed artifacts and the caller's destinations are untouched.
#[test]
fn a_repeat_run_against_a_completed_directory_fails_without_mutating_it() {
    let scratch = Scratch::new("cli-immutable");
    let scenario = repo_path(WALKING);

    let first = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args([
            "--ticks",
            "250",
            "--output",
            "trace.jsonl",
            "--run-dir",
            "run",
        ])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");
    assert_eq!(first.status.code(), Some(0), "stderr: {}", stderr(&first));

    let run_dir = scratch.path("run");
    let before = content_hashes(&run_dir);
    let trace_before = file_sha256(&scratch.path("trace.jsonl"));

    // A different run that targets the same completed directory: it fails, and
    // it writes neither the directory nor its own trace destination.
    let second = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args([
            "--ticks",
            "5",
            "--output",
            "trace2.jsonl",
            "--run-dir",
            "run",
        ])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");

    assert_eq!(second.status.code(), Some(1));
    assert_eq!(stdout(&second), "");
    let message = stderr(&second);
    assert!(
        message.contains("already holds a completed run"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("run"),
        "the message must name the directory: {message}"
    );

    assert_eq!(
        content_hashes(&run_dir),
        before,
        "the rejected rerun mutated a completed artifact"
    );
    assert_eq!(file_sha256(&scratch.path("trace.jsonl")), trace_before);
    assert!(
        !scratch.path("trace2.jsonl").exists(),
        "the rejected rerun still wrote its trace destination"
    );
}

/// Without `--run-dir`, `run` writes exactly what it always wrote.
#[test]
fn run_without_a_run_directory_writes_only_the_trace() {
    let scratch = Scratch::new("cli-default");
    let scenario = repo_path(WALKING);

    let output = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args(["--seed", "0", "--ticks", "250", "--output", "trace.jsonl"])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        scratch.entries(),
        vec!["trace.jsonl".to_owned()],
        "a default run added an artifact"
    );
    assert_eq!(
        std::fs::read_to_string(scratch.path("trace.jsonl")).expect("trace is written"),
        golden("jsonl")
    );
    assert_eq!(
        stderr(&output),
        format!("trace hash: {}\n", golden("sha256").trim())
    );
}
