//! Phase 1 Increment 2 slice B: IDM longitudinal control, leader following,
//! queueing, exit, authored stop lines, and the fixed-time signal phase machine.
//!
//! These tests author their own small scenarios so a checked-in benchmark
//! change cannot silently move the kernel contract they assert. The
//! controlled-following bound and non-overlap evidence for the checked-in
//! benchmark lives in `apps/hekate-cli/tests/scenarios.rs`.

use hekate_model::{CompiledScenario, MovementId, SignalColor, parse_scenario_source};
use hekate_sim::{Event, RunConfig, Simulation, SnapshotDetail, VehicleProfile};

/// An 80 m signalized approach whose movement stops at an authored 34 m stop
/// line. The first 12 s are red, then 20 s green; the cycle repeats.
const SIGNALIZED: &str = r#"
{
  schema_version: 1,
  id: 'signalized_stop_line',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  movements: [
    { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0, stop_line_m: 34.0 },
  ],
  rules: [ { id: 'r_through', movement: 'through', kind: 'signal', signal: 'main' } ],
  signals: [ { id: 'main',
    heads: [ { id: 'head', movement: 'through' } ],
    phases: [
      { duration_s: 12.0, states: [ { head: 'head', color: 'red' } ] },
      { duration_s: 20.0, states: [ { head: 'head', color: 'green' } ] },
    ] } ],
  demand: [
    { id: 'inflow', portal: 'entry', rate_vph: 1800.0,
      routes: [ { movement: 'through', weight: 1.0 } ] },
  ],
  profiles: {
    speed_mps: { min: 10.0, max: 10.0 },
    length_m: { min: 4.0, max: 4.0 },
    width_m: { min: 2.0, max: 2.0 },
    time_gap_s: { min: 1.5, max: 1.5 },
    max_accel_mps2: { min: 2.0, max: 2.0 },
    comfortable_brake_mps2: { min: 3.0, max: 3.0 },
  },
}
"#;

/// A free-flowing straight approach with no signal or stop line.
const FREE_FLOW: &str = r#"
{
  schema_version: 1,
  id: 'free_flow',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  movements: [
    { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 },
  ],
  demand: [
    { id: 'inflow', portal: 'entry', rate_vph: 720.0,
      routes: [ { movement: 'through', weight: 1.0 } ] },
  ],
  profiles: {
    speed_mps: { min: 10.0, max: 10.0 },
    length_m: { min: 4.0, max: 4.0 },
    width_m: { min: 2.0, max: 2.0 },
    time_gap_s: { min: 1.5, max: 1.5 },
    max_accel_mps2: { min: 2.0, max: 2.0 },
    comfortable_brake_mps2: { min: 3.0, max: 3.0 },
  },
}
"#;

