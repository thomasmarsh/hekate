//! Phase 1 Increment 3 slice D: vehicle yielding to an occupied crossing.
//!
//! These tests cover the shared spatial index, the shared `yield` rule, the
//! shared `Event::Yielded` transition, and the slice-B residual: under the
//! mixed benchmark, vehicle-versus-pedestrian overlaps must be eliminated and
//! the run must not deadlock. The checked-in `pedestrian_crossing_v1` benchmark
//! authors a `yield` rule on its road movement and is measured directly, so the
//! acceptance test cannot drift from the benchmark; the residual before the
//! rule was authored is measured by removing that one rule from the same text.
//!
//! The benchmark's crossing region is the rectangle `x in [-2, 2]` over the
//! east-west road, so a forward vehicle reaches its entry at `x = -2` and
//! yields just short of it.

use std::collections::BTreeMap;

use glam::DVec2;
use tangle_model::{CompiledScenario, CrossingId, parse_scenario_source};
use tangle_sim::{AgentMode, AgentSample, Event, RunConfig, Simulation, SnapshotDetail};

/// The fixed kernel step these tests assert bounds against.
const STEP_S: f64 = 0.05;

/// The checked-in mixed benchmark, measured rather than duplicated.
const BENCHMARK: &str = include_str!("../../../scenarios/benchmarks/pedestrian_crossing_v1.json5");

/// The benchmark's crossing entry along the road, in metres on the x axis.
///
/// The crossing region spans `x in [-2, 2]`, so a forward vehicle (entering at
/// the path start) reaches the entry at `x = -2`; a backward vehicle (entering at
/// the path end) reaches it at `x = +2`.
const CROSSING_ENTRY_X: f64 = -2.0;
const BACKWARD_CROSSING_ENTRY_X: f64 = 2.0;

fn scenario(text: &str) -> CompiledScenario {
    let source = parse_scenario_source(text).expect("scenario parses");
    CompiledScenario::compile(source).expect("compiles")
}

fn sim(text: &str, seed: u64) -> Simulation {
    Simulation::new(scenario(text), RunConfig::new(seed)).expect("simulation builds")
}

/// The benchmark text with its authored `yield` rule removed, so the same run
/// measures the residual that existed before yielding was authored.
fn without_yield_rule() -> String {
    const RULE_BLOCK: &str = "  rules: [\n    { id: 'yield_to_road_crossing', movement: 'ew_through', kind: 'yield' },\n  ],\n";
    let text = BENCHMARK.replace(RULE_BLOCK, "");
    assert!(
        text.len() < BENCHMARK.len(),
        "the benchmark's yield rule block was not found verbatim"
    );
    text
}

/// The benchmark with its road movement reversed, so vehicles enter at the path
/// end and travel backward (`direction = -1`) onto the same crossing.
///
/// `movement_entry` sends a movement backward when its `from` portal is on the
/// path end. Reversing only the movement's portals and its demand portal leaves
/// every other authored primitive (the crossing, its pedestrian signal, the
/// pedestrian route, and the `yield` rule) byte-identical, so the same fixture
/// exercises the yield path in the backward direction.
fn backward_benchmark() -> String {
    let text = BENCHMARK
        .replace(
            "{ id: 'ew_through', from: 'west_entry', to: 'east_exit', path: 'ew_road', priority: 0 }",
            "{ id: 'ew_through', from: 'east_exit', to: 'west_entry', path: 'ew_road', priority: 0 }",
        )
        .replace("portal: 'west_entry',", "portal: 'east_exit',");
    assert_eq!(
        text.matches("from: 'east_exit'").count(),
        1,
        "the benchmark's road movement was not reversed verbatim"
    );
    assert_eq!(
        text.matches("portal: 'east_exit',").count(),
        1,
        "the benchmark's vehicle demand portal was not redirected to the path end"
    );
    text
}

/// Live agent samples from one snapshot.
fn agents(sim: &Simulation) -> Vec<AgentSample> {
    sim.snapshot(SnapshotDetail::Full).agents().to_vec()
}

/// Surface clearance in metres between a circle body and an oriented box body.
///
/// Negative means the circle overlaps the box. This duplicates the slice-B
/// contract's exact circle-versus-box distance so the residual is measured the
/// same way in both places.
fn circle_box_clearance(
    circle_m: DVec2,
    radius_m: f64,
    centre_m: DVec2,
    heading_rad: f64,
    length_m: f64,
    width_m: f64,
) -> f64 {
    let (sin, cos) = heading_rad.sin_cos();
    let offset = circle_m - centre_m;
    let local = DVec2::new(
        offset.x * cos + offset.y * sin,
        -offset.x * sin + offset.y * cos,
    );
    let clamped = DVec2::new(
        local.x.clamp(-length_m * 0.5, length_m * 0.5),
        local.y.clamp(-width_m * 0.5, width_m * 0.5),
    );
    (local - clamped).length() - radius_m
}

