//! Phase 1 Increment 3 slice C: pedestrian signal compliance.
//!
//! These tests use their own small scenarios rather than the checked-in
//! benchmark, so a benchmark change cannot silently change the kernel contract
//! they assert. They cover the fixed-time pedestrian signal state, the
//! contextual crossing decision and its boundaries, the physical possibility of
//! both outcomes under the waypoint controller, determinism, and stream
//! isolation from the vehicle mode.
//!
//! Vehicle yielding to an occupied crossing is a later slice; nothing here
//! asserts that a vehicle reacts to a pedestrian.

use tangle_model::{CompiledScenario, CrossingId, PathId, parse_scenario_source};
use tangle_sim::{
    AgentId, Event, PedestrianComplianceReason, PedestrianSignalAction, PedestrianSignalColor,
    RunConfig, Simulation, SnapshotDetail,
};

/// The fixed kernel step these tests assert bounds against.
const STEP_S: f64 = 0.05;

/// The route progress in metres of the crossing stop point: the crossing
/// region's centroid projects to the middle of the 40 m walking path.
const CROSSING_PROGRESS_M: f64 = 20.0;

/// The sampled constant walking speed in these scenarios.
const WALK_SPEED_MPS: f64 = 1.25;

/// One east-west road, one north-south walking path over it, a crossing with a
/// fixed-time pedestrian signal whose first 30 s are don't-walk and next 30 s
/// walk, a south-kerb waiting area, and one pedestrian route across the road.
/// `compliance` is the authored pedestrian compliance range and `rate_pph` the
/// pedestrian arrival rate.
fn crossing_scenario(compliance_min: f64, compliance_max: f64, rate_pph: f64) -> CompiledScenario {
    let text = format!(
        r#"{{
  schema_version: 1,
  id: 'pedestrian_signal_contract',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'road', points: [ {{ x: -40.0, y: 0.0 }}, {{ x: 40.0, y: 0.0 }} ] }},
    {{ id: 'walk', points: [ {{ x: 0.0, y: -20.0 }}, {{ x: 0.0, y: 20.0 }} ] }},
  ],
  portals: [
    {{ id: 'west', path: 'road', end: 'start', width_m: 7.0 }},
    {{ id: 'east', path: 'road', end: 'end', width_m: 7.0 }},
    {{ id: 'south', path: 'walk', end: 'start', width_m: 3.0 }},
    {{ id: 'north', path: 'walk', end: 'end', width_m: 3.0 }},
  ],
  regions: [
    {{ id: 'crossing_zone', points: [
      {{ x: -3.0, y: -3.0 }}, {{ x: 3.0, y: -3.0 }}, {{ x: 3.0, y: 3.0 }}, {{ x: -3.0, y: 3.0 }}
    ] }},
    {{ id: 'south_kerb', points: [
      {{ x: -3.0, y: -20.0 }}, {{ x: 3.0, y: -20.0 }}, {{ x: 3.0, y: -16.0 }}, {{ x: -3.0, y: -16.0 }}
    ] }},
  ],
  movements: [ {{ id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 }} ],
  crossings: [ {{ id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ],
    pedestrian_signal: {{ phases: [
      {{ duration_s: 30.0, walk: false }},
      {{ duration_s: 30.0, walk: true }},
    ] }} }} ],
  waiting_areas: [ {{ id: 'south_wait', region: 'south_kerb' }} ],
  pedestrian_routes: [ {{ id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
    crossings: [ 'road_crossing' ], waiting_areas: [ 'south_wait' ] }} ],
  pedestrian_demand: [ {{ id: 'footfall', portal: 'south', rate_pph: {rate_pph},
    routes: [ {{ route: 'cross_to_north', weight: 1.0 }} ] }} ],
  pedestrian_profiles: {{
    radius_m: {{ min: 0.25, max: 0.25 }},
    speed_mps: {{ min: 1.25, max: 1.25 }},
    compliance: {{ min: {compliance_min}, max: {compliance_max} }},
  }},
}}"#
    );
    let source = parse_scenario_source(&text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

/// The crossing scenario with road demand added, so vehicle arrivals can be
/// compared across pedestrian compliance settings.
fn mixed_scenario(compliance_min: f64, compliance_max: f64) -> CompiledScenario {
    let text = format!(
        r#"{{
  schema_version: 1,
  id: 'pedestrian_signal_mixed',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'road', points: [ {{ x: -40.0, y: 0.0 }}, {{ x: 40.0, y: 0.0 }} ] }},
    {{ id: 'walk', points: [ {{ x: 0.0, y: -20.0 }}, {{ x: 0.0, y: 20.0 }} ] }},
  ],
  portals: [
    {{ id: 'west', path: 'road', end: 'start', width_m: 7.0 }},
    {{ id: 'east', path: 'road', end: 'end', width_m: 7.0 }},
    {{ id: 'south', path: 'walk', end: 'start', width_m: 3.0 }},
    {{ id: 'north', path: 'walk', end: 'end', width_m: 3.0 }},
  ],
  regions: [
    {{ id: 'crossing_zone', points: [
      {{ x: -3.0, y: -3.0 }}, {{ x: 3.0, y: -3.0 }}, {{ x: 3.0, y: 3.0 }}, {{ x: -3.0, y: 3.0 }}
    ] }},
  ],
  movements: [ {{ id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 }} ],
  crossings: [ {{ id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ],
    pedestrian_signal: {{ phases: [
      {{ duration_s: 30.0, walk: false }},
      {{ duration_s: 30.0, walk: true }},
    ] }} }} ],
  pedestrian_routes: [ {{ id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
    crossings: [ 'road_crossing' ] }} ],
  demand: [ {{ id: 'road_inflow', portal: 'west', rate_vph: 600.0,
    routes: [ {{ movement: 'ew_through', weight: 1.0 }} ] }} ],
  pedestrian_demand: [ {{ id: 'footfall', portal: 'south', rate_pph: 720.0,
    routes: [ {{ route: 'cross_to_north', weight: 1.0 }} ] }} ],
  pedestrian_profiles: {{
    radius_m: {{ min: 0.25, max: 0.25 }},
    speed_mps: {{ min: 1.25, max: 1.25 }},
    compliance: {{ min: {compliance_min}, max: {compliance_max} }},
  }},
}}"#
    );
    let source = parse_scenario_source(&text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(scenario: CompiledScenario, seed: u64) -> Simulation {
    Simulation::new(scenario, RunConfig::new(seed)).expect("simulation builds")
}

/// One live pedestrian sampled from a full-detail snapshot.
struct WalkSample {
    id: AgentId,
    path_distance_m: f64,
    speed_mps: f64,
    decision: Option<tangle_sim::PedestrianComplianceDecision>,
}

/// Advance one tick, returning its pedestrian samples and its events.
fn tick(sim: &mut Simulation) -> (Vec<WalkSample>, Vec<Event>) {
    let events = sim.step().events().to_vec();
    let samples = sim
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref().expect("full detail");
            (motion.mode == tangle_sim::AgentMode::Pedestrian).then_some(WalkSample {
                id: sample.id,
                path_distance_m: motion.path_distance_m,
                speed_mps: motion.speed_mps,
                decision: motion.pedestrian_decision,
            })
        })
        .collect();
    (samples, events)
}

