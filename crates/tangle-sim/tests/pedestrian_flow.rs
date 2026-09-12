//! Phase 1 Increment 3 slice A: pedestrian bodies, demand, route assignment,
//! and admission into the shared agent store.
//!
//! These tests use their own small scenarios rather than the checked-in
//! benchmarks so a rate, profile, or geometry change in a benchmark cannot
//! silently change what the kernel contract asserts. They cover admission,
//! route assignment, profile sampling, constant-speed route tracking, bounded
//! load shedding, and stream isolation from the vehicle mode; waypoint
//! steering, collision avoidance, and signal compliance are later slices.

use tangle_model::parse_scenario_source;
use tangle_model::{CompiledScenario, PathId, PedestrianDemandId, PedestrianRouteId};
use tangle_sim::{
    AgentId, AgentMode, Event, RunConfig, Simulation, SnapshotDetail, VehicleProfile,
};

/// A constant-profile walking scenario: a 40 m north-south path with one
/// pedestrian route and demand at its south end. Constant values keep the
/// tracking assertions exact.
const WALK: &str = r#"
{
  schema_version: 1,
  id: 'walking_demand',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'walk', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] } ],
  portals: [
    { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
  ],
  regions: [ { id: 'kerb', points: [
    { x: -2.0, y: -20.0 }, { x: 2.0, y: -20.0 }, { x: 2.0, y: -16.0 }, { x: -2.0, y: -16.0 }
  ] } ],
  waiting_areas: [ { id: 'south_kerb', region: 'kerb' } ],
  pedestrian_routes: [ { id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
    waiting_areas: [ 'south_kerb' ] } ],
  pedestrian_demand: [ { id: 'footfall', portal: 'south', rate_pph: 600.0,
    routes: [ { route: 'cross_to_north', weight: 1.0 } ] } ],
  pedestrian_profiles: {
    radius_m: { min: 0.25, max: 0.25 },
    speed_mps: { min: 1.25, max: 1.25 },
  },
}
"#;

/// The same walking scenario with an effectively saturated arrival rate.
const WALK_SATURATED: &str = r#"
{
  schema_version: 1,
  id: 'walking_saturated',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'walk', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] } ],
  portals: [
    { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
  ],
  pedestrian_routes: [ { id: 'cross_to_north', from: 'south', to: 'north', path: 'walk' } ],
  pedestrian_demand: [ { id: 'footfall', portal: 'south', rate_pph: 3600000.0,
    routes: [ { route: 'cross_to_north', weight: 1.0 } ] } ],
  pedestrian_profiles: {
    radius_m: { min: 0.25, max: 0.25 },
    speed_mps: { min: 1.25, max: 1.25 },
  },
}
"#;

/// A walking scenario with a route that enters at the path end and travels
/// backward, plus a non-degenerate profile envelope.
const WALK_REVERSE: &str = r#"
{
  schema_version: 1,
  id: 'walking_reverse',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'walk', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] } ],
  portals: [
    { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
  ],
  pedestrian_routes: [ { id: 'cross_to_south', from: 'north', to: 'south', path: 'walk' } ],
  pedestrian_demand: [ { id: 'footfall', portal: 'north', rate_pph: 600.0,
    routes: [ { route: 'cross_to_south', weight: 1.0 } ] } ],
  pedestrian_profiles: {
    radius_m: { min: 0.20, max: 0.30 },
    speed_mps: { min: 1.0, max: 1.6 },
  },
}
"#;

