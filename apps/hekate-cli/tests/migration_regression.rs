//! Migration-regression gate over schema provenance (Phase 2 Increment 0).
//!
//! `docs/schema-v2-contract.md` fixes the deterministic version-1 to version-2
//! migration, and `PHASE_2_PLAN.md` gates Phase 2 on the promise that a schema
//! migration silently changing Phase 1 behavior is caught by "source and
//! normalized hashes, explicit migration version, and retained Phase 1
//! baseline". This suite is that gate.
//!
//! For every checked-in version-1 scenario under `scenarios/` it drives two runs
//! at the same seed and tick count and proves behavioral preservation:
//!
//! 1. the **original reader** — `load_scenario`, the version-1 direct compile
//!    path the `run` command still uses; and
//! 2. the **migration path** — parse, [`migrate_v1_to_v2`], validate the
//!    version-2 document, then [`CompiledScenario::compile_v2`].
//!
//! A checked-in version-2 scenario (the Phase 2 increment fixtures under
//! `scenarios/phase2/`) has no migration to compare, so the gate validates and
//! compiles it directly instead. Either way every checked-in scenario is
//! enumerated and must load.
//!
//! The two canonical traces must agree on every line after the run header. The
//! header is provenance, and it carries exactly one declared correction; see
//! below.
//!
//! The three scenarios with a frozen golden trace additionally reproduce it: the
//! migrated event body equals the golden body byte-for-byte, the migrated header
//! equals the golden header except for `schema_version`, and the migrated
//! full-trace hash matches the pinned fixture under
//! `apps/hekate-cli/tests/golden/<id>.migrated.trace.sha256`.
//!
//! # The declared, versioned correction
//!
//! A migrated run consumes a normalized version-2 document, so
//! [`CompiledScenario::compile_v2`] records `schema_version: 2` on the compiled
//! scenario, and the trace header — the field that names the schema version the
//! scenario was authored against — records `2` where the version-1 direct reader
//! records `1`. That single header field is the only difference; the event body
//! is byte-identical for every scenario. Nothing changes silently, because the
//! run's `ScenarioProvenance` still records the authored `schema_version` (`1`),
//! the `normalized_sha256` of the version-2 bytes the run consumed, and the
//! `migration_version` (`1`), so a trace change is attributable to the migration
//! rather than to the kernel.
//!
//! No frozen Phase 1 baseline hash is altered: the `run` command still uses the
//! version-1 direct reader, so `baselines/phase1/baseline.json` and
//! `tests/golden/*.trace.sha256` are unchanged, and the pinned migrated fixtures
//! below are *additional* evidence, not a replacement.
//!
//! Regenerate a pinned fixture (after confirming the diff is only the declared
//! header field) with:
//!
//! ```sh
//! cargo run -p hekate-cli -- migrate <scenario>.json5 --output /tmp/v2.json
//! cargo run -p hekate-cli -- run /tmp/v2.json --seed <seed> --ticks <ticks> \
//!   --hash-file apps/hekate-cli/tests/golden/<id>.migrated.trace.sha256
//! ```
//!
//! An unexpected difference is a migration regression, not a stale fixture.

use std::path::{Path, PathBuf};

use hekate_cli::{Trace, canonical_trace, load_scenario};
use hekate_model::{
    CompiledScenario, SUPPORTED_SCHEMA_VERSION, ScenarioDocument, ScenarioSource, migrate_v1_to_v2,
    parse_scenario_document, validate_v2,
};
use hekate_sim::RunConfig;
use serde_json::Value;

/// A scenario with a frozen golden trace and the declared run that reproduces
/// it. `golden` is the id the checked-in `tests/golden/<id>.trace.*` files and
/// the pinned `<id>.migrated.trace.sha256` fixture are named by.
struct Frozen {
    scenario: &'static str,
    id: &'static str,
    seed: u64,
    ticks: u64,
}

/// Every scenario whose trace is frozen by a checked-in golden.
const FROZEN: [Frozen; 3] = [
    Frozen {
        scenario: "scenarios/walking/walking_guide_v1.json5",
        id: "walking_guide_v1",
        seed: 0,
        ticks: 250,
    },
    Frozen {
        scenario: "scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5",
        id: "four_leg_pedestrian_ew_priority_v1",
        seed: 1,
        ticks: 1200,
    },
    Frozen {
        scenario: "scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5",
        id: "four_leg_pedestrian_ns_priority_v1",
        seed: 1,
        ticks: 1200,
    },
];

/// The fixed run every checked-in scenario is compared at. It differs from the
/// golden seeds on purpose, so the differential proof is not a restatement of
/// the golden check.
const DIFFERENTIAL_SEED: u64 = 7;
const DIFFERENTIAL_TICKS: u64 = 1200;

/// The frozen Phase 1 baseline whose Standard preset the walking scenario
/// reproduces through the original reader.
const BASELINE: &str = "baselines/phase1/baseline.json";

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The pinned migrated-hash fixture, which lives beside this suite rather than
/// under the repository's `tests/golden/` so a migration fixture cannot be
/// mistaken for a Phase 1 golden.
fn pinned_migrated_fixture(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{id}.migrated.trace.sha256"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()))
}

