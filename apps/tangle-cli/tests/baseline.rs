//! Checked-in Phase 1 baseline guard.
//!
//! `baselines/phase1/baseline.json` freezes the observable Phase 1 behavior at
//! every fidelity preset. Regenerate it with:
//!
//! ```sh
//! cargo run -p tangle-cli -- baseline scenarios/walking/walking_guide_v1.json5 \
//!   --seed 0 --duration-s 12.5 \
//!   --output baselines/phase1/baseline.json \
//!   --performance baselines/phase1/performance.json
//! ```
//!
//! A failure here means kernel behavior or the baseline format changed. Inspect
//! the diff: an unexpected trace-hash change is a regression, not a stale
//! fixture, and needs a versioned explanation before regenerating.

use std::path::PathBuf;

use tangle_cli::{CaptureRequest, capture, load_scenario_hashed};

const BASELINE_SEED: u64 = 0;
const BASELINE_DURATION_S: f64 = 12.5;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn checked_in_baseline_matches_a_fresh_capture() {
    let (scenario, content_sha256) =
        load_scenario_hashed(&repo_path("scenarios/walking/walking_guide_v1.json5"))
            .expect("walking scenario loads");
    let (baseline, _) = capture(CaptureRequest {
        scenario: &scenario,
        source_path: "scenarios/walking/walking_guide_v1.json5",
        content_sha256: &content_sha256,
        seed: BASELINE_SEED,
        duration_s: BASELINE_DURATION_S,
    })
    .expect("baseline captures");

    let checked_in = std::fs::read_to_string(repo_path("baselines/phase1/baseline.json"))
        .expect("baselines/phase1/baseline.json is checked in");
    assert_eq!(
        checked_in,
        baseline.to_pretty_json(),
        "checked-in Phase 1 baseline is stale; see the module docs before regenerating"
    );
}

#[test]
fn standard_preset_hash_matches_the_golden_trace() {
    let golden = std::fs::read_to_string(repo_path("tests/golden/walking_guide_v1.trace.sha256"))
        .expect("golden trace hash is checked in");
    let (scenario, content_sha256) =
        load_scenario_hashed(&repo_path("scenarios/walking/walking_guide_v1.json5"))
            .expect("walking scenario loads");
    let (baseline, _) = capture(CaptureRequest {
        scenario: &scenario,
        source_path: "scenarios/walking/walking_guide_v1.json5",
        content_sha256: &content_sha256,
        seed: BASELINE_SEED,
        duration_s: BASELINE_DURATION_S,
    })
    .expect("baseline captures");

    let standard = baseline
        .presets
        .iter()
        .find(|preset| preset.preset == "standard")
        .expect("standard preset exists");
    assert_eq!(standard.trace_sha256, golden.trim());
}
