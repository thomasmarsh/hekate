//! Contract tests for the Increment 6 experiment inputs.
//!
//! The Increment 6 comparison is only reproducible if its inputs are checked in
//! and their design is pinned: two scenario-source variants of one four-leg
//! car/pedestrian intersection that differ *only* in the signal timing plan, a
//! checked-in experiment spec and common-random-number seed bank naming them,
//! and evidence that a geometrically different benchmark is expressible through
//! scenario data alone.
//!
//! These tests hold that contract from the outside: the equivalence of the two
//! variants is proven at the byte level and again through the parsed source, the
//! CLI is invoked to validate and run both variants, the spec and bank parse and
//! name the checked-in scenarios, the ordered seeds, and the run policy, and the
//! offset-junction benchmark loads, runs, and is structurally different from the
//! variants.

use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;
use hekate_cli::{
    METRIC_DEFINITION_VERSION, PRESETS, SEED_BANK_VERSION, SamplingPolicy, load_scenario,
    read_seed_bank,
};
use hekate_model::{ScenarioSource, parse_scenario_source};
use hekate_sim::{Event, RunConfig, Simulation};

/// The binary under test, built by Cargo for this integration test.
const CLI: &str = env!("CARGO_BIN_EXE_hekate-cli");

const VARIANT_A: &str = "scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5";
const VARIANT_B: &str = "scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5";
const OFFSET_JUNCTION: &str = "scenarios/benchmarks/offset_junction_v1.json5";
const EXPERIMENT: &str = "experiments/increment6_signal_timing_v1/experiment.json";
const SEED_BANK: &str = "experiments/increment6_signal_timing_v1/seed_bank.json";

/// Ticks the CLI contract tests run: 30 s at the Standard step, past the point
/// where the two variants' signal plans diverge (20 s versus 30 s of green).
const CONTRACT_TICKS: u64 = 600;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn read(relative: &str) -> String {
    let path = repo_path(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()))
}

/// The checked-in experiment spec, as a runner reads it.
#[derive(Debug, Deserialize)]
struct ExperimentSpec {
    experiment_version: u32,
    id: String,
    summary: String,
    fidelity: String,
    step_s: f64,
    ticks: u64,
    duration_s: f64,
    seed_bank: String,
    sampling: SamplingPolicy,
    controlled: Vec<String>,
    independent_variable: String,
    variants: Vec<ExperimentVariant>,
}

/// One side of the experiment.
#[derive(Debug, Deserialize)]
struct ExperimentVariant {
    variant: String,
    scenario: String,
}

fn experiment() -> ExperimentSpec {
    serde_json::from_str(&read(EXPERIMENT)).expect("the experiment spec is valid JSON")
}

/// A scratch working directory that is removed when the test ends.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "hekate-cli-experiment-{name}-{}",
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

/// Run the CLI with `arguments` from the repository root.
fn cli(arguments: &[&str]) -> std::process::Output {
    Command::new(CLI)
        .args(arguments)
        .current_dir(repo_path(""))
        .output()
        .expect("hekate-cli runs")
}

