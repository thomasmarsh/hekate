//! TAS-110: the Increment 2 performance-profile counters.
//!
//! The kernel exposes additive, non-behavioural counters for the representative
//! mixed-mode profile: the broad-phase candidate volume and the tactical
//! predictor's evaluation and candidate volume, read through
//! [`Simulation::performance_counters`]. No decision, event, trace byte, or
//! metric reads them and nothing serializes them, so a run is identical with
//! and without them.
//!
//! [`counters_advance_without_changing_the_run`] is the non-release proof that
//! the counters move on the checked-in profile and that a run stays
//! deterministic. [`increment_2_profile_counters`] is the `#[ignore]` release
//! measurement that records the bounded per-agent-step ratios and the
//! lateral-disabled ablation the profile reads:
//!
//! ```sh
//! cargo test --release -p hekate-sim --test performance_counters -- --ignored --nocapture
//! ```

use std::time::Instant;

use hekate_model::{CompiledScenario, parse_scenario_source_v2};
use hekate_sim::{PerformanceCounters, RunConfig, Simulation};

/// The checked-in Increment 2 representative mixed-mode profile.
const PROFILE: &str = include_str!("../../../scenarios/phase2/inc2/mixed_mode_profile_v2.json5");
/// Its lateral-disabled ablation twin.
const NO_LATERAL: &str =
    include_str!("../../../scenarios/phase2/inc2/mixed_mode_profile_v2_no_lateral.json5");

/// The fixture's seed bank entry the profile measures on.
const SEED: u64 = 11;

/// The fixture's simulation, built from its checked-in text.
fn build(text: &str) -> Simulation {
    let source = parse_scenario_source_v2(text).expect("the fixture is a version 2 document");
    let compiled = CompiledScenario::compile_v2(source).expect("the fixture compiles");
    Simulation::new(compiled, RunConfig::new(SEED)).expect("the simulation builds")
}

/// One bounded run: the elapsed wall time, the agent steps, and the counters.
struct Run {
    ticks: u64,
    agent_steps: u64,
    wall_secs: f64,
    counters: PerformanceCounters,
}

/// Step `sim` for `ticks`, summing live agents before each step as the release
/// harness does, with the diagnostic counters reset first.
fn measure(sim: &mut Simulation, ticks: u64) -> Run {
    Simulation::reset_performance_counters();
    let mut agent_steps = 0u64;
    let start = Instant::now();
    for _ in 0..ticks {
        agent_steps += sim.agent_count() as u64;
        sim.step();
    }
    let wall_secs = start.elapsed().as_secs_f64();
    Run {
        ticks,
        agent_steps,
        wall_secs,
        counters: Simulation::performance_counters(),
    }
}

fn per_step(total: u64, agent_steps: u64) -> f64 {
    if agent_steps == 0 {
        0.0
    } else {
        total as f64 / agent_steps as f64
    }
}

/// Short window whose counters must be non-zero: by four simulated minutes the
/// profile has admitted cars that have caught the slower bicycle and scooter
/// leaders, so the predictor and the broad phase both move.
const FOCUSED_TICKS: u64 = 2000;

/// The shorter fresh window the determinism check compares over.
const DETERMINISM_TICKS: u64 = 500;

#[test]
fn counters_advance_without_changing_the_run() {
    let mut sim = build(PROFILE);
    Simulation::reset_performance_counters();
    for _ in 0..FOCUSED_TICKS {
        sim.step();
    }
    let counters = Simulation::performance_counters();
    assert!(
        counters.broad_phase_candidates > 0,
        "the broad phase must return candidates on the profile: {counters:?}"
    );
    assert!(
        counters.predictions > 0,
        "the predictor must evaluate candidates on the profile: {counters:?}"
    );
    assert!(
        counters.prediction_candidates >= counters.predictions,
        "each prediction scans at least its own obstacle set: {counters:?}"
    );

    // The counters are inert: two fresh runs of the same short window produce
    // the same event count and the same agent-step count, so reading them
    // changes no decision. (The checked-in fixture tests are the byte-level
    // proof that the kernel's event stream is unchanged.)
    let first = event_totals(DETERMINISM_TICKS);
    let second = event_totals(DETERMINISM_TICKS);
    assert_eq!(first, second, "the profile run must stay deterministic");
    assert!(first.0 > 0, "the run must emit events");
}

