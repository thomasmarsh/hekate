//! End-to-end golden-trace check for the headless command.
//!
//! Regenerate the fixtures with:
//!
//! ```sh
//! cargo run -p tangle-cli -- run scenarios/walking/walking_guide_v1.json5 \
//!   --seed 0 --ticks 250 \
//!   --output tests/golden/walking_guide_v1.trace.jsonl \
//!   --hash-file tests/golden/walking_guide_v1.trace.sha256
//! ```
//!
//! A failure here means canonical serialization or kernel behavior changed.
//! Inspect the printed trace before regenerating: an unexpected diff is a
//! regression, not a stale fixture.

use std::path::PathBuf;

use tangle_cli::{canonical_trace, load_scenario};
use tangle_sim::RunConfig;

const GOLDEN_SEED: u64 = 0;
const GOLDEN_TICKS: u64 = 250;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn golden(extension: &str) -> String {
    let path = repo_path(&format!("tests/golden/walking_guide_v1.trace.{extension}"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read golden file '{}': {error}", path.display()))
}

#[test]
fn checked_in_walking_trace_matches_golden_bytes_and_hash() {
    let scenario = load_scenario(&repo_path("scenarios/walking/walking_guide_v1.json5"))
        .expect("walking scenario loads");
    let trace = canonical_trace(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS)
        .expect("walking scenario runs");

    let expected_bytes = golden("jsonl");
    assert_eq!(
        std::str::from_utf8(trace.bytes()).expect("trace is UTF-8"),
        expected_bytes,
        "canonical trace changed; see the module docs before regenerating the golden file"
    );

    let expected_hash = golden("sha256");
    assert_eq!(
        trace.hash(),
        expected_hash.trim(),
        "trace hash disagrees with the checked-in golden hash"
    );
}

#[test]
fn same_seed_produces_the_same_hash_twice() {
    fn hash() -> String {
        let scenario = load_scenario(&repo_path("scenarios/walking/walking_guide_v1.json5"))
            .expect("walking scenario loads");
        canonical_trace(scenario, RunConfig::new(GOLDEN_SEED), GOLDEN_TICKS)
            .expect("walking scenario runs")
            .hash()
            .to_owned()
    }

    assert_eq!(hash(), hash());
}