/// Run the CLI and require success, returning its stdout.
fn cli_ok(arguments: &[&str]) -> String {
    let output = cli(arguments);
    assert_eq!(
        output.status.code(),
        Some(0),
        "`hekate-cli {}` failed: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Parse a scenario source document without validating or compiling it.
fn source(relative: &str) -> ScenarioSource {
    parse_scenario_source(&read(relative))
        .unwrap_or_else(|error| panic!("'{relative}' parses: {error}"))
}

/// Zero every signal phase duration in `source`, so only the timing plan is
/// normalized away.
fn clear_signal_durations(source: &mut ScenarioSource) {
    for signal in source.signals.iter_mut() {
        for phase in signal.phases.iter_mut() {
            phase.duration_s = 1.0;
        }
    }
    for crossing in source.crossings.iter_mut() {
        if let Some(signal) = crossing.pedestrian_signal.as_mut() {
            for phase in signal.phases.iter_mut() {
                phase.duration_s = 1.0;
            }
        }
    }
}

/// The signal controller's cycle length, in seconds.
fn signal_cycle(source: &ScenarioSource) -> f64 {
    source
        .signals
        .first()
        .expect("both variants signal their intersection")
        .phases
        .iter()
        .map(|phase| phase.duration_s)
        .sum()
}

/// The variant `id` and the signal timing plan are the only differences.
///
/// The check is made twice. First on the raw bytes: every differing line is
/// either the scenario `id` or a `duration_s` phase duration, so no other
/// scenario data can drift between the files. Then on the parsed documents:
/// with the `id` and every phase duration normalized, the variants are equal,
/// so nothing else about the parsed scenario differs.
#[test]
fn the_variants_differ_only_in_the_signal_timing_plan() {
    let a = read(VARIANT_A);
    let b = read(VARIANT_B);
    let a_lines: Vec<&str> = a.lines().collect();
    let b_lines: Vec<&str> = b.lines().collect();
    assert_eq!(
        a_lines.len(),
        b_lines.len(),
        "the variants must stay line-for-line comparable"
    );

    let differing: Vec<&str> = a_lines
        .iter()
        .zip(&b_lines)
        .filter(|(left, right)| left != right)
        .map(|(left, _)| *left)
        .collect();
    assert!(!differing.is_empty(), "the variants are identical");
    for line in &differing {
        assert!(
            line.contains("duration_s:") || line.trim_start().starts_with("id: '"),
            "the variants differ outside the id and the signal timing plan: {line}"
        );
    }

    let mut a_source = source(VARIANT_A);
    let mut b_source = source(VARIANT_B);
    assert_ne!(a_source.id, b_source.id, "each variant needs its own id");

    // Both variants control the same movements with the same heads and the same
    // rules, and hold the cycle length: the independent variable is the green
    // split, not the cycle or the controlled movements.
    let heads = |source: &ScenarioSource| {
        source
            .signals
            .iter()
            .map(|signal| {
                signal
                    .heads
                    .iter()
                    .map(|head| head.movement.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(heads(&a_source), heads(&b_source));
    assert_eq!(a_source.rules, b_source.rules);
    assert!(
        a_source.signals[0].phases.len() > 1,
        "the plan is multi-phase"
    );
    assert_eq!(
        signal_cycle(&a_source),
        signal_cycle(&b_source),
        "the green split changes, the cycle length does not"
    );
    assert_ne!(
        a_source.signals[0].phases[0].duration_s, b_source.signals[0].phases[0].duration_s,
        "the first green must actually differ between the variants"
    );

    // Normalized to one timing plan, the two documents are one scenario.
    a_source.id = "variant".to_owned();
    b_source.id = "variant".to_owned();
    clear_signal_durations(&mut a_source);
    clear_signal_durations(&mut b_source);
    assert_eq!(
        a_source, b_source,
        "apart from the id and the timing plan the variants are one scenario"
    );
}

/// The CLI validates and runs both checked-in variants, and the two variants'
/// canonical traces differ at one seed, so the independent variable is
/// observable rather than decorative.
#[test]
fn both_variants_validate_and_run_through_the_cli() {
    let scratch = Scratch::new("variants");
    let mut traces = Vec::new();
    for (variant, relative) in [("ew_priority", VARIANT_A), ("ns_priority", VARIANT_B)] {
        let report = cli_ok(&["validate", relative]);
        assert!(
            report.contains("valid scenario") && report.contains(variant),
            "validate reported '{report}' for {relative}"
        );

        let trace = scratch.path(&format!("{variant}.jsonl"));
        cli_ok(&[
            "run",
            relative,
            "--seed",
            "1",
            "--ticks",
            &CONTRACT_TICKS.to_string(),
            "--output",
            trace.to_str().expect("utf-8 path"),
        ]);
        let bytes = std::fs::read(&trace).expect("the run wrote a trace");
        assert!(!bytes.is_empty(), "{relative} wrote an empty trace");
        traces.push(bytes);
    }
    assert_ne!(
        traces[0], traces[1],
        "the signal timing plan must change the canonical trace at one seed"
    );
}

/// The checked-in spec and bank name the checked-in variants, the fidelity and
/// step, the ordered seeds, and the bounded sampling policy.
#[test]
fn the_experiment_spec_and_bank_name_the_checked_in_inputs() {
    let spec = experiment();
    assert_eq!(spec.experiment_version, 1);
    assert_eq!(spec.id, "increment6_signal_timing_v1");
    assert!(!spec.summary.is_empty());
    assert!(!spec.independent_variable.is_empty());
    assert!(!spec.controlled.is_empty());

    // The fidelity and step are a Phase 1 preset, and the ticks and duration
    // agree with each other.
    let preset = PRESETS
        .iter()
        .find(|preset| preset.name == spec.fidelity)
        .unwrap_or_else(|| panic!("unknown fidelity '{}'", spec.fidelity));
    assert_eq!(preset.step_s, spec.step_s, "the step must match the preset");
    assert_eq!(
        spec.duration_s,
        spec.ticks as f64 * spec.step_s,
        "the declared duration must be ticks at the declared step"
    );

    // Output stays bounded by the declared policy: the default samples
    // trajectories and keeps every event, and nothing opts into full ones.
    assert_eq!(
        spec.sampling,
        SamplingPolicy::default(),
        "the experiment runs the bounded default sampling policy"
    );

    // Both sides of the comparison are named, exist, and load.
    assert_eq!(spec.variants.len(), 2, "a paired comparison has two sides");
    assert_eq!(
        spec.variants
            .iter()
            .map(|variant| variant.scenario.as_str())
            .collect::<Vec<_>>(),
        vec![VARIANT_A, VARIANT_B],
        "the spec must name the checked-in variants"
    );
    let ids: Vec<String> = spec
        .variants
        .iter()
        .map(|variant| {
            assert_ne!(variant.variant, "", "each variant needs a label");
            load_scenario(&repo_path(&variant.scenario))
                .unwrap_or_else(|error| panic!("'{}' loads: {error}", variant.scenario))
                .id()
                .to_owned()
        })
        .collect();
    assert_ne!(ids[0], ids[1], "the two sides name different scenarios");

    // The seed bank names the ordered seeds both sides run.
    assert_eq!(spec.seed_bank, SEED_BANK);
    let bank = read_seed_bank(&repo_path(&spec.seed_bank)).expect("the seed bank is valid");
    assert_eq!(bank.bank.seed_bank_version, SEED_BANK_VERSION);
    assert_eq!(
        bank.bank.seeds,
        (1..=10).collect::<Vec<u64>>(),
        "the experiment runs one ordered bank of ten seeds"
    );
    assert_eq!(
        bank.content_sha256.len(),
        64,
        "the bank carries the identity a comparison proves both sides share"
    );
}

/// The checked-in spec's two variants run paired from the checked-in seed bank
/// through the CLI, and `compare` pairs them, so the whole Increment 5 chain
/// consumes these inputs without any source change.
#[test]
fn the_checked_in_experiment_runs_paired_from_the_seed_bank() {
    let spec = experiment();
    let scratch = Scratch::new("paired");
    let bank = repo_path(&spec.seed_bank);
    let bank_path = bank.to_str().expect("utf-8 path");
    let expected = read_seed_bank(&bank).expect("the seed bank is valid");

    let mut roots = Vec::new();
    for variant in &spec.variants {
        let root = scratch.path(&variant.variant);
        cli_ok(&[
            "batch",
            &variant.scenario,
            "--seed-bank",
            bank_path,
            "--out-root",
            root.to_str().expect("utf-8 path"),
            "--ticks",
            &CONTRACT_TICKS.to_string(),
            "--jobs",
            "4",
        ]);
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("batch.json"))
                .expect("the batch wrote its manifest"),
        )
        .expect("batch.json is valid JSON");
        assert_eq!(
            manifest["seeds"],
            serde_json::json!(expected.bank.seeds),
            "{} ran the bank's ordered seeds",
            variant.variant
        );
        assert_eq!(
            manifest["seed_bank"]["content_sha256"],
            serde_json::json!(expected.content_sha256),
            "{} recorded the bank it ran",
            variant.variant
        );
        roots.push(root);
    }

    let comparison = scratch.path("comparison.json");
    cli_ok(&[
        "compare",
        "--a",
        roots[0].to_str().expect("utf-8 path"),
        "--b",
        roots[1].to_str().expect("utf-8 path"),
        "--seed-bank",
        bank_path,
        "--output",
        comparison.to_str().expect("utf-8 path"),
    ]);
    let comparison: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&comparison).expect("compare wrote its artifact"),
    )
    .expect("comparison.json is valid JSON");
    assert_eq!(
        comparison["metric_definition_version"],
        METRIC_DEFINITION_VERSION
    );
    assert_eq!(
        comparison["seed_bank"]["content_sha256"],
        serde_json::json!(expected.content_sha256),
        "the comparison proves both sides ran one bank"
    );
    let paired: Vec<u64> = comparison["pairs"]
        .as_array()
        .expect("the comparison lists its pairs")
        .iter()
        .map(|pair| pair["seed"].as_u64().expect("a pair names its seed"))
        .collect();
    assert_eq!(
        paired, expected.bank.seeds,
        "every seed is paired by position"
    );
    assert!(
        !comparison["metrics"]
            .as_object()
            .expect("the comparison reports metrics")
            .is_empty(),
        "the comparison reported no metric"
    );
}

/// A geometrically different benchmark is expressible through scenario data
/// alone: it validates and runs through the same CLI, and its structure differs
/// from the two variants without any simulator change.
#[test]
fn a_geometrically_different_benchmark_needs_no_simulator_change() {
    let scratch = Scratch::new("offset");
    cli_ok(&["validate", OFFSET_JUNCTION]);
    let trace = scratch.path("offset.jsonl");
    cli_ok(&[
        "run",
        OFFSET_JUNCTION,
        "--seed",
        "1",
        "--ticks",
        &CONTRACT_TICKS.to_string(),
        "--output",
        trace.to_str().expect("utf-8 path"),
    ]);
    assert!(
        !std::fs::read(&trace)
            .expect("the run wrote a trace")
            .is_empty(),
        "the offset junction produced no trace"
    );

    let offset = load_scenario(&repo_path(OFFSET_JUNCTION)).expect("the offset junction loads");
    let variant = load_scenario(&repo_path(VARIANT_A)).expect("the variant loads");

    // Six arms instead of four, two staggered conflict regions instead of one
    // shared crossing, and no signal or pedestrian infrastructure: a different
    // layout, expressed from the same version 1 primitives.
    assert_eq!(offset.movements().len(), 3);
    assert_eq!(offset.conflict_regions().len(), 2);
    assert_eq!(offset.signals().len(), 0);
    assert_eq!(offset.crossings().len(), 0);
    assert_eq!(offset.demand().len(), 3);
    assert_eq!(variant.signals().len(), 1);
    assert_eq!(variant.conflict_regions().len(), 1);
    assert_ne!(
        offset.movements().len(),
        variant.movements().len(),
        "the benchmark must not share the variants' leg count"
    );

    // It runs as a benchmark: demand admits vehicles on the evaluated kernel
    // with no scenario-specific branch.
    let mut sim = Simulation::new(offset, RunConfig::new(1)).expect("the offset junction runs");
    let mut spawned = 0u32;
    for _ in 0..CONTRACT_TICKS {
        spawned += sim
            .step()
            .events()
            .iter()
            .filter(|event| matches!(event, Event::Spawned { .. }))
            .count() as u32;
    }
    assert!(spawned > 0, "the offset junction admitted no demand");
}