/// Every `*.json5` under `dir`, recursively, sorted for determinism.
fn json5_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("scenario directory is readable") {
        let entry = entry.expect("directory entry is readable");
        let path = entry.path();
        if path.is_dir() {
            json5_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "json5") {
            out.push(path);
        }
    }
}

fn checked_in_scenarios() -> Vec<PathBuf> {
    let root = repo_path("scenarios");
    let mut files = Vec::new();
    json5_files(&root, &mut files);
    files.sort();
    files
}

/// Parse, migrate, validate, and compile a version-1 source through the
/// migration path, failing loudly on anything but a clean migration.
fn migrated_scenario(path: &Path, text: &str) -> CompiledScenario {
    let document = parse_scenario_document(text)
        .unwrap_or_else(|error| panic!("'{}' does not parse: {error}", path.display()));
    let source: ScenarioSource = match document {
        ScenarioDocument::V1(source) => source,
        ScenarioDocument::V2(_) => panic!(
            "'{}' declares schema_version 2; this gate requires every checked-in \
             scenario to be a version-1 source that migrates",
            path.display()
        ),
    };
    let migrated = migrate_v1_to_v2(&source);
    assert_eq!(
        migrated.schema_version,
        SUPPORTED_SCHEMA_VERSION,
        "'{}' must normalize to schema_version {SUPPORTED_SCHEMA_VERSION}",
        path.display()
    );
    let diagnostics = validate_v2(&migrated);
    assert!(
        diagnostics.is_empty(),
        "'{}' migrated to an invalid version-2 document: {diagnostics:?}",
        path.display()
    );
    CompiledScenario::compile_v2(migrated).unwrap_or_else(|diagnostics| {
        panic!(
            "'{}' failed to compile from its version-2 form: {diagnostics:?}",
            path.display()
        )
    })
}

fn run(scenario: CompiledScenario, seed: u64, ticks: u64) -> Trace {
    canonical_trace(scenario, RunConfig::new(seed), ticks)
        .unwrap_or_else(|error| panic!("run at seed {seed}, {ticks} ticks failed: {error}"))
}

/// The trace's run header (its first JSON line) and the remaining event body.
fn split_trace(trace: &Trace) -> (&str, &str) {
    std::str::from_utf8(trace.bytes())
        .expect("a canonical trace is UTF-8")
        .split_once('\n')
        .expect("a canonical trace starts with a run header line")
}

/// Assert two run headers are equal once the copied header's `schema_version` is
/// restored to the original's, which proves they differ in that field alone.
fn assert_header_differs_only_in_schema_version(original: &str, migrated: &str) {
    let original_json: Value = serde_json::from_str(original).expect("the header is JSON");
    let mut migrated_json: Value =
        serde_json::from_str(migrated).expect("the migrated header is JSON");
    let authored = original_json
        .get("schema_version")
        .and_then(Value::as_u64)
        .expect("the original header names a schema version");
    assert_eq!(
        migrated_json.get("schema_version").and_then(Value::as_u64),
        Some(SUPPORTED_SCHEMA_VERSION as u64),
        "a migrated run's header must name the normalized schema version"
    );
    migrated_json["schema_version"] = Value::from(authored);
    assert_eq!(
        migrated_json, original_json,
        "a migrated run may differ from the original only in the header schema_version"
    );
}

/// Parse a fresh Phase 1 capture's Standard preset hash from the checked-in
/// baseline manifest.
fn baseline_standard_preset_hash() -> String {
    let baseline: Value =
        serde_json::from_str(&read(&repo_path(BASELINE))).expect("the Phase 1 baseline is JSON");
    baseline["presets"]
        .as_array()
        .expect("the baseline lists presets")
        .iter()
        .find(|preset| preset["preset"] == "standard")
        .expect("the baseline records the Standard preset")["trace_sha256"]
        .as_str()
        .expect("the Standard preset records a trace hash")
        .to_owned()
}