/// Both modes in one scenario: a car demand source on the road and a
/// pedestrian demand source on the crossing path.
const MIXED: &str = r#"
{
  schema_version: 1,
  id: 'mixed_world',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'road', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] },
    { id: 'walk', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] },
  ],
  portals: [
    { id: 'west', path: 'road', end: 'start', width_m: 7.0 },
    { id: 'east', path: 'road', end: 'end', width_m: 7.0 },
    { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
  ],
  regions: [ { id: 'crossing_zone', points: [
    { x: -2.0, y: -3.0 }, { x: 2.0, y: -3.0 }, { x: 2.0, y: 3.0 }, { x: -2.0, y: 3.0 }
  ] } ],
  movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 } ],
  crossings: [ { id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ] } ],
  pedestrian_routes: [ { id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
    crossings: [ 'road_crossing' ] } ],
  demand: [ { id: 'road_inflow', portal: 'west', rate_vph: 720.0,
    routes: [ { movement: 'ew_through', weight: 1.0 } ] } ],
  pedestrian_demand: [ { id: 'footfall', portal: 'south', rate_pph: 600.0,
    routes: [ { route: 'cross_to_north', weight: 1.0 } ] } ],
  profiles: {
    speed_mps: { min: 10.0, max: 10.0 },
    length_m: { min: 4.0, max: 4.0 },
    width_m: { min: 2.0, max: 2.0 },
    time_gap_s: { min: 1.5, max: 1.5 },
    max_accel_mps2: { min: 2.0, max: 2.0 },
    comfortable_brake_mps2: { min: 3.0, max: 3.0 },
  },
  pedestrian_profiles: {
    radius_m: { min: 0.25, max: 0.25 },
    speed_mps: { min: 1.25, max: 1.25 },
  },
}
"#;

/// The mixed scenario's road half only, with identical authored road geometry
/// and car demand. Comparing the two isolates the vehicle mode from any
/// pedestrian draw.
const ROAD_ONLY: &str = r#"
{
  schema_version: 1,
  id: 'road_only',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'road', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] } ],
  portals: [
    { id: 'west', path: 'road', end: 'start', width_m: 7.0 },
    { id: 'east', path: 'road', end: 'end', width_m: 7.0 },
  ],
  movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 } ],
  demand: [ { id: 'road_inflow', portal: 'west', rate_vph: 720.0,
    routes: [ { movement: 'ew_through', weight: 1.0 } ] } ],
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

/// Spawn events from one step, in emission order.
fn spawns(sim: &mut Simulation) -> Vec<(AgentId, PathId, f64)> {
    sim.step()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Spawned {
                agent,
                path,
                distance_m,
                ..
            } => Some((*agent, *path, *distance_m)),
            Event::Despawned { .. }
            | Event::Yielded { .. }
            | Event::Collision { .. }
            | Event::NearMiss { .. }
            | Event::Violation { .. }
            | Event::Entry { .. }
            | Event::Exit { .. }
            | Event::Queue { .. }
            | Event::ControlTransition { .. } => None,
        })
        .collect()
}

#[test]
fn pedestrian_demand_generates_pedestrians_instead_of_the_static_population() {
    let mut sim = sim(WALK, 1);
    assert_eq!(
        sim.agent_count(),
        0,
        "a demand scenario starts without the static walking population"
    );

    let mut first = None;
    for _ in 0..1000 {
        let spawned = spawns(&mut sim);
        if let Some((agent, path, distance_m)) = spawned.first().copied() {
            first = Some((agent, path, distance_m));
            break;
        }
    }
    let (agent, path, distance_m) = first.expect("pedestrian demand produced no arrivals");
    assert_eq!(sim.agent_mode(agent), Some(AgentMode::Pedestrian));
    assert!(sim.agent_pedestrian_route(agent).is_some());
    assert!(sim.agent_pedestrian_profile(agent).is_some());
    assert_eq!(
        sim.agent_route(agent),
        None,
        "a pedestrian has no car route"
    );
    assert_eq!(sim.agent_profile(agent), None);

    let profile = sim
        .agent_pedestrian_profile(agent)
        .expect("sampled pedestrian profile");
    assert!((profile.radius_m - 0.25).abs() < 1e-9);
    assert!((profile.desired_speed_mps - 1.25).abs() < 1e-9);

    // A pedestrian enters at the route's `from` portal, here the path start.
    assert_eq!(path, PathId::from_index(0));
    assert!(distance_m.abs() < 1e-9);

    // The reported body bounds are the circle's diameter on both axes.
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let sample = snapshot
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .expect("the pedestrian is alive");
    let motion = sample.motion.expect("full detail");
    assert_eq!(motion.mode, AgentMode::Pedestrian);
    assert!((motion.body_length_m - 0.5).abs() < 1e-9);
    assert!((motion.body_width_m - 0.5).abs() < 1e-9);
    assert_eq!(
        motion.pedestrian_route,
        Some(PedestrianRouteId::from_index(0))
    );
    assert_eq!(motion.route, None);
    assert_eq!(motion.profile, None);
}