/// One run's observable outcome, gathered as the run advances.
struct RunReport {
    /// Overlapping vehicle-pedestrian pairs observed at a tick end.
    overlaps: u32,
    /// Overlaps that appeared because a vehicle moved onto a pedestrian that was
    /// not itself stepping into the vehicle; these are the slice-B residual.
    vehicle_initiated: u32,
    /// Vehicles that ever reported yielding, with the greatest front-bumper x
    /// reached while yielding.
    yielding_vehicles: BTreeMap<u32, f64>,
    /// Every begin and end yield transition, in emission order.
    yield_events: Vec<Event>,
    /// Ticks at which each pedestrian was first observed.
    first_seen: BTreeMap<u32, u64>,
    /// Whether a vehicle and a pedestrian were each observed.
    saw_vehicle: bool,
    saw_pedestrian: bool,
    /// Whether any observed agent state was non-finite.
    non_finite: bool,
    /// Live agent ids at the end.
    live: Vec<u32>,
    /// Vehicles that braked beyond their comfortable bound; each must be a
    /// counted emergency-cap step.
    over_comfort_brakes: u64,
    dropped: u64,
    spawned: u64,
    despawned: u64,
}

/// Run a scenario and gather the slice-D observables.
fn run_report(text: &str, seed: u64, ticks: u64) -> RunReport {
    let mut sim = sim(text, seed);
    let mut report = RunReport {
        overlaps: 0,
        vehicle_initiated: 0,
        yielding_vehicles: BTreeMap::new(),
        yield_events: Vec::new(),
        first_seen: BTreeMap::new(),
        saw_vehicle: false,
        saw_pedestrian: false,
        non_finite: false,
        live: Vec::new(),
        over_comfort_brakes: 0,
        dropped: 0,
        spawned: 0,
        despawned: 0,
    };
    let mut previous_speed: BTreeMap<u32, f64> = BTreeMap::new();
    for _ in 0..ticks {
        let tick = sim.time().tick();
        let caps_before = sim.emergency_cap_steps();
        let before = agents(&sim);
        let events: Vec<Event> = sim.step().events().to_vec();
        let cap_engaged = sim.emergency_cap_steps() > caps_before;
        report.yield_events.extend(
            events
                .iter()
                .filter(|event| matches!(event, Event::Yielded { .. }))
                .cloned(),
        );
        let after = agents(&sim);
        for sample in &after {
            let motion = sample.motion.as_ref().expect("full detail");
            if !(sample.position.is_finite()
                && sample.heading_rad.is_finite()
                && motion.speed_mps.is_finite())
            {
                report.non_finite = true;
            }
            match motion.mode {
                AgentMode::Vehicle => report.saw_vehicle = true,
                AgentMode::Pedestrian => {
                    report.saw_pedestrian = true;
                    report.first_seen.entry(sample.id.get()).or_insert(tick);
                }
            }
            if sim.agent_yield_crossing(sample.id).is_some() {
                // A yielding vehicle holds its front bumper clear of the
                // crossing entry, so its whole body stays out of the region.
                let front = sample.position.x + motion.body_length_m * 0.5;
                let deepest = report
                    .yielding_vehicles
                    .entry(sample.id.get())
                    .or_insert(f64::NEG_INFINITY);
                *deepest = deepest.max(front);
                assert!(
                    front <= CROSSING_ENTRY_X + 1e-6,
                    "agent {} yielded with its front bumper at {front}, past the entry",
                    sample.id.get()
                );
            }
            if motion.mode == AgentMode::Vehicle {
                let profile = sim
                    .agent_profile(sample.id)
                    .expect("a demand vehicle has a profile");
                if let Some(&previous) = previous_speed.get(&sample.id.get()) {
                    let change = motion.speed_mps - previous;
                    if change < -profile.comfortable_brake_mps2 * STEP_S - 1e-9 {
                        assert!(
                            cap_engaged,
                            "agent {} braked beyond its comfort bound without a counted cap step",
                            sample.id.get()
                        );
                        report.over_comfort_brakes += 1;
                    }
                }
                previous_speed.insert(sample.id.get(), motion.speed_mps);
            }
            if motion.mode != AgentMode::Pedestrian {
                continue;
            }
            let radius_m = motion.body_length_m * 0.5;
            for other in &after {
                let other_motion = other.motion.as_ref().expect("full detail");
                if other_motion.mode != AgentMode::Vehicle {
                    continue;
                }
                let clearance = circle_box_clearance(
                    sample.position,
                    radius_m,
                    other.position,
                    other.heading_rad,
                    other_motion.body_length_m,
                    other_motion.body_width_m,
                );
                if clearance >= 0.0 {
                    continue;
                }
                report.overlaps += 1;
                let (Some(previous_self), Some(previous_other)) = (
                    before.iter().find(|prior| prior.id == sample.id),
                    before.iter().find(|prior| prior.id == other.id),
                ) else {
                    continue;
                };
                let clearance_before = circle_box_clearance(
                    previous_self.position,
                    radius_m,
                    previous_other.position,
                    previous_other.heading_rad,
                    other_motion.body_length_m,
                    other_motion.body_width_m,
                );
                if clearance_before < 0.0 {
                    continue;
                }
                // The pedestrian stepped into the vehicle's pre-step body if its
                // new position already overlapped where the vehicle was;
                // anything else is the vehicle moving onto the pedestrian.
                let stepped_into_it = circle_box_clearance(
                    sample.position,
                    radius_m,
                    previous_other.position,
                    previous_other.heading_rad,
                    other_motion.body_length_m,
                    other_motion.body_width_m,
                ) < 0.0;
                if !stepped_into_it {
                    report.vehicle_initiated += 1;
                }
            }
        }
    }
    report.live = agents(&sim).iter().map(|sample| sample.id.get()).collect();
    let summary = sim.finish();
    report.dropped = summary.dropped();
    report.spawned = summary.spawned();
    report.despawned = summary.despawned();
    report
}