#[test]
fn the_crossing_signal_follows_its_fixed_time_phases() {
    let mut sim = sim(crossing_scenario(1.0, 1.0, 720.0), 1);
    let crossing = CrossingId::from_index(0);
    // The signal starts in its first, don't-walk phase. The exact boundary tick
    // is asserted on the compiled signal; here the phase clock is advanced by
    // the fixed step, so the checks stay a tick inside each interval.
    assert_eq!(
        sim.crossing_signal(crossing),
        Some(PedestrianSignalColor::DontWalk)
    );
    for _ in 0..599 {
        sim.step();
    }
    assert_eq!(sim.time().tick(), 599);
    assert_eq!(
        sim.crossing_signal(crossing),
        Some(PedestrianSignalColor::DontWalk),
        "the first 30 s are don't-walk"
    );
    for _ in 0..2 {
        sim.step();
    }
    assert_eq!(sim.time().tick(), 601);
    assert_eq!(
        sim.crossing_signal(crossing),
        Some(PedestrianSignalColor::Walk),
        "the next 30 s are walk"
    );
    // Advance past the 60 s cycle boundary, a few ticks inside the next cycle.
    for _ in 601..1205 {
        sim.step();
    }
    assert_eq!(
        sim.crossing_signal(crossing),
        Some(PedestrianSignalColor::DontWalk),
        "the cycle wraps back to don't-walk"
    );
}