#[test]
fn pedestrians_track_their_route_at_the_sampled_walking_speed() {
    let mut sim = sim(WALK, 2);
    let mut watched = None;
    for _ in 0..1000 {
        if let Some((agent, _, _)) = spawns(&mut sim).first().copied() {
            watched = Some(agent);
            break;
        }
    }
    let agent = watched.expect("a pedestrian spawned");

    // The spawn tick leaves the pedestrian at its entry point; the next tick
    // advances it exactly one step of its sampled speed along the route path.
    let distance_at_entry = sampled_distance(&sim, agent);
    assert!(distance_at_entry.abs() < 1e-9);

    sim.step();
    let distance_after_one = sampled_distance(&sim, agent);
    assert!(
        (distance_after_one - 1.25 * 0.05).abs() < 1e-12,
        "expected one 1.25 m/s step, got {distance_after_one}"
    );

    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let sample = snapshot
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .expect("the pedestrian is alive");
    let expected = sim
        .scenario()
        .path(PathId::from_index(0))
        .expect("route path")
        .position_at(distance_after_one);
    assert!((sample.position - expected).length() < 1e-12);
}

#[test]
fn pedestrians_despawn_at_the_route_exit() {
    let mut sim = sim(WALK, 3);
    let mut watched = None;
    for _ in 0..1000 {
        if let Some((agent, _, _)) = spawns(&mut sim).first().copied() {
            watched = Some(agent);
            break;
        }
    }
    let agent = watched.expect("a pedestrian spawned");

    let mut last_distance = 0.0;
    let mut exited = false;
    for _ in 0..2000 {
        let events: Vec<Event> = sim.step().events().to_vec();
        if let Some(distance) = sampled_distance_opt(&sim, agent) {
            assert!(
                distance >= last_distance - 1e-9,
                "a pedestrian must not move backward along its route"
            );
            last_distance = distance;
        }
        for event in &events {
            if let Event::Despawned {
                agent: departed,
                reason: tangle_sim::DespawnReason::ExitedPath,
                ..
            } = *event
                && departed == agent
            {
                exited = true;
            }
        }
        if exited {
            break;
        }
    }

    assert!(exited, "the pedestrian never reached its route exit");
    assert!(
        (last_distance - 40.0).abs() < 0.2,
        "the pedestrian left at {last_distance} m, not the path end"
    );
    let summary = sim.finish();
    assert_eq!(
        summary.spawned(),
        summary.despawned() + summary.remaining() as u64
    );
}

#[test]
fn every_admitted_pedestrian_receives_its_assigned_route() {
    let mut sim = sim(WALK, 4);
    let mut routes = Vec::new();
    for _ in 0..2000 {
        for (agent, _, _) in spawns(&mut sim) {
            routes.push(
                sim.agent_pedestrian_route(agent)
                    .expect("a demand pedestrian has a route"),
            );
        }
    }
    assert!(!routes.is_empty());
    assert!(
        routes
            .iter()
            .all(|route| *route == PedestrianRouteId::from_index(0))
    );
}