#[test]
fn a_vehicle_holds_before_an_occupied_crossing_and_resumes() {
    let report = run_report(BENCHMARK, 0, 4000);
    assert!(
        !report.yielding_vehicles.is_empty(),
        "no vehicle ever yielded to the crossing"
    );
    // Every yielding vehicle held its front bumper clear of the crossing entry,
    // which the report asserts per step; nothing reported it inside the region.
    let begins: Vec<Event> = report
        .yield_events
        .iter()
        .filter(|event| matches!(event, Event::Yielded { yielding: true, .. }))
        .cloned()
        .collect();
    assert!(!begins.is_empty(), "no yield began");
    // Resumption: everyone seen well before the end is through, so no vehicle
    // stayed stopped at the crossing.
    for (id, tick) in &report.first_seen {
        if *tick < 3000 {
            assert!(
                !report.live.contains(id),
                "agent {id} never resumed from its yield"
            );
        }
    }
    assert_eq!(report.dropped, 0);
}

#[test]
fn yielding_emits_a_begin_and_an_end_transition_per_agent() {
    let report = run_report(BENCHMARK, 1, 4000);
    let mut yielding: BTreeMap<u32, bool> = BTreeMap::new();
    let mut begins = 0u64;
    let mut ends = 0u64;
    for event in &report.yield_events {
        let Event::Yielded {
            agent,
            crossing,
            yielding: now,
        } = event
        else {
            continue;
        };
        assert_eq!(*crossing, CrossingId::from_index(0));
        let was = yielding.insert(agent.get(), *now).unwrap_or(false);
        assert_ne!(was, *now, "agent {} repeated a yield state", agent.get());
        if *now {
            begins += 1;
        } else {
            ends += 1;
        }
    }
    assert!(begins > 0, "no vehicle began yielding");
    assert_eq!(begins, ends, "every begun yield must also end");
}

#[test]
fn yielding_stays_within_the_comfort_bound_except_a_counted_cap_step() {
    // `run_report` asserts, per step, that any deceleration beyond a vehicle's
    // comfortable braking is a counted emergency-cap step. The comfortable
    // bound stays the normal regime: only a small minority of steps invoke the
    // backstop, mirroring the stop-line and queue precedent.
    let report = run_report(BENCHMARK, 2, 4000);
    assert!(
        report.over_comfort_brakes < 4000 / 10,
        "the yield cap was the normal braking regime ({} steps)",
        report.over_comfort_brakes
    );
}

#[test]
fn the_mixed_benchmark_has_no_vehicle_pedestrian_overlap_or_deadlock() {
    for seed in 0..6u64 {
        let report = run_report(BENCHMARK, seed, 4000);
        assert!(!report.non_finite, "seed {seed}: non-finite agent state");
        assert!(
            report.saw_vehicle && report.saw_pedestrian,
            "seed {seed}: a mode was absent"
        );
        assert_eq!(
            report.overlaps, 0,
            "seed {seed}: a vehicle and a pedestrian overlapped"
        );
        assert_eq!(
            report.vehicle_initiated, 0,
            "seed {seed}: a vehicle moved onto a pedestrian"
        );
        for (id, tick) in &report.first_seen {
            if *tick < 3000 {
                assert!(
                    !report.live.contains(id),
                    "seed {seed}: agent {id} never left its route (route deadlock)"
                );
            }
        }
        assert_eq!(report.dropped, 0, "seed {seed}: nominal demand was shed");
        assert!(report.spawned > 0 && report.despawned > 0);
    }
}