#[test]
fn an_uncontrolled_crossing_has_no_pedestrian_signal() {
    // The layout with the pedestrian signal removed.
    let text = r#"{
      schema_version: 1, id: 'uncontrolled', coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'road', points: [ { x: -40, y: 0 }, { x: 40, y: 0 } ] } ],
      portals: [ { id: 'west', path: 'road', end: 'start', width_m: 7.0 },
                 { id: 'east', path: 'road', end: 'end', width_m: 7.0 } ],
      regions: [ { id: 'zone', points: [
        { x: -3, y: -3 }, { x: 3, y: -3 }, { x: 3, y: 3 }, { x: -3, y: 3 } ] } ],
      movements: [ { id: 'ew', from: 'west', to: 'east', path: 'road', priority: 0 } ],
      crossings: [ { id: 'cross', region: 'zone', movements: [ 'ew' ] } ],
      demand: [ { id: 'in', portal: 'west', rate_vph: 600.0,
        routes: [ { movement: 'ew', weight: 1.0 } ] } ],
    }"#;
    let source = parse_scenario_source(text).expect("parses");
    let mut sim = sim(CompiledScenario::compile(source).expect("compiles"), 1);
    sim.step();
    assert_eq!(sim.crossing_signal(CrossingId::from_index(0)), None);
}

#[test]
fn a_compliant_pedestrian_waits_before_a_forbidding_crossing_and_crosses_on_walk() {
    let mut sim = sim(crossing_scenario(1.0, 1.0, 720.0), 7);
    let crossing = CrossingId::from_index(0);

    let mut observed_wait = false;
    let mut observed_cross_on_walk = false;
    let mut despawns = 0u64;
    for _ in 0..1800 {
        let (samples, events) = tick(&mut sim);
        let signal = sim.crossing_signal(crossing);
        for event in &events {
            if matches!(event, Event::Despawned { .. }) {
                despawns += 1;
            }
        }
        for sample in &samples {
            let Some(decision) = sample.decision else {
                continue;
            };
            match decision.action {
                PedestrianSignalAction::Wait => {
                    observed_wait = true;
                    assert_eq!(decision.reason, PedestrianComplianceReason::CompliantWait);
                    assert_eq!(decision.signal, PedestrianSignalColor::DontWalk);
                    // A waiting pedestrian never reaches the crossing stop
                    // point and never exceeds its bounded walking speed.
                    assert!(
                        sample.path_distance_m <= CROSSING_PROGRESS_M + 1e-9,
                        "a waiting pedestrian reached {:.6} m, past the crossing at {CROSSING_PROGRESS_M}",
                        sample.path_distance_m
                    );
                    assert!(sample.speed_mps <= WALK_SPEED_MPS + 1e-9);
                }
                PedestrianSignalAction::Cross => {
                    if signal == Some(PedestrianSignalColor::Walk)
                        && sample.path_distance_m >= CROSSING_PROGRESS_M
                    {
                        observed_cross_on_walk = true;
                    }
                }
            }
        }
    }
    assert!(observed_wait, "no pedestrian waited for the crossing");
    assert!(
        observed_cross_on_walk,
        "no pedestrian crossed on the walk interval"
    );
    assert!(despawns > 0, "no pedestrian completed the crossing");
}