#[test]
fn pedestrian_profiles_stay_within_the_authored_ranges() {
    let compiled = scenario(WALK_REVERSE);
    let bounds = *compiled.pedestrian_profiles();
    let mut sim = Simulation::new(compiled, RunConfig::new(5)).expect("simulation builds");

    let mut sampled = Vec::new();
    for _ in 0..2000 {
        for (agent, _, _) in spawns(&mut sim) {
            sampled.push(
                sim.agent_pedestrian_profile(agent)
                    .expect("a demand pedestrian has a profile"),
            );
        }
    }
    assert!(sampled.len() > 4, "too few pedestrian profiles sampled");

    for profile in &sampled {
        assert!(
            profile.radius_m >= bounds.radius_m().min() - 1e-9
                && profile.radius_m <= bounds.radius_m().max() + 1e-9,
            "radius {} left its authored range",
            profile.radius_m
        );
        assert!(
            profile.desired_speed_mps >= bounds.speed_mps().min() - 1e-9
                && profile.desired_speed_mps <= bounds.speed_mps().max() + 1e-9,
            "speed {} left its authored range",
            profile.desired_speed_mps
        );
    }

    // The envelope is non-degenerate, so the samples must spread across it.
    let speeds: std::collections::BTreeSet<u64> = sampled
        .iter()
        .map(|profile| profile.desired_speed_mps.to_bits())
        .collect();
    assert!(
        speeds.len() > 1,
        "a wide walking-speed range must sample more than one value"
    );
}

#[test]
fn pedestrian_admission_never_overlaps_two_pedestrians() {
    let mut sim = sim(WALK_SATURATED, 6);
    for _ in 0..600 {
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let agents = snapshot.agents();
        for (index, first) in agents.iter().enumerate() {
            let first_motion = first.motion.expect("full detail");
            assert_eq!(first_motion.mode, AgentMode::Pedestrian);
            for second in &agents[index + 1..] {
                let second_motion = second.motion.expect("full detail");
                if first_motion.path != second_motion.path {
                    continue;
                }
                let half_lengths = (first_motion.body_length_m + second_motion.body_length_m) * 0.5;
                let gap = (first_motion.path_distance_m - second_motion.path_distance_m).abs();
                assert!(
                    gap >= half_lengths - 1e-9,
                    "pedestrians overlap at tick {}: gap {gap} < {half_lengths}",
                    snapshot.time().tick()
                );
            }
        }
    }
}

#[test]
fn saturated_pedestrian_demand_sheds_load_instead_of_growing_the_queue() {
    let mut sim = sim(WALK_SATURATED, 7);
    for _ in 0..400 {
        sim.step();
        assert!(sim.pending_pedestrian_arrivals(PedestrianDemandId::from_index(0)) <= 64);
    }
    let dropped = sim.dropped_pedestrian_arrivals(PedestrianDemandId::from_index(0));
    assert!(dropped > 0, "saturated demand should shed some arrivals");
    assert_eq!(dropped, sim.finish().dropped());
}

#[test]
fn same_seed_reproduces_pedestrian_demand_and_profiles() {
    fn run(seed: u64) -> Vec<(u32, Option<tangle_sim::PedestrianProfile>, u64)> {
        let mut sim = sim(WALK_REVERSE, seed);
        let mut record = Vec::new();
        for _ in 0..1200 {
            for (agent, _, _) in spawns(&mut sim) {
                record.push((
                    agent.get(),
                    sim.agent_pedestrian_profile(agent),
                    sim.time().tick(),
                ));
            }
        }
        record
    }

    assert_eq!(run(31), run(31));
    assert_ne!(run(31), run(32), "different seeds should diverge");
}