/// The (events, agent steps) of a fresh run over `ticks`, for the determinism
/// check.
fn event_totals(ticks: u64) -> (usize, u64) {
    let mut sim = build(PROFILE);
    let mut events = 0usize;
    let mut steps = 0u64;
    for _ in 0..ticks {
        steps += sim.agent_count() as u64;
        events += sim.step().events().len();
    }
    (events, steps)
}

/// The bounded window the release measurement uses, well inside the flowing
/// regime the profile documents (the corridor reaches a bounded population).
const MEASURED_TICKS: u64 = 5000;
/// Warm-up ticks run from a fresh simulation the measurement does not time.
const WARMUP_TICKS: u64 = 500;
/// Independent timed passes per fixture, so the spread is the same shape the
/// release harness reports and no single pass is a budget.
const REPEATS: usize = 3;

fn warmed(text: &str) -> Simulation {
    let mut sim = build(text);
    for _ in 0..WARMUP_TICKS {
        sim.step();
    }
    sim
}

/// The min and median of a slice, the two statistics the release report quotes.
fn min_median(values: &[f64]) -> (f64, f64) {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    (sorted[0], sorted[sorted.len() / 2])
}

fn us_per_tick(run: &Run) -> f64 {
    run.wall_secs * 1e6 / run.ticks as f64
}

#[test]
#[ignore = "release-mode wall-clock measurement; run with --release --ignored"]
fn increment_2_profile_counters() {
    let enabled: Vec<Run> = (0..REPEATS)
        .map(|_| measure(&mut warmed(PROFILE), MEASURED_TICKS))
        .collect();
    let disabled: Vec<Run> = (0..REPEATS)
        .map(|_| measure(&mut warmed(NO_LATERAL), MEASURED_TICKS))
        .collect();

    // The counters are deterministic, so every pass agrees; assert that and
    // read them once.
    let enabled_counters = enabled[0].counters;
    let disabled_counters = disabled[0].counters;
    assert!(
        enabled.iter().all(|run| run.counters == enabled_counters),
        "the enabled counters must be identical across passes"
    );
    assert!(
        disabled.iter().all(|run| run.counters == disabled_counters),
        "the disabled counters must be identical across passes"
    );

    for (label, runs, counters) in [
        ("enabled", &enabled, &enabled_counters),
        ("no_lateral", &disabled, &disabled_counters),
    ] {
        let us: Vec<f64> = runs.iter().map(us_per_tick).collect();
        let (min, median) = min_median(&us);
        let steps = runs[0].agent_steps;
        println!(
            "{label}: ticks={MEASURED_TICKS} us_per_tick_min={min:.1} us_per_tick_median={median:.1} \
             us_per_tick_passes={us:?} agent_steps={steps} agent_steps_per_tick={:.3} \
             broad_phase_candidates={} bpc_per_agent_step={:.3} predictions={} \
             predictions_per_agent_step={:.3} prediction_candidates={} pc_per_agent_step={:.3}",
            steps as f64 / MEASURED_TICKS as f64,
            counters.broad_phase_candidates,
            per_step(counters.broad_phase_candidates, steps),
            counters.predictions,
            per_step(counters.predictions, steps),
            counters.prediction_candidates,
            per_step(counters.prediction_candidates, steps),
        );
    }

    let (enabled_min, enabled_median) =
        min_median(&enabled.iter().map(us_per_tick).collect::<Vec<_>>());
    let (disabled_min, disabled_median) =
        min_median(&disabled.iter().map(us_per_tick).collect::<Vec<_>>());
    println!(
        "ablation: enabled_min={enabled_min:.1} enabled_median={enabled_median:.1} \
         disabled_min={disabled_min:.1} disabled_median={disabled_median:.1} \
         lateral_share_median={:.1}%",
        (enabled_median - disabled_median) / enabled_median * 100.0,
    );

    assert!(enabled_counters.predictions > 0);
    assert!(
        enabled_counters.predictions > disabled_counters.predictions,
        "the lateral profile must run more prediction than its ablation"
    );
}
