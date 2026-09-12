//! Phase 1 Increment 2 slice A: portal demand, route assignment, profile
//! sampling, and safe spawn admission.
//!
//! These tests use their own small scenarios rather than the checked-in
//! benchmarks so a rate, profile, or geometry change in a benchmark cannot
//! silently change what the kernel contract asserts.

use tangle_model::{CompiledScenario, DemandId, MovementId, parse_scenario_source};
use tangle_sim::{AgentId, Event, RunConfig, Simulation, SnapshotDetail, VehicleProfile};

/// A straight 80 m approach with one entry/exit portal pair and one movement.
/// Constant profile values keep the behavior assertions exact.
const STRAIGHT: &str = r#"
{
  schema_version: 1,
  id: 'straight_demand',
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

/// The same approach with an effectively saturated arrival rate.
const SATURATED: &str = r#"
{
  schema_version: 1,
  id: 'saturated_demand',
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
    { id: 'inflow', portal: 'entry', rate_vph: 3600000.0,
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

/// One movement that enters at the path end and travels back to the start.
const REVERSE: &str = r#"
{
  schema_version: 1,
  id: 'reverse_demand',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  movements: [
    { id: 'reverse', from: 'exit', to: 'entry', path: 'guide', priority: 0 },
  ],
  demand: [
    { id: 'inflow', portal: 'exit', rate_vph: 720.0,
      routes: [ { movement: 'reverse', weight: 1.0 } ] },
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

#[test]
fn demand_generates_arrivals_over_time() {
    let mut sim = sim(STRAIGHT, 1);
    assert_eq!(sim.agent_count(), 0, "demand scenarios start empty");

    let mut spawned = Vec::new();
    for _ in 0..2000 {
        for event in sim.step().events() {
            if let Event::Spawned {
                agent, distance_m, ..
            } = *event
            {
                spawned.push((agent, distance_m));
            }
        }
    }
    assert!(!spawned.is_empty(), "demand produced no arrivals");
    // Every vehicle enters exactly at the movement entry, distance zero.
    assert!(
        spawned
            .iter()
            .all(|(_, distance_m)| distance_m.abs() < 1e-9)
    );

    let summary = sim.finish();
    assert_eq!(summary.spawned(), spawned.len() as u64);
    assert!(summary.spawned() > 0);
    assert_eq!(summary.dropped(), 0, "moderate demand should not shed load");
}

#[test]
fn every_admitted_vehicle_receives_its_assigned_route() {
    let mut sim = sim(STRAIGHT, 2);
    let mut routes = Vec::new();
    for _ in 0..2000 {
        let spawned: Vec<AgentId> = sim
            .step()
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::Spawned { agent, .. } => Some(*agent),
                Event::Despawned { .. } => None,
            })
            .collect();
        for agent in spawned {
            let route = sim.agent_route(agent).expect("demand vehicle has a route");
            routes.push(route);
        }
    }
    assert!(!routes.is_empty());
    assert!(
        routes
            .iter()
            .all(|route| *route == MovementId::from_index(0))
    );
}

#[test]
fn safe_admission_never_overlaps_two_vehicles() {
    let mut sim = sim(SATURATED, 3);
    for _ in 0..400 {
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let agents = snapshot.agents();
        for (index, first) in agents.iter().enumerate() {
            let first_motion = first.motion.expect("full detail");
            for second in &agents[index + 1..] {
                let second_motion = second.motion.expect("full detail");
                if first_motion.path != second_motion.path {
                    continue;
                }
                let half_lengths = (first_motion.body_length_m + second_motion.body_length_m) * 0.5;
                let gap = (first_motion.path_distance_m - second_motion.path_distance_m).abs();
                assert!(
                    gap >= half_lengths - 1e-9,
                    "vehicles overlap at tick {}: gap {gap} < {half_lengths}",
                    snapshot.time().tick()
                );
            }
        }
    }
}

#[test]
fn saturated_demand_sheds_load_instead_of_growing_the_queue() {
    let mut sim = sim(SATURATED, 4);
    for _ in 0..400 {
        sim.step();
        // The pending queue is bounded, so a saturating portal cannot grow it
        // without limit.
        assert!(sim.pending_arrivals(DemandId::from_index(0)) <= 64);
    }
    let dropped = sim.dropped_arrivals(DemandId::from_index(0));
    assert!(dropped > 0, "saturated demand should shed some arrivals");
    assert_eq!(dropped, sim.finish().dropped());
}

#[test]
fn profile_sampling_stays_within_the_configured_ranges() {
    let mut sim = sim(STRAIGHT, 5);
    let expected = VehicleProfile {
        desired_speed_mps: 10.0,
        length_m: 4.0,
        width_m: 2.0,
        time_gap_s: 1.5,
        max_accel_mps2: 2.0,
        comfortable_brake_mps2: 3.0,
    };
    let mut checked = 0;
    for _ in 0..2000 {
        let spawned: Vec<AgentId> = sim
            .step()
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::Spawned { agent, .. } => Some(*agent),
                Event::Despawned { .. } => None,
            })
            .collect();
        for agent in spawned {
            let profile = sim.agent_profile(agent).expect("sampled profile");
            assert_eq!(profile, expected);
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn profile_sampling_uses_its_own_stream() {
    // Two runs that differ only in demand rate draw a different amount from the
    // `demand` stream, but agent 0's `profile` substream is derived from the
    // stable agent id, so its profile is unchanged.
    let mut slow = sim(STRAIGHT, 6);
    let mut fast = sim(SATURATED, 6);
    for _ in 0..4000 {
        slow.step();
        fast.step();
        if slow.agent_profile(AgentId::from_index(0)).is_some() {
            break;
        }
    }
    let slow_profile = slow.agent_profile(AgentId::from_index(0));
    let fast_profile = fast.agent_profile(AgentId::from_index(0));
    assert!(
        slow_profile.is_some(),
        "slow demand never spawned a vehicle"
    );
    assert_eq!(slow_profile, fast_profile);
}

#[test]
fn same_seed_reproduces_demand_and_profiles() {
    fn run(seed: u64) -> Vec<(u32, Option<VehicleProfile>, u64)> {
        let mut sim = sim(STRAIGHT, seed);
        let mut record = Vec::new();
        for _ in 0..1200 {
            let spawned: Vec<AgentId> = sim
                .step()
                .events()
                .iter()
                .filter_map(|event| match event {
                    Event::Spawned { agent, .. } => Some(*agent),
                    Event::Despawned { .. } => None,
                })
                .collect();
            for agent in spawned {
                record.push((agent.get(), sim.agent_profile(agent), sim.time().tick()));
            }
        }
        record
    }

    assert_eq!(run(21), run(21));
    assert_ne!(run(21), run(22), "different seeds should diverge");
}

#[test]
fn a_movement_that_enters_at_the_path_end_travels_backward() {
    let mut sim = sim(REVERSE, 7);
    // Drive until the first vehicle enters at the path end (80 m).
    let mut entered = None;
    for _ in 0..4000 {
        for event in sim.step().events() {
            if let Event::Spawned {
                agent, distance_m, ..
            } = *event
            {
                entered = Some((agent, distance_m));
                break;
            }
        }
        if entered.is_some() {
            break;
        }
    }
    let (agent, entry_distance) = entered.expect("reverse movement spawned a vehicle");
    assert!((entry_distance - 80.0).abs() < 1e-9);

    // One tick later the vehicle is closer to the start, not the end.
    sim.step();
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let sample = snapshot
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .expect("vehicle alive one tick after entry");
    assert!(sample.motion.expect("full detail").path_distance_m < 80.0);
}
