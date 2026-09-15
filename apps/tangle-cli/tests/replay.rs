//! Contract tests for the read-only `replay` command.
//!
//! A completed run directory is the durable record of one run, and `replay` is
//! what proves the record still reproduces: it re-loads the recorded scenario
//! source, re-runs the kernel with the recorded parameters, and emits the
//! canonical event stream. These tests pin the command's whole contract — the
//! reproduction equals the recorded stream and trace hash; verification fails
//! non-zero on a tampered artifact, a manifest whose recorded hash disagrees,
//! and a scenario source edited after the run; a missing or interrupted run
//! directory fails; and replay writes nothing, so a run directory's bytes are
//! untouched by either a successful or a refused replay.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use tangle_cli::{EVENT_STREAM_FILE, MANIFEST_FILE, RunManifest, read_trajectories};
use tangle_model::{MovementDirection, PermissionEffect};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

const WALKING: &str = "scenarios/walking/walking_guide_v1.json5";
const GOLDEN_SEED: u64 = 0;
const GOLDEN_TICKS: u64 = 250;

/// A version-2 fixture whose rider enters at the reference end and travels the
/// facility's reverse traversal, against its authored forward nominal direction,
/// with a `nominal_direction` `permit` statement binding the pair. The run's
/// sampled trajectories therefore carry the wrong-way rule state, which replay
/// must leave exactly as recorded.
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

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tangle-cli-replay-{name}-{}", std::process::id()));
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
    entries(directory)
        .into_iter()
        .map(|name| {
            let hash = file_sha256(&directory.join(&name));
            (name, hash)
        })
        .collect()
}

