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

use sha2::{Digest, Sha256};
use tangle_cli::{CaptureRequest, capture, load_scenario_provenance, migrate_scenario};

const BASELINE_SEED: u64 = 0;
const BASELINE_DURATION_S: f64 = 12.5;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn checked_in_baseline_matches_a_fresh_capture() {
    let (scenario, provenance) =
        load_scenario_provenance(&repo_path("scenarios/walking/walking_guide_v1.json5"))
            .expect("walking scenario loads");
    let (baseline, _) = capture(CaptureRequest {
        scenario: &scenario,
        source_path: "scenarios/walking/walking_guide_v1.json5",
        content_sha256: &provenance.content_sha256,
        normalized_sha256: &provenance.normalized_sha256,
        migration_version: provenance.migration_version,
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
    let (scenario, provenance) =
        load_scenario_provenance(&repo_path("scenarios/walking/walking_guide_v1.json5"))
            .expect("walking scenario loads");
    let (baseline, _) = capture(CaptureRequest {
        scenario: &scenario,
        source_path: "scenarios/walking/walking_guide_v1.json5",
        content_sha256: &provenance.content_sha256,
        normalized_sha256: &provenance.normalized_sha256,
        migration_version: provenance.migration_version,
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

/// The provenance fields come from the bytes on disk and the migration that
/// produced them, not from a placeholder: for the checked-in version-1 walking
/// scenario the normalized hash is the SHA-256 of the `migrate` output and the
/// recorded migration version is the transform that ran.
#[test]
fn version_1_provenance_covers_the_migration_output() {
    let walking = repo_path("scenarios/walking/walking_guide_v1.json5");
    let (_, provenance) = load_scenario_provenance(&walking).expect("walking scenario loads");
    let normalized = migrate_scenario(&walking).expect("the version-1 source migrates");

    assert_eq!(provenance.schema_version, 1);
    assert_eq!(
        provenance.migration_version,
        tangle_model::MIGRATION_VERSION
    );
    assert_eq!(
        provenance.normalized_sha256,
        sha256_hex(normalized.as_bytes()),
        "the normalized hash must cover exactly the migrate output"
    );
}

/// A source already authored at version 2 records no migration, and its
/// normalized hash covers the canonical re-serialization of the document it
/// parsed: for a canonical fixture that is the fixture's own bytes.
#[test]
fn version_2_provenance_records_no_migration() {
    let v2 = repo_path("crates/tangle-model/tests/fixtures/migration/population_v2.json");
    let (_, provenance) = load_scenario_provenance(&v2).expect("the version-2 fixture loads");
    let bytes = std::fs::read(&v2).expect("the version-2 fixture is readable");

    assert_eq!(provenance.schema_version, 2);
    assert_eq!(provenance.migration_version, 0);
    assert_eq!(provenance.normalized_sha256, sha256_hex(&bytes));
}