#[test]
fn the_yield_rule_removes_the_slice_b_vehicle_overlap_residual() {
    let without = without_yield_rule();
    // Without the rule the benchmark still shows vehicle-initiated overlaps;
    // with it, none remain. Both runs use identical scenario text otherwise.
    let mut before_vehicle = 0u32;
    let mut before_overlaps = 0u32;
    let mut after_vehicle = 0u32;
    let mut after_overlaps = 0u32;
    for seed in 0..6u64 {
        let without_report = run_report(&without, seed, 2000);
        before_overlaps += without_report.overlaps;
        before_vehicle += without_report.vehicle_initiated;
        let with_report = run_report(BENCHMARK, seed, 2000);
        after_overlaps += with_report.overlaps;
        after_vehicle += with_report.vehicle_initiated;
    }
    assert!(
        before_vehicle > 0,
        "the ruleless benchmark must still exhibit the residual"
    );
    assert_eq!(
        after_vehicle, 0,
        "yielding must remove every vehicle-initiated overlap"
    );
    assert_eq!(after_overlaps, 0, "yielding must remove every overlap");
    assert!(before_overlaps > after_overlaps);
}

#[test]
fn the_mixed_yielding_run_reproduces_for_the_same_seed() {
    fn trace(seed: u64) -> Vec<String> {
        let mut sim = sim(BENCHMARK, seed);
        let mut frames = Vec::new();
        for _ in 0..600 {
            let events: Vec<Event> = sim.step().events().to_vec();
            let mut frame = format!("{events:?}");
            for sample in agents(&sim) {
                let motion = sample.motion.as_ref().expect("full detail");
                frame.push_str(&format!(
                    " {}:{:?}:{:.12}:{:?}",
                    sample.id.get(),
                    sample.position,
                    motion.speed_mps,
                    sim.agent_yield_crossing(sample.id).map(CrossingId::get),
                ));
            }
            frames.push(frame);
        }
        frames
    }
    assert_eq!(trace(7), trace(7));
    assert_ne!(trace(7), trace(8), "different seeds diverge");
}

#[test]
fn a_backward_vehicle_brakes_before_an_occupied_crossing() {
    // A movement that enters at the path end travels backward. The yield path
    // must use one progress convention: when the entry was measured as travel
    // progress while the vehicle front used the signed convention, the gap was
    // a whole path length too large for `direction = -1`, so a backward vehicle
    // emitted `Event::Yielded` and reported yielding without ever braking. This
    // runs the same benchmark with only the road movement reversed.
    let text = backward_benchmark();
    let mut saw_yield = false;
    let mut min_yielding_speed = f64::INFINITY;
    for seed in 0..6u64 {
        let mut sim = sim(&text, seed);
        for _ in 0..4000 {
            let events: Vec<Event> = sim.step().events().to_vec();
            saw_yield |= events
                .iter()
                .any(|event| matches!(event, Event::Yielded { yielding: true, .. }));
            for sample in agents(&sim) {
                let motion = sample.motion.as_ref().expect("full detail");
                if motion.mode != AgentMode::Vehicle
                    || sim.agent_yield_crossing(sample.id).is_none()
                {
                    continue;
                }
                // The leading bumper travels toward smaller x, so it reaches the
                // region's +x face; it must hold clear of it.
                let front = sample.position.x - motion.body_length_m * 0.5;
                assert!(
                    front >= BACKWARD_CROSSING_ENTRY_X - 1e-6,
                    "seed {seed}: agent {} yielded with its front bumper at {front}, \
                     past the crossing entry",
                    sample.id.get()
                );
                min_yielding_speed = min_yielding_speed.min(motion.speed_mps);
            }
        }
    }
    assert!(saw_yield, "no backward vehicle ever yielded");
    assert!(
        min_yielding_speed < 1.0,
        "a backward yielding vehicle must brake to a stop before the crossing, \
         lowest yielding speed {min_yielding_speed} m/s"
    );
}

#[test]
fn an_unobliged_movement_never_yields() {
    // The same benchmark without the `yield` rule never emits a yield or reports
    // a yielding vehicle, so yielding is authored scenario data rather than a
    // simulator branch.
    let without = without_yield_rule();
    let mut sim = sim(&without, 3);
    let mut yield_events = 0u64;
    for _ in 0..4000 {
        let events: Vec<Event> = sim.step().events().to_vec();
        yield_events += events
            .iter()
            .filter(|event| matches!(event, Event::Yielded { .. }))
            .count() as u64;
        for sample in agents(&sim) {
            assert_eq!(sim.agent_yield_crossing(sample.id), None);
        }
    }
    assert_eq!(yield_events, 0);
}