fn golden(extension: &str) -> String {
    let path = repo_path(&format!("tests/golden/walking_guide_v1.trace.{extension}"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read golden file '{}': {error}", path.display()))
}

fn read_manifest(directory: &Path) -> RunManifest {
    let json = std::fs::read_to_string(directory.join(MANIFEST_FILE)).expect("manifest is written");
    serde_json::from_str(&json).expect("manifest is JSON")
}

/// Rewrite a run directory's manifest from its parsed JSON, as a tamper would.
fn write_manifest_json(directory: &Path, manifest: &serde_json::Value) {
    let mut json = serde_json::to_string_pretty(manifest).expect("manifest serializes");
    json.push('\n');
    std::fs::write(directory.join(MANIFEST_FILE), json).expect("manifest is written");
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

/// Gzip `bytes` with the fixed header the run directory writer uses.
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .expect("writing to memory cannot fail");
    encoder.finish().expect("writing to memory cannot fail")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Record the golden walking run against `scenario` into `<scratch>/run` and
/// return that run directory.
///
/// `scenario` is passed absolute so the manifest records an absolute source
/// path that replay resolves wherever the command runs.
fn record_run(scratch: &Scratch, scenario: &Path) -> PathBuf {
    let seed = GOLDEN_SEED.to_string();
    let ticks = GOLDEN_TICKS.to_string();
    let output = Command::new(CLI)
        .arg("run")
        .arg(scenario)
        .args(["--seed", &seed, "--ticks", &ticks])
        .args(["--output", "trace.jsonl", "--run-dir", "run"])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    scratch.path("run")
}

/// Run `replay <run-dir> <flags>` from the scratch directory.
fn replay(scratch: &Scratch, run_dir: &str, flags: &[&str]) -> Output {
    Command::new(CLI)
        .arg("replay")
        .arg(run_dir)
        .args(flags)
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs")
}

/// Replaying a healthy run directory emits the recorded canonical event stream
/// unchanged and adds nothing to the filesystem.
#[test]
fn replay_reproduces_the_recorded_stream_and_writes_no_artifact() {
    let scratch = Scratch::new("reproduces");
    let run_dir = record_run(&scratch, &repo_path(WALKING));

    let entries_before = scratch.entries();
    let artifacts_before = content_hashes(&run_dir);

    let output = replay(&scratch, "run", &[]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        golden("jsonl"),
        "replay must emit the canonical event stream"
    );
    assert_eq!(
        stdout(&output),
        String::from_utf8(decompressed_stream(&run_dir)).expect("the recorded stream is UTF-8"),
        "the reproduced stream must equal the recorded stream"
    );
    assert_eq!(
        stderr(&output),
        format!("replay trace hash: {}\n", golden("sha256").trim())
    );

    // Read-only: no artifact was added and no recorded byte changed.
    assert_eq!(
        scratch.entries(),
        entries_before,
        "replay wrote an artifact"
    );
    assert_eq!(
        content_hashes(&run_dir),
        artifacts_before,
        "replay mutated the run directory"
    );
}

/// A run directory whose trajectories carry the wrong-way rule state replays
/// with verification, and replay leaves every recorded byte — the rule-state
/// rows included — exactly as the run wrote them.
#[test]
fn replay_keeps_the_recorded_rule_state_and_touches_nothing() {
    let scratch = Scratch::new("rule-state");
    let scenario = scratch.path("opposing.json5");
    std::fs::write(&scenario, OPPOSING_V2).expect("the fixture is written");
    let seed = GOLDEN_SEED.to_string();
    let output = Command::new(CLI)
        .arg("run")
        .arg(&scenario)
        .args(["--seed", &seed, "--ticks", "400"])
        .args(["--output", "trace.jsonl", "--run-dir", "run"])
        .arg("--full-trajectories")
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    let run_dir = scratch.path("run");
    let samples = read_trajectories(&run_dir).expect("trajectories read back");
    let opposing: Vec<_> = samples
        .iter()
        .filter(|sample| sample.opposing_direction.is_some())
        .collect();
    assert!(
        !opposing.is_empty(),
        "the recorded run must carry the wrong-way rule state"
    );
    for sample in &opposing {
        assert_eq!(sample.opposing_direction, Some(MovementDirection::Reverse));
        assert_eq!(sample.perceived_rule, Some(PermissionEffect::Permit));
    }

    let before = content_hashes(&run_dir);
    let verified = replay(&scratch, "run", &["--verify"]);

    assert_eq!(
        verified.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&verified)
    );
    assert!(
        stderr(&verified).contains("replay verified:"),
        "unexpected message: {}",
        stderr(&verified)
    );
    assert_eq!(
        content_hashes(&run_dir),
        before,
        "replay mutated the run directory"
    );
    assert_eq!(
        read_trajectories(&run_dir).expect("trajectories read back"),
        samples,
        "replay replaced the recorded rule-state rows"
    );
}

/// `--verify` accepts a faithful reproduction and names the recorded trace hash
/// the reproduction matched.
#[test]
fn verify_accepts_a_fresh_reproduction_and_names_the_recorded_hash() {
    let scratch = Scratch::new("verify-ok");
    let run_dir = record_run(&scratch, &repo_path(WALKING));
    let manifest = read_manifest(&run_dir);

    let output = replay(&scratch, "run", &["--verify"]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), golden("jsonl"));
    // The manifest records the reproduction's hash, which is the checked-in
    // trace hash golden.
    assert_eq!(manifest.stream.uncompressed_sha256, golden("sha256").trim());
    let message = stderr(&output);
    assert!(
        message.contains(&format!(
            "replay trace hash: {}",
            manifest.stream.uncompressed_sha256
        )),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("replay verified:"),
        "unexpected message: {message}"
    );
}

/// A recorded stream that no longer holds the canonical run fails verification
/// non-zero, names the artifact and the first differing byte, and emits no
/// stream of its own.
#[test]
fn verify_rejects_a_tampered_recorded_stream() {
    let scratch = Scratch::new("tampered-stream");
    let run_dir = record_run(&scratch, &repo_path(WALKING));

    let recorded = String::from_utf8(decompressed_stream(&run_dir)).expect("stream is UTF-8");
    let tampered = recorded.replace(r#""remaining":0"#, r#""remaining":1"#);
    assert_ne!(tampered, recorded, "the tamper must change the stream");
    std::fs::write(run_dir.join(EVENT_STREAM_FILE), gzip(tampered.as_bytes()))
        .expect("tampered stream is written");

    let manifest_before = file_sha256(&run_dir.join(MANIFEST_FILE));
    let output = replay(&scratch, "run", &["--verify"]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "",
        "a failed verification must emit no stream"
    );
    let message = stderr(&output);
    assert!(
        message.contains("does not reproduce the recorded run"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("first differing byte at offset"),
        "the diagnostic must locate the difference: {message}"
    );
    assert!(
        message.contains(EVENT_STREAM_FILE),
        "the diagnostic must name the artifact: {message}"
    );
    assert_eq!(
        file_sha256(&run_dir.join(MANIFEST_FILE)),
        manifest_before,
        "a failed verification mutated the run directory"
    );
}

/// A manifest whose recorded trace hash disagrees with the reproduction fails
/// even though the recorded stream still holds the run.
#[test]
fn verify_rejects_a_manifest_whose_recorded_hash_disagrees() {
    let scratch = Scratch::new("tampered-hash");
    let run_dir = record_run(&scratch, &repo_path(WALKING));

    let mut manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run_dir.join(MANIFEST_FILE)).expect("manifest"),
    )
    .expect("manifest is JSON");
    let absent = "0".repeat(64);
    manifest["stream"]["uncompressed_sha256"] = serde_json::Value::String(absent.clone());
    write_manifest_json(&run_dir, &manifest);

    let stream_before = file_sha256(&run_dir.join(EVENT_STREAM_FILE));
    let output = replay(&scratch, "run", &["--verify"]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let message = stderr(&output);
    assert!(
        message.contains("does not reproduce the recorded run"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("does not equal the manifest's recorded stream hash"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains(golden("sha256").trim()),
        "the diagnostic must name the reproduced hash: {message}"
    );
    assert!(
        message.contains(&absent),
        "the diagnostic must name the recorded hash: {message}"
    );
    assert_eq!(
        file_sha256(&run_dir.join(EVENT_STREAM_FILE)),
        stream_before,
        "a failed verification mutated the run directory"
    );
}

/// A manifest that names a stream this run-directory layout does not hold is
/// refused rather than silently verified against a different file.
#[test]
fn verify_rejects_a_manifest_that_names_another_stream() {
    let scratch = Scratch::new("tampered-descriptor");
    let run_dir = record_run(&scratch, &repo_path(WALKING));

    let mut manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run_dir.join(MANIFEST_FILE)).expect("manifest"),
    )
    .expect("manifest is JSON");
    manifest["stream"]["path"] = serde_json::Value::String("other.jsonl.gz".to_owned());
    write_manifest_json(&run_dir, &manifest);

    let output = replay(&scratch, "run", &["--verify"]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let message = stderr(&output);
    assert!(
        message.contains("other.jsonl.gz"),
        "the diagnostic must name the described stream: {message}"
    );
    assert!(
        message.contains(EVENT_STREAM_FILE),
        "the diagnostic must name the stream replay verifies: {message}"
    );
}

/// A scenario source edited after the run fails the recorded content hash check
/// before any reproduction, naming both hashes.
#[test]
fn replay_rejects_a_scenario_source_that_changed_since_the_run() {
    let scratch = Scratch::new("changed-source");
    // Record against a copy we can edit afterwards; the manifest records the
    // absolute source path, so replay reads exactly this file.
    let source = scratch.path("scenario.json5");
    std::fs::copy(repo_path(WALKING), &source).expect("scenario copy is written");
    let run_dir = record_run(&scratch, &source);
    let recorded = read_manifest(&run_dir).scenario.content_sha256;

    let mut edited = std::fs::read_to_string(&source).expect("scenario is readable");
    edited.push_str("\n// edited after the run\n");
    std::fs::write(&source, edited).expect("edited scenario is written");

    let artifacts_before = content_hashes(&run_dir);
    let output = replay(&scratch, "run", &[]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let message = stderr(&output);
    assert!(
        message.contains("content hash"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains(&recorded),
        "the diagnostic must name the recorded hash: {message}"
    );
    assert!(
        message.contains("changed since the run"),
        "unexpected message: {message}"
    );
    assert_eq!(
        content_hashes(&run_dir),
        artifacts_before,
        "a refused replay mutated the run directory"
    );
}

/// A run directory that does not exist, or that holds artifacts but no
/// completion marker, is refused with a diagnostic naming the path.
#[test]
fn replay_rejects_a_missing_or_incomplete_run_directory() {
    let scratch = Scratch::new("missing");

    let absent = replay(&scratch, "absent", &[]);
    assert_eq!(absent.status.code(), Some(1), "stderr: {}", stderr(&absent));
    assert_eq!(stdout(&absent), "");
    assert!(
        stderr(&absent).contains("does not exist"),
        "unexpected message: {}",
        stderr(&absent)
    );

    // An interrupted run holds its artifacts but not the completion marker.
    let run_dir = record_run(&scratch, &repo_path(WALKING));
    std::fs::remove_file(run_dir.join(MANIFEST_FILE)).expect("the completion marker is removed");

    let incomplete = replay(&scratch, "run", &[]);
    assert_eq!(
        incomplete.status.code(),
        Some(1),
        "stderr: {}",
        stderr(&incomplete)
    );
    assert_eq!(stdout(&incomplete), "");
    let message = stderr(&incomplete);
    assert!(
        message.contains("incomplete"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains(MANIFEST_FILE),
        "the diagnostic must name the missing marker: {message}"
    );
}