fn scenario(text: &str) -> CompiledScenario {
    let source = parse_scenario_source(text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(text: &str, seed: u64) -> Simulation {
    Simulation::new(scenario(text), RunConfig::new(seed)).expect("simulation builds")
}

/// Step to the exact tick and return the full snapshot's agent states.
fn run_to(sim: &mut Simulation, tick: u64) -> Vec<(u32, f64, f64, f64)> {
    while sim.time().tick() < tick {
        sim.step();
    }
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .map(|agent| {
            let motion = agent.motion.as_ref().expect("full detail");
            (
                agent.id.get(),
                motion.path_distance_m,
                motion.speed_mps,
                motion.body_length_m,
            )
        })
        .collect()
}

#[test]
fn signal_phases_advance_through_green_yellow_red_and_wrap() {
    let mut sim = sim(SIGNALIZED, 1);
    let movement = MovementId::from_index(0);
    // t = 0 .. 12 s is red, 12 .. 32 s is green, then the cycle wraps to red.
    assert_eq!(sim.movement_signal(movement), Some(SignalColor::Red));
    run_to(&mut sim, 239);
    assert_eq!(sim.movement_signal(movement), Some(SignalColor::Red));
    run_to(&mut sim, 240);
    assert_eq!(sim.movement_signal(movement), Some(SignalColor::Green));
    run_to(&mut sim, 639);
    assert_eq!(sim.movement_signal(movement), Some(SignalColor::Green));
    run_to(&mut sim, 640);
    assert_eq!(
        sim.movement_signal(movement),
        Some(SignalColor::Red),
        "the fixed-time cycle wraps"
    );
}

#[test]
fn a_vehicle_comes_to_rest_at_its_authored_stop_line() {
    let mut sim = sim(SIGNALIZED, 1);
    let stop_line_m = 34.0;
    // Just before the red phase ends, the lead vehicle is held at the line.
    let agents = run_to(&mut sim, 239);
    let lead = agents
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("a vehicle queued at the stop line");
    let front_m = lead.1 + lead.3 * 0.5;
    assert!(
        (front_m - stop_line_m).abs() <= 0.5,
        "front bumper {front_m} did not rest at the stop line {stop_line_m}"
    );
    assert!(
        lead.2 < 0.25,
        "the held vehicle should be at rest, got {} m/s",
        lead.2
    );
}

#[test]
fn a_red_signal_queues_vehicles_behind_the_stop_line_without_overlap() {
    let mut sim = sim(SIGNALIZED, 1);
    let stop_line_m = 34.0;
    let agents = run_to(&mut sim, 239);
    let queued = agents
        .iter()
        .filter(|agent| agent.2 < 1.0 && agent.1 <= stop_line_m)
        .count();
    assert!(
        queued >= 2,
        "expected a queue, found {queued} held vehicles"
    );
    for (index, first) in agents.iter().enumerate() {
        for second in &agents[index + 1..] {
            let half_lengths = (first.3 + second.3) * 0.5;
            let gap = (first.1 - second.1).abs();
            assert!(
                gap >= half_lengths - 1e-9,
                "queued bodies overlap: gap {gap} < {half_lengths}"
            );
        }
    }
}

#[test]
fn a_green_signal_releases_the_queue_and_the_vehicles_exit() {
    let mut sim = sim(SIGNALIZED, 1);
    let stop_line_m = 34.0;
    run_to(&mut sim, 239);
    // After the green phase starts the queue clears the line.
    run_to(&mut sim, 500);
    let agents = sim.snapshot(SnapshotDetail::Full).agents().to_vec();
    let beyond = agents
        .iter()
        .filter(|agent| agent.motion.as_ref().expect("full detail").path_distance_m > stop_line_m)
        .count();
    assert!(beyond > 0, "no vehicle crossed the stop line on green");

    // Everything eventually leaves the 80 m path.
    let mut exited = 0u64;
    for _ in 0..800 {
        for event in sim.step().events() {
            if let Event::Despawned { .. } = event {
                exited += 1;
            }
        }
    }
    assert!(exited > 0, "no vehicle exited the path after green");
}

#[test]
fn a_free_flowing_vehicle_reaches_and_exits_the_path_end() {
    let mut sim = sim(FREE_FLOW, 2);
    let mut max_distance: f64 = 0.0;
    let mut exited = 0u64;
    for _ in 0..1200 {
        for event in sim.step().events() {
            if let Event::Despawned { .. } = event {
                exited += 1;
            }
        }
        for agent in sim.snapshot(SnapshotDetail::Full).agents() {
            max_distance =
                max_distance.max(agent.motion.as_ref().expect("full detail").path_distance_m);
        }
    }
    assert!(max_distance > 70.0, "free flow stalled at {max_distance} m");
    assert!(exited > 0, "free-flowing vehicles must exit the path");
}

#[test]
fn control_and_signal_state_are_reproducible_for_a_seed() {
    fn trace(seed: u64) -> Vec<(u64, Vec<String>, Option<SignalColor>)> {
        let mut sim = sim(SIGNALIZED, seed);
        let mut frames = Vec::new();
        for _ in 0..700 {
            sim.step();
            let agents = sim
                .snapshot(SnapshotDetail::Full)
                .agents()
                .iter()
                .map(|agent| {
                    let motion = agent.motion.as_ref().expect("full detail");
                    format!(
                        "{}:{:.6}:{:.6}",
                        agent.id.get(),
                        motion.path_distance_m,
                        motion.speed_mps
                    )
                })
                .collect();
            frames.push((
                sim.time().tick(),
                agents,
                sim.movement_signal(MovementId::from_index(0)),
            ));
        }
        frames
    }
    assert_eq!(trace(7), trace(7));
    assert_ne!(trace(7), trace(8), "different seeds should diverge");
}

#[test]
fn every_vehicle_keeps_its_sampled_profile_speed_bound() {
    let mut sim = sim(SIGNALIZED, 3);
    let expected = VehicleProfile {
        desired_speed_mps: 10.0,
        length_m: 4.0,
        width_m: 2.0,
        time_gap_s: 1.5,
        max_accel_mps2: 2.0,
        comfortable_brake_mps2: 3.0,
        compliance: 1.0,
    };
    for _ in 0..700 {
        sim.step();
        for agent in sim.snapshot(SnapshotDetail::Full).agents() {
            let motion = agent.motion.as_ref().expect("full detail");
            assert_eq!(sim.agent_profile(agent.id), Some(expected));
            assert!(
                motion.speed_mps >= -1e-9 && motion.speed_mps <= expected.desired_speed_mps + 1e-9,
                "speed {} outside the profile bound",
                motion.speed_mps
            );
        }
    }
}