#[test]
fn a_route_that_enters_at_the_path_end_travels_backward() {
    let mut sim = sim(WALK_REVERSE, 8);
    let mut entered = None;
    for _ in 0..1000 {
        if let Some((agent, path, distance_m)) = spawns(&mut sim).first().copied() {
            entered = Some((agent, path, distance_m));
            break;
        }
    }
    let (agent, path, entry_distance) = entered.expect("reverse route spawned a pedestrian");
    assert_eq!(path, PathId::from_index(0));
    assert!((entry_distance - 40.0).abs() < 1e-9);

    sim.step();
    let distance = sampled_distance(&sim, agent);
    assert!(
        distance < entry_distance,
        "the pedestrian should move toward the path start"
    );
}

#[test]
fn both_modes_share_one_store_snapshot_and_event_stream() {
    let mut sim = sim(MIXED, 9);
    let mut saw_vehicle = false;
    let mut saw_pedestrian = false;
    for _ in 0..2000 {
        for (agent, _, _) in spawns(&mut sim) {
            match sim.agent_mode(agent) {
                Some(AgentMode::Vehicle) => saw_vehicle = true,
                Some(AgentMode::Pedestrian) => saw_pedestrian = true,
                None => panic!("a spawned agent must have a mode"),
            }
        }
        if saw_vehicle && saw_pedestrian {
            break;
        }
    }
    assert!(
        saw_vehicle && saw_pedestrian,
        "both modes must be generated"
    );

    // One snapshot carries both body kinds, distinguished by the mode field.
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let modes: Vec<AgentMode> = snapshot
        .agents()
        .iter()
        .map(|sample| sample.motion.expect("full detail").mode)
        .collect();
    assert!(modes.contains(&AgentMode::Vehicle));
    assert!(modes.contains(&AgentMode::Pedestrian));
}

#[test]
fn pedestrian_demand_does_not_perturb_vehicle_arrivals() {
    // Two scenarios differ only in whether pedestrian demand is declared. The
    // vehicle `demand` substream is derived from the root seed alone, so the
    // vehicle arrival trace is byte-identical.
    fn road_arrivals(text: &str, seed: u64) -> Vec<(u64, f64)> {
        let mut sim = sim(text, seed);
        let mut record = Vec::new();
        for _ in 0..1200 {
            let tick = sim.time().tick();
            for (_, path, distance_m) in spawns(&mut sim) {
                if path == PathId::from_index(0) {
                    record.push((tick, distance_m));
                }
            }
        }
        record
    }

    let with_pedestrians = road_arrivals(MIXED, 11);
    let without_pedestrians = road_arrivals(ROAD_ONLY, 11);
    assert!(!with_pedestrians.is_empty(), "no vehicle arrivals recorded");
    assert_eq!(with_pedestrians, without_pedestrians);
}

#[test]
fn a_pedestrian_is_not_a_profile_vehicle() {
    // Guard the mode split directly, so a regression that admitted a
    // pedestrian as an IDM vehicle would fail here rather than silently.
    let mut sim = sim(WALK, 12);
    let mut agent = None;
    for _ in 0..1000 {
        if let Some((spawned, _, _)) = spawns(&mut sim).first().copied() {
            agent = Some(spawned);
            break;
        }
    }
    let agent = agent.expect("a pedestrian spawned");
    assert_eq!(sim.agent_mode(agent), Some(AgentMode::Pedestrian));
    assert_eq!(sim.agent_profile(agent), None);
    assert!(
        sim.agent_pedestrian_profile(agent).is_some(),
        "a pedestrian must sample the pedestrian profile"
    );
    assert_eq!(
        sim.agent_route(agent),
        None,
        "a pedestrian has no movement route"
    );
    let _: Option<VehicleProfile> = sim.agent_profile(agent);
}

/// Arc-length position of a live agent, or `None` once it is gone.
fn sampled_distance_opt(sim: &Simulation, agent: AgentId) -> Option<f64> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .map(|sample| sample.motion.expect("full detail").path_distance_m)
}

fn sampled_distance(sim: &Simulation, agent: AgentId) -> f64 {
    sampled_distance_opt(sim, agent).expect("the agent is alive")
}