#[test]
fn a_noncompliant_pedestrian_crosses_against_a_forbidding_signal() {
    let mut sim = sim(crossing_scenario(0.0, 0.0, 720.0), 3);
    let crossing = CrossingId::from_index(0);

    let mut observed_choice = false;
    let mut crossed_against_signal = false;
    for _ in 0..1800 {
        let (samples, _) = tick(&mut sim);
        let signal = sim.crossing_signal(crossing);
        for sample in &samples {
            let Some(decision) = sample.decision else {
                continue;
            };
            if decision.reason == PedestrianComplianceReason::NonCompliantCross {
                observed_choice = true;
            }
            // Bounded speed for both outcomes, so a crossing is a physically
            // possible movement rather than a teleport.
            assert!(sample.speed_mps <= WALK_SPEED_MPS + 1e-9);
            if signal == Some(PedestrianSignalColor::DontWalk)
                && sample.path_distance_m >= CROSSING_PROGRESS_M
                && matches!(
                    decision.reason,
                    PedestrianComplianceReason::NonCompliantCross
                        | PedestrianComplianceReason::CannotStop
                )
            {
                crossed_against_signal = true;
            }
        }
    }
    assert!(
        observed_choice,
        "a zero-compliance pedestrian must record the noncompliant choice"
    );
    assert!(
        crossed_against_signal,
        "a noncompliant pedestrian must cross the forbidding signal"
    );
}

#[test]
fn every_pedestrian_step_stays_within_its_bounded_speed() {
    // The no-teleport check: on every tick, every pedestrian moves no farther
    // than its sampled speed times the fixed step, for both compliance beats.
    for (compliance_min, compliance_max) in [(1.0, 1.0), (0.0, 0.0)] {
        let mut sim = sim(crossing_scenario(compliance_min, compliance_max, 720.0), 5);
        let mut previous: Vec<(AgentId, f64)> = Vec::new();
        for _ in 0..1200 {
            let (samples, _) = tick(&mut sim);
            let mut current = Vec::new();
            for sample in &samples {
                if let Some((_, prior)) = previous.iter().find(|(id, _)| *id == sample.id) {
                    let step_m = (sample.path_distance_m - prior).abs();
                    assert!(
                        step_m <= WALK_SPEED_MPS * STEP_S + 1e-6,
                        "pedestrian {} moved {step_m:.6} m in one step",
                        sample.id.get()
                    );
                }
                current.push((sample.id, sample.path_distance_m));
            }
            previous = current;
        }
    }
}

#[test]
fn the_same_seed_reproduces_the_mixed_trace() {
    fn trace(scenario: CompiledScenario, seed: u64) -> Vec<String> {
        let mut sim = sim(scenario, seed);
        let mut frames = Vec::new();
        for _ in 0..1200 {
            sim.step();
            let snapshot = sim.snapshot(SnapshotDetail::Full);
            frames.push(format!("{:?}", snapshot.agents()));
        }
        frames
    }

    assert_eq!(
        trace(crossing_scenario(0.0, 1.0, 720.0), 9),
        trace(crossing_scenario(0.0, 1.0, 720.0), 9)
    );
}

#[test]
fn pedestrian_compliance_does_not_perturb_vehicle_arrivals() {
    // Two scenarios differ only in the pedestrian compliance range, so their
    // pedestrians wait or cross differently. Vehicle arrivals depend only on
    // the per-source `demand` stream and the road path clearance, so the road
    // arrival trace is byte-identical: the pedestrian compliance draw under a
    // pedestrian agent id cannot perturb the vehicle streams.
    fn road_arrivals(compliance_min: f64, compliance_max: f64, seed: u64) -> Vec<(u64, f64)> {
        let mut sim = sim(mixed_scenario(compliance_min, compliance_max), seed);
        let mut record = Vec::new();
        for _ in 0..1200 {
            let tick = sim.time().tick();
            for event in sim.step().events() {
                if let Event::Spawned {
                    path, distance_m, ..
                } = event
                    && *path == PathId::from_index(0)
                {
                    record.push((tick, *distance_m));
                }
            }
        }
        record
    }

    let with_noncompliant = road_arrivals(0.0, 0.0, 11);
    assert!(
        !with_noncompliant.is_empty(),
        "no vehicle arrivals recorded"
    );
    assert_eq!(with_noncompliant, road_arrivals(1.0, 1.0, 11));
}