/// The suite enumerates every checked-in scenario, so adding one cannot escape
/// the gate.
#[test]
fn every_checked_in_scenario_is_enumerated() {
    let files = checked_in_scenarios();
    assert!(
        !files.is_empty(),
        "no scenarios found under {}",
        repo_path("scenarios").display()
    );

    let mut names: Vec<String> = files
        .iter()
        .map(|path| {
            path.strip_prefix(repo_path("scenarios"))
                .expect("a scenario lives under scenarios/")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    for frozen in &FROZEN {
        let relative = frozen
            .scenario
            .strip_prefix("scenarios/")
            .expect("a frozen scenario lives under scenarios/");
        assert!(
            names.iter().any(|name| name == relative),
            "the frozen scenario '{}' is missing from the enumerated set",
            frozen.scenario
        );
    }
}

/// The gate: every checked-in scenario is covered. A version-1 scenario
/// migrates and reproduces the original reader's event body byte-for-byte,
/// differing only in the declared header `schema_version`. A version-2 scenario
/// has no version-1 original to compare against, so it must validate, compile,
/// and advance the kernel through the version-2 run path, which keeps it inside
/// the gate as a checked-in, loadable, runnable scenario rather than an escape
/// hatch.
#[test]
fn every_scenario_migrates_or_is_a_valid_version_2_document() {
    for path in checked_in_scenarios() {
        let text = read(&path);
        let document = parse_scenario_document(&text)
            .unwrap_or_else(|error| panic!("'{}' does not parse: {error}", path.display()));

        match document {
            ScenarioDocument::V2(source) => {
                let diagnostics = validate_v2(&source);
                assert!(
                    diagnostics.is_empty(),
                    "'{}' is an invalid version-2 document: {diagnostics:?}",
                    path.display()
                );
                let scenario = CompiledScenario::compile_v2(source).unwrap_or_else(|diagnostics| {
                    panic!(
                        "'{}' failed to compile from its version-2 form: {diagnostics:?}",
                        path.display()
                    )
                });
                let trace = run(scenario, DIFFERENTIAL_SEED, DIFFERENTIAL_TICKS);
                assert!(
                    !trace.bytes().is_empty(),
                    "'{}' produced an empty version-2 run",
                    path.display()
                );
                continue;
            }
            ScenarioDocument::V1(_) => {}
        }

        let original = run(
            load_scenario(&path).expect("the original reader compiles the scenario"),
            DIFFERENTIAL_SEED,
            DIFFERENTIAL_TICKS,
        );
        let migrated = run(
            migrated_scenario(&path, &text),
            DIFFERENTIAL_SEED,
            DIFFERENTIAL_TICKS,
        );

        let (original_header, original_body) = split_trace(&original);
        let (migrated_header, migrated_body) = split_trace(&migrated);

        assert_eq!(
            migrated_body,
            original_body,
            "'{}' migrated to a different event body at seed {DIFFERENTIAL_SEED}, \
             {DIFFERENTIAL_TICKS} ticks",
            path.display()
        );
        assert_header_differs_only_in_schema_version(original_header, migrated_header);
    }
}

/// Each frozen golden is reproduced through the migration path: the event body
/// is byte-identical, the header differs only in `schema_version`, and the
/// migrated full-trace hash matches its pinned fixture.
#[test]
fn migrated_frozen_runs_reproduce_their_golden_bodies_and_pinned_hashes() {
    for frozen in &FROZEN {
        let path = repo_path(frozen.scenario);
        let text = read(&path);

        let original = run(
            load_scenario(&path).expect("the original reader compiles the scenario"),
            frozen.seed,
            frozen.ticks,
        );
        let migrated = run(migrated_scenario(&path, &text), frozen.seed, frozen.ticks);

        let golden_bytes = std::fs::read(repo_path(&format!(
            "tests/golden/{}.trace.jsonl",
            frozen.id
        )))
        .unwrap_or_else(|error| {
            panic!("cannot read the golden trace for '{}': {error}", frozen.id)
        });
        let golden_text = std::str::from_utf8(&golden_bytes).expect("the golden trace is UTF-8");
        let (golden_header, golden_body) = golden_text
            .split_once('\n')
            .expect("the golden trace starts with a run header line");

        // The original reader still reproduces the frozen golden exactly, so the
        // baseline hash is untouched.
        assert_eq!(
            original.bytes(),
            golden_bytes,
            "'{}' no longer reproduces its frozen golden through the original reader",
            frozen.id
        );

        let (migrated_header, migrated_body) = split_trace(&migrated);
        assert_eq!(
            migrated_body, golden_body,
            "'{}' migrated to a different event body than its frozen golden",
            frozen.id
        );
        assert_header_differs_only_in_schema_version(golden_header, migrated_header);

        let pinned = read(&pinned_migrated_fixture(frozen.id));
        assert_eq!(
            migrated.hash(),
            pinned.trim(),
            "'{}' migrated trace hash changed; see this module's docs before \
             regenerating the pinned fixture",
            frozen.id
        );
    }
}

/// The walking scenario's migrated run is the frozen Standard preset plus the
/// declared header correction: the original reader reproduces the baseline hash
/// exactly and the migrated body is unchanged from it.
#[test]
fn migrated_walking_run_matches_the_frozen_standard_preset_baseline() {
    let walking = FROZEN
        .iter()
        .find(|frozen| frozen.id == "walking_guide_v1")
        .expect("the walking scenario is frozen");
    let path = repo_path(walking.scenario);
    let text = read(&path);

    let original = run(
        load_scenario(&path).expect("the original reader compiles the scenario"),
        walking.seed,
        walking.ticks,
    );
    let migrated = run(migrated_scenario(&path, &text), walking.seed, walking.ticks);

    let baseline_hash = baseline_standard_preset_hash();
    assert_eq!(
        original.hash(),
        baseline_hash,
        "the original reader must still reproduce the frozen Standard preset baseline hash"
    );
    let (baseline_header, baseline_body) = split_trace(&original);
    let (migrated_header, migrated_body) = split_trace(&migrated);
    assert_eq!(
        migrated_body, baseline_body,
        "the migrated walking run changed the frozen Standard preset event body"
    );
    assert_header_differs_only_in_schema_version(baseline_header, migrated_header);
}
