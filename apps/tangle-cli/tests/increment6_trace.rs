//! Golden-trace check for the Increment 6 comparison's canonical event stream.
//!
//! The increment's gate pins the canonical event stream of the released
//! comparison: the same manifest must reproduce the same stream in the
//! supported determinism environment. Each variant therefore has one checked-in
//! golden — the canonical trace at the declared seed and tick count, plus its
//! SHA-256 — and this suite proves the golden is that stream: the `run` command
//! writes exactly the golden bytes and hash into a fresh run directory, and
//! `replay --verify` reproduces the recorded stream from that same manifest.
//!
//! The declared run covers one 58 s signal cycle of the variants (1200 fixed
//! steps of 50 ms) at seed 1, one of the experiment's ten bank seeds, so the
//! trace spans both green phases and both walk intervals.
//!
//! Regenerate the fixtures with:
//!
//! ```sh
//! cargo run -p tangle-cli -- run scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5 \
//!   --seed 1 --ticks 1200 \
//!   --output tests/golden/four_leg_pedestrian_ew_priority_v1.trace.jsonl \
//!   --hash-file tests/golden/four_leg_pedestrian_ew_priority_v1.trace.sha256
//! cargo run -p tangle-cli -- run scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5 \
//!   --seed 1 --ticks 1200 \
//!   --output tests/golden/four_leg_pedestrian_ns_priority_v1.trace.jsonl \
//!   --hash-file tests/golden/four_leg_pedestrian_ns_priority_v1.trace.sha256
//! ```
//!
//! A failure here means canonical serialization or kernel behavior changed.
//! Inspect the diff before regenerating: an unexpected change is a regression,
//! not a stale fixture.

use std::path::PathBuf;
use std::process::Command;

use sha2::{Digest, Sha256};
use tangle_cli::{MANIFEST_FILE, RunManifest};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_tangle-cli");

/// The two Increment 6 variants' scenario sources, in the spec's declared order.
const VARIANTS: [&str; 2] = [
    "scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5",
    "scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5",
];

/// The scenario ids the variants' goldens are named by, in the same order.
const IDS: [&str; 2] = [
    "four_leg_pedestrian_ew_priority_v1",
    "four_leg_pedestrian_ns_priority_v1",
];

/// The declared golden run: seed 1 is a bank seed and 1200 steps of the Standard
/// 50 ms preset cover one 58 s signal cycle.
const GOLDEN_SEED: u64 = 1;
const GOLDEN_TICKS: u64 = 1200;
const GOLDEN_STEP_S: f64 = 0.05;

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "tangle-cli-increment6-trace-{name}-{}",
            std::process::id()
        ));
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

/// A checked-in golden file's text.
fn golden(id: &str, extension: &str) -> String {
    let path = repo_path(&format!("tests/golden/{id}.trace.{extension}"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read golden '{}': {error}", path.display()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The declared golden run of one variant, written into a fresh run directory
/// under `scratch`.
///
/// The scenario is passed absolute so the manifest records an absolute source
/// path that `replay` resolves wherever the command runs.
fn golden_run(scratch: &Scratch, index: usize) -> PathBuf {
    let seed = GOLDEN_SEED.to_string();
    let ticks = GOLDEN_TICKS.to_string();
    let output = Command::new(CLI)
        .arg("run")
        .arg(repo_path(VARIANTS[index]))
        .args(["--seed", &seed, "--ticks", &ticks])
        .args(["--output", "trace.jsonl", "--hash-file", "trace.sha256"])
        .args(["--run-dir", "run"])
        .current_dir(&scratch.dir)
        .output()
        .expect("tangle-cli runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    scratch.path("run")
}

/// Each variant's declared run writes exactly its checked-in golden bytes and
/// hash, and the manifest records the parameters the golden declares.
#[test]
fn the_increment6_variant_traces_match_their_golden_bytes_and_hashes() {
    for (index, id) in IDS.iter().enumerate() {
        let scratch = Scratch::new(&format!("golden-{id}"));
        let run_dir = golden_run(&scratch, index);
        let written = std::fs::read(scratch.path("trace.jsonl")).expect("the trace is written");

        assert_eq!(
            std::str::from_utf8(&written).expect("the trace is UTF-8"),
            golden(id, "jsonl"),
            "'{id}' changed since its golden was recorded; see the module docs before regenerating"
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path("trace.sha256"))
                .expect("the hash file is written")
                .trim(),
            golden(id, "sha256").trim(),
            "'{id}' hashes differently from its checked-in golden hash"
        );
        assert_eq!(
            golden(id, "sha256").trim(),
            sha256_hex(&written),
            "'{id}'s golden hash is not the hash of its golden bytes"
        );

        // The manifest the golden was recorded from is the one replay verifies:
        // it names the declared seed, step, and tick count, and it hashes the
        // stream the golden holds.
        let manifest: RunManifest = serde_json::from_str(
            &std::fs::read_to_string(run_dir.join(MANIFEST_FILE)).expect("the manifest is written"),
        )
        .expect("the manifest is JSON");
        assert_eq!(manifest.seed, GOLDEN_SEED, "'{id}'");
        assert_eq!(manifest.ticks, GOLDEN_TICKS, "'{id}'");
        assert_eq!(manifest.step_s, GOLDEN_STEP_S, "'{id}'");
        assert_eq!(manifest.fidelity, "standard", "'{id}'");
        assert_eq!(
            manifest.scenario.source_path,
            repo_path(VARIANTS[index]).display().to_string()
        );
        assert_eq!(manifest.scenario.id, *id);
        assert_eq!(
            manifest.stream.uncompressed_sha256,
            golden(id, "sha256").trim(),
            "'{id}'s manifest records the golden hash"
        );
    }
}

/// `replay --verify` reproduces each variant's golden event stream from the same
/// manifest, so the checked-in golden is the canonical stream of a run the
/// determinism contract still reproduces.
#[test]
fn replay_verifies_the_increment6_golden_manifests() {
    for (index, id) in IDS.iter().enumerate() {
        let scratch = Scratch::new(&format!("replay-{id}"));
        let run_dir = golden_run(&scratch, index);

        let verified = Command::new(CLI)
            .arg("replay")
            .arg(&run_dir)
            .arg("--verify")
            .current_dir(&scratch.dir)
            .output()
            .expect("tangle-cli runs");
        assert_eq!(
            verified.status.code(),
            Some(0),
            "'{id}': {}",
            String::from_utf8_lossy(&verified.stderr)
        );
        assert_eq!(
            std::str::from_utf8(&verified.stdout).expect("the reproduced stream is UTF-8"),
            golden(id, "jsonl"),
            "'{id}'s reproduced stream is not the checked-in golden"
        );
    }
}
