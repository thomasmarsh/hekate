//! Phase 1 Increment 3 slice B: the pedestrian waypoint controller.
//!
//! These tests cover waypoint and path-progress tracking along a compiled
//! pedestrian route, bounded steering, and deterministic local collision
//! avoidance against pedestrians and vehicles that share the world. They use
//! their own small scenarios, so a checked-in benchmark change cannot silently
//! move the kernel contract they assert.
//!
//! Nothing here asserts a pedestrian signal rule. Vehicle yielding to an
//! occupied crossing is slice D and is asserted in `vehicle_yielding.rs`; the
//! benchmark test below still asserts only what this slice guarantees: a
//! pedestrian's own step never initiates contact with a body. The checked-in
//! benchmark now authors a `yield` rule, so its isolated scenarios here keep
//! their own small layouts and this file's contract is unchanged by yielding.

use glam::DVec2;
use hekate_model::{
    CompiledScenario, CrossingId, PedestrianRouteId, WaitingAreaId, parse_scenario_source,
};
use hekate_sim::pedestrian::{
    CONTACT_MARGIN_M, MAX_ACCEL_MPS2, MAX_DECEL_MPS2, MAX_LATERAL_ACCEL_MPS2, MAX_TURN_RATE_RAD_S,
    SENSE_RADIUS_M,
};
use hekate_sim::{
    AgentMode, AgentSample, Event, PedestrianZone, RunConfig, Simulation, SnapshotDetail,
};

/// The fixed kernel step these tests assert bounds against.
const STEP_S: f64 = 0.05;

/// One east-west road, one north-south walking path over it, a crossing region,
/// one waiting area on each kerb, and one pedestrian route in each direction.
/// Each zone projects onto the walking path so the derived waypoints are
/// exactly known: arcs 2 m, 20 m, and 38 m of the path's 40 m length.
const CROSSING: &str = r#"
{
  schema_version: 1,
  id: 'pedestrian_crossing_contract',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'road', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] },
    { id: 'walk', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] },
  ],
  portals: [
    { id: 'west', path: 'road', end: 'start', width_m: 7.0 },
    { id: 'east', path: 'road', end: 'end', width_m: 7.0 },
    { id: 'south', path: 'walk', end: 'start', width_m: 3.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 3.0 },
  ],
  regions: [
    { id: 'crossing_zone', points: [
      { x: -3.0, y: -3.0 }, { x: 3.0, y: -3.0 }, { x: 3.0, y: 3.0 }, { x: -3.0, y: 3.0 }
    ] },
    { id: 'south_kerb', points: [
      { x: -3.0, y: -20.0 }, { x: 3.0, y: -20.0 }, { x: 3.0, y: -16.0 }, { x: -3.0, y: -16.0 }
    ] },
    { id: 'north_kerb', points: [
      { x: -3.0, y: 16.0 }, { x: 3.0, y: 16.0 }, { x: 3.0, y: 20.0 }, { x: -3.0, y: 20.0 }
    ] },
  ],
  movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 } ],
  crossings: [ { id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ] } ],
  waiting_areas: [
    { id: 'south_wait', region: 'south_kerb' },
    { id: 'north_wait', region: 'north_kerb' },
  ],
  pedestrian_routes: [
    { id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
      crossings: [ 'road_crossing' ], waiting_areas: [ 'south_wait', 'north_wait' ] },
    { id: 'cross_to_south', from: 'north', to: 'south', path: 'walk',
      crossings: [ 'road_crossing' ], waiting_areas: [ 'south_wait', 'north_wait' ] },
  ],
  demand: [
    { id: 'road_inflow', portal: 'west', rate_vph: 600.0,
      routes: [ { movement: 'ew_through', weight: 1.0 } ] },
  ],
  pedestrian_demand: [
    { id: 'northbound', portal: 'south', rate_pph: 300.0,
      routes: [ { route: 'cross_to_north', weight: 1.0 } ] },
    { id: 'southbound', portal: 'north', rate_pph: 300.0,
      routes: [ { route: 'cross_to_south', weight: 1.0 } ] },
  ],
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

/// One 40 m walking path with a route in each direction and no named zone at
/// all, so opposite-direction pedestrians meet head-on on the same path.
const HEAD_ON: &str = r#"
{
  schema_version: 1,
  id: 'head_on_contract',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'walk', points: [ { x: 0.0, y: 0.0 }, { x: 0.0, y: 40.0 } ] } ],
  portals: [
    { id: 'south', path: 'walk', end: 'start', width_m: 3.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 3.0 },
  ],
  pedestrian_routes: [
    { id: 'northbound', from: 'south', to: 'north', path: 'walk' },
    { id: 'southbound', from: 'north', to: 'south', path: 'walk' },
  ],
  pedestrian_demand: [
    { id: 'south_arrivals', portal: 'south', rate_pph: 300.0,
      routes: [ { route: 'northbound', weight: 1.0 } ] },
    { id: 'north_arrivals', portal: 'north', rate_pph: 300.0,
      routes: [ { route: 'southbound', weight: 1.0 } ] },
  ],
  pedestrian_profiles: {
    radius_m: { min: 0.25, max: 0.25 },
    speed_mps: { min: 1.25, max: 1.25 },
  },
}
"#;

/// The crossing layout with no vehicle demand and a saturated pedestrian rate,
/// so pedestrians queue at the portal admission clearance on a straight path.
const WALK_ONLY: &str = r#"
{
  schema_version: 1,
  id: 'walking_queue_contract',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [
    { id: 'road', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] },
    { id: 'walk', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] },
  ],
  portals: [
    { id: 'west', path: 'road', end: 'start', width_m: 7.0 },
    { id: 'east', path: 'road', end: 'end', width_m: 7.0 },
    { id: 'south', path: 'walk', end: 'start', width_m: 3.0 },
    { id: 'north', path: 'walk', end: 'end', width_m: 3.0 },
  ],
  regions: [
    { id: 'crossing_zone', points: [
      { x: -3.0, y: -3.0 }, { x: 3.0, y: -3.0 }, { x: 3.0, y: 3.0 }, { x: -3.0, y: 3.0 }
    ] },
    { id: 'south_kerb', points: [
      { x: -3.0, y: -20.0 }, { x: 3.0, y: -20.0 }, { x: 3.0, y: -16.0 }, { x: -3.0, y: -16.0 }
    ] },
  ],
  movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 } ],
  crossings: [ { id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ] } ],
  waiting_areas: [ { id: 'south_wait', region: 'south_kerb' } ],
  pedestrian_routes: [
    { id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
      crossings: [ 'road_crossing' ], waiting_areas: [ 'south_wait' ] },
  ],
  pedestrian_demand: [
    { id: 'northbound', portal: 'south', rate_pph: 3600000.0,
      routes: [ { route: 'cross_to_north', weight: 1.0 } ] },
  ],
  pedestrian_profiles: {
    radius_m: { min: 0.25, max: 0.25 },
    speed_mps: { min: 1.25, max: 1.25 },
  },
}
"#;

/// One path shared by a slow vehicle stream and a pedestrian stream: the
/// pedestrian catches the leading vehicle and must steer around it, which is
/// this slice's pedestrian-versus-vehicle avoidance in one dimension of space.
const SHARED_PATH: &str = r#"
{
  schema_version: 1,
  id: 'shared_path_contract',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'shared', points: [ { x: 0.0, y: -20.0 }, { x: 0.0, y: 20.0 } ] } ],
  portals: [
    { id: 'south', path: 'shared', end: 'start', width_m: 3.0 },
    { id: 'north', path: 'shared', end: 'end', width_m: 3.0 },
  ],
  movements: [ { id: 'north_run', from: 'south', to: 'north', path: 'shared', priority: 0 } ],
  pedestrian_routes: [
    { id: 'north_walk', from: 'south', to: 'north', path: 'shared' },
  ],
  demand: [
    { id: 'slow_traffic', portal: 'south', rate_vph: 180.0,
      routes: [ { movement: 'north_run', weight: 1.0 } ] },
  ],
  pedestrian_demand: [
    { id: 'footfall', portal: 'south', rate_pph: 200.0,
      routes: [ { route: 'north_walk', weight: 1.0 } ] },
  ],
  profiles: {
    speed_mps: { min: 0.5, max: 0.5 },
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

/// The checked-in mixed benchmark, measured rather than duplicated.
const BENCHMARK: &str = include_str!("../../../scenarios/benchmarks/pedestrian_crossing_v1.json5");

fn scenario(text: &str) -> CompiledScenario {
    let source = parse_scenario_source(text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(text: &str, seed: u64) -> Simulation {
    Simulation::new(scenario(text), RunConfig::new(seed)).expect("simulation builds")
}

/// Live pedestrian samples from one snapshot.
fn pedestrians(sim: &Simulation) -> Vec<AgentSample> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter(|sample| sample.motion.as_ref().expect("full detail").mode == AgentMode::Pedestrian)
        .cloned()
        .collect()
}

/// Live agent identifiers from one snapshot.
fn live_ids(sim: &Simulation) -> Vec<u32> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .map(|sample| sample.id.get())
        .collect()
}

/// Surface clearance in metres between a circle body and an oriented box body.
///
/// This is the standard exact circle-versus-box distance, computed in the box's
/// own frame; negative means the circle overlaps the box.
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

/// Shortest angle from `from_rad` to `to_rad`, in `(-pi, pi]`.
fn angle_delta(from_rad: f64, to_rad: f64) -> f64 {
    let mut delta = (to_rad - from_rad) % std::f64::consts::TAU;
    if delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    } else if delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    delta
}

#[test]
fn waypoints_follow_the_route_in_travel_order() {
    let sim = sim(CROSSING, 1);
    let waypoints = sim.route_waypoints(PedestrianRouteId::from_index(0));
    assert_eq!(waypoints.len(), 4, "two zones, the crossing, and the exit");

    let expected = [
        (
            2.0,
            DVec2::new(0.0, -18.0),
            Some(PedestrianZone::WaitingArea(WaitingAreaId::from_index(0))),
        ),
        (
            20.0,
            DVec2::new(0.0, 0.0),
            Some(PedestrianZone::Crossing(CrossingId::from_index(0))),
        ),
        (
            38.0,
            DVec2::new(0.0, 18.0),
            Some(PedestrianZone::WaitingArea(WaitingAreaId::from_index(1))),
        ),
        (40.0, DVec2::new(0.0, 20.0), None),
    ];
    for (waypoint, (arc_m, position, zone)) in waypoints.iter().zip(expected) {
        assert!(
            (waypoint.arc_m() - arc_m).abs() < 1e-9,
            "arc {} != {arc_m}",
            waypoint.arc_m()
        );
        assert!((waypoint.position() - position).length() < 1e-9);
        assert_eq!(waypoint.zone(), zone);
    }
    // The entry is the admission point, so the first target is a zone ahead and
    // the last target is the exit.
    assert!(waypoints[0].zone().is_some());
    assert!(waypoints.last().expect("an exit").zone().is_none());
}

#[test]
fn a_route_that_enters_at_the_path_end_reverses_the_waypoint_order() {
    let sim = sim(CROSSING, 1);
    let waypoints = sim.route_waypoints(PedestrianRouteId::from_index(1));
    assert_eq!(waypoints.len(), 4);
    let zones: Vec<Option<PedestrianZone>> =
        waypoints.iter().map(|waypoint| waypoint.zone()).collect();
    assert_eq!(
        zones,
        vec![
            Some(PedestrianZone::WaitingArea(WaitingAreaId::from_index(1))),
            Some(PedestrianZone::Crossing(CrossingId::from_index(0))),
            Some(PedestrianZone::WaitingArea(WaitingAreaId::from_index(0))),
            None,
        ],
        "a backward route visits the same zones in reverse travel order"
    );
    let arcs: Vec<f64> = waypoints.iter().map(|waypoint| waypoint.arc_m()).collect();
    assert_eq!(arcs, vec![38.0, 20.0, 2.0, 0.0]);
    // Its exit is the far end of its travel: the path start.
    assert!((waypoints[3].position() - DVec2::new(0.0, -20.0)).length() < 1e-9);
}

#[test]
fn a_route_that_names_no_zone_has_only_its_exit() {
    let sim = sim(HEAD_ON, 1);
    for (route_index, exit_arc_m, exit_position) in [
        (0usize, 40.0, DVec2::new(0.0, 40.0)),
        (1usize, 0.0, DVec2::new(0.0, 0.0)),
    ] {
        let waypoints = sim.route_waypoints(PedestrianRouteId::from_index(route_index));
        assert_eq!(waypoints.len(), 1);
        assert!((waypoints[0].arc_m() - exit_arc_m).abs() < 1e-9);
        assert!((waypoints[0].position() - exit_position).length() < 1e-9);
        assert!(waypoints[0].zone().is_none());
    }
}

#[test]
fn the_waypoint_cursor_only_moves_forward_and_reaches_the_exit() {
    let mut sim = sim(CROSSING, 2);
    let plan: Vec<f64> = sim
        .route_waypoints(PedestrianRouteId::from_index(0))
        .iter()
        .map(|waypoint| waypoint.arc_m())
        .collect();
    let mut cursors: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    let mut advanced = 0u32;
    let mut reached_exit = 0u32;
    for _ in 0..2000 {
        sim.step();
        for sample in pedestrians(&sim) {
            let motion = sample.motion.as_ref().expect("full detail");
            if motion.pedestrian_route != Some(PedestrianRouteId::from_index(0)) {
                continue;
            }
            let target = sim
                .pedestrian_waypoint(sample.id)
                .expect("a live pedestrian has a target");
            let index = plan
                .iter()
                .position(|arc_m| (arc_m - target.arc_m()).abs() < 1e-9)
                .expect("the target is one of the route's waypoints");
            if let Some(previous) = cursors.insert(sample.id.get(), index) {
                assert!(
                    index >= previous,
                    "the cursor moved backward for agent {}: {previous} -> {index}",
                    sample.id.get()
                );
                if index > previous {
                    advanced += 1;
                }
            }
            if index + 1 == plan.len() {
                reached_exit += 1;
            }
        }
    }
    assert!(advanced > 0, "some pedestrian advanced a waypoint");
    assert!(reached_exit > 0, "some pedestrian aimed at the route exit");
}

#[test]
fn steering_stays_within_the_documented_bounds() {
    let mut sim = sim(CROSSING, 3);
    let mut previous: std::collections::BTreeMap<u32, (f64, f64)> =
        std::collections::BTreeMap::new();
    let mut cap_steps = 0u64;
    let mut observed = 0u64;
    for _ in 0..3000 {
        let caps_before = sim.pedestrian_cap_steps();
        sim.step();
        let cap_engaged = sim.pedestrian_cap_steps() > caps_before;
        if cap_engaged {
            cap_steps += 1;
        }
        for sample in pedestrians(&sim) {
            let motion = sample.motion.as_ref().expect("full detail");
            let desired_speed_mps = sim
                .agent_pedestrian_profile(sample.id)
                .expect("a demand pedestrian has a profile")
                .desired_speed_mps;
            assert!(sample.position.is_finite() && sample.heading_rad.is_finite());
            assert!(motion.speed_mps >= -1e-9, "negative speed");
            assert!(
                motion.speed_mps <= desired_speed_mps + 1e-9,
                "speed {} above the desired {desired_speed_mps}",
                motion.speed_mps
            );
            let Some((speed_before, heading_before)) =
                previous.insert(sample.id.get(), (motion.speed_mps, sample.heading_rad))
            else {
                observed += 1;
                continue;
            };
            let turned = angle_delta(heading_before, sample.heading_rad).abs();
            assert!(
                turned <= MAX_TURN_RATE_RAD_S * STEP_S + 1e-9,
                "agent {} turned {turned} rad in one step",
                sample.id.get()
            );
            assert!(
                speed_before * turned / STEP_S <= MAX_LATERAL_ACCEL_MPS2 + 1e-6,
                "agent {} exceeded the lateral-acceleration bound",
                sample.id.get()
            );
            let change = motion.speed_mps - speed_before;
            assert!(
                change <= MAX_ACCEL_MPS2 * STEP_S + 1e-9,
                "agent {} accelerated {change} m/s in one step",
                sample.id.get()
            );
            // A step that brakes harder than the steering bound must be a
            // counted spacing-cap step, which is the documented exception.
            if !cap_engaged {
                assert!(
                    change >= -MAX_DECEL_MPS2 * STEP_S - 1e-9,
                    "agent {} braked {change} m/s without a counted cap step",
                    sample.id.get()
                );
            }
        }
    }
    assert!(observed > 0, "pedestrians were admitted");
    assert!(cap_steps > 0, "the documented spacing cap was exercised");
}

#[test]
fn a_pedestrian_steers_around_a_vehicle_that_shares_its_path() {
    let mut sim = sim(SHARED_PATH, 4);
    let mut minimum_clearance_m = f64::INFINITY;
    let mut maximum_lateral_offset_m: f64 = 0.0;
    let mut pedestrian_overlaps = 0u32;
    let mut first_seen: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    for _ in 0..4000 {
        let tick = sim.time().tick();
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let agents: Vec<AgentSample> = snapshot.agents().to_vec();
        for sample in &agents {
            let motion = sample.motion.as_ref().expect("full detail");
            if motion.mode != AgentMode::Pedestrian {
                continue;
            }
            first_seen.entry(sample.id.get()).or_insert(tick);
            // The route path is the line x = 0, so |x| is the lateral deviation
            // the controller commanded to get around the vehicle.
            maximum_lateral_offset_m = maximum_lateral_offset_m.max(sample.position.x.abs());
            let radius_m = motion.body_length_m * 0.5;
            for other in &agents {
                let other_motion = other.motion.as_ref().expect("full detail");
                match other_motion.mode {
                    AgentMode::Vehicle => {
                        minimum_clearance_m = minimum_clearance_m.min(circle_box_clearance(
                            sample.position,
                            radius_m,
                            other.position,
                            other.heading_rad,
                            other_motion.body_length_m,
                            other_motion.body_width_m,
                        ));
                    }
                    AgentMode::Pedestrian => {
                        if other.id <= sample.id {
                            continue;
                        }
                        let touching_m = radius_m + other_motion.body_length_m * 0.5;
                        if (other.position - sample.position).length() < touching_m - 1e-9 {
                            pedestrian_overlaps += 1;
                        }
                    }
                }
            }
        }
    }
    // Passing a body ahead never closes below the documented contact margin.
    assert!(
        minimum_clearance_m >= CONTACT_MARGIN_M - 1e-9,
        "a pedestrian closed to {} m on the vehicle, below the contact margin",
        minimum_clearance_m
    );
    assert!(
        minimum_clearance_m < 1.0,
        "the vehicle was never encountered"
    );
    assert_eq!(pedestrian_overlaps, 0);
    assert!(
        maximum_lateral_offset_m > 1.0,
        "the pedestrian never steered around the vehicle (offset {maximum_lateral_offset_m} m)"
    );

    // Nothing deadlocks behind the slow vehicle: everyone who arrived with time
    // to spare is through by the end.
    let live_ids = live_ids(&sim);
    for (id, tick) in first_seen {
        if tick < 3000 {
            assert!(
                !live_ids.contains(&id),
                "agent {id} was still stuck behind the vehicle"
            );
        }
    }
}

#[test]
fn pedestrians_pass_each_other_without_overlapping_or_teleporting() {
    let mut sim = sim(HEAD_ON, 5);
    let mut minimum_gap_m = f64::INFINITY;
    let mut encounters = 0u64;
    let mut first_seen: std::collections::BTreeMap<u32, u64> = std::collections::BTreeMap::new();
    for _ in 0..4000 {
        let tick = sim.time().tick();
        let before = pedestrians(&sim);
        sim.step();
        let after = pedestrians(&sim);
        for sample in &after {
            first_seen.entry(sample.id.get()).or_insert(tick);
            if let Some(previous) = before.iter().find(|prior| prior.id == sample.id) {
                let step_m = (sample.position - previous.position).length();
                assert!(
                    step_m <= 1.25 * STEP_S + 1e-9,
                    "agent {} moved {step_m} m in one step",
                    sample.id.get()
                );
            }
        }
        for (index, sample) in after.iter().enumerate() {
            let motion = sample.motion.as_ref().expect("full detail");
            let radius_m = motion.body_length_m * 0.5;
            for other in &after[index + 1..] {
                let other_motion = other.motion.as_ref().expect("full detail");
                if other_motion.path != motion.path {
                    continue;
                }
                let gap_m = (other.position - sample.position).length()
                    - radius_m
                    - other_motion.body_length_m * 0.5;
                minimum_gap_m = minimum_gap_m.min(gap_m);
                if gap_m < 0.5 {
                    encounters += 1;
                }
            }
        }
    }
    assert!(
        minimum_gap_m >= -1e-9,
        "pedestrians overlapped by {} m",
        -minimum_gap_m
    );
    assert!(
        encounters > 0,
        "opposite-direction pedestrians never came close enough to interact"
    );

    // No route deadlock: everyone who arrived with time to spare is through.
    let live_ids = live_ids(&sim);
    for (id, tick) in first_seen {
        if tick < 3000 {
            assert!(!live_ids.contains(&id), "agent {id} never left the path");
        }
    }
}

#[test]
fn a_same_speed_pedestrian_queue_keeps_its_admission_spacing() {
    // Every pedestrian in this saturated queue walks at the same sampled speed
    // and the personal-space boundary is tighter than the portal admission
    // clearance, so none is deflected or capped: the queue holds its spacing at
    // its sampled speed instead of compressing or stalling.
    let mut sim = sim(WALK_ONLY, 6);
    for _ in 0..1200 {
        sim.step();
    }
    let agents = pedestrians(&sim);
    assert!(agents.len() > 3, "the saturated queue must be populated");
    for (index, sample) in agents.iter().enumerate() {
        let motion = sample.motion.as_ref().expect("full detail");
        assert!(
            (motion.speed_mps - 1.25).abs() < 1e-9,
            "a queued pedestrian left its sampled speed: {}",
            motion.speed_mps
        );
        assert!(sample.position.x.abs() < 1e-9, "no lateral deviation");
        for other in &agents[index + 1..] {
            let other_motion = other.motion.as_ref().expect("full detail");
            if other_motion.path != motion.path {
                continue;
            }
            let half_lengths = (motion.body_length_m + other_motion.body_length_m) * 0.5;
            let gap_m = (motion.path_distance_m - other_motion.path_distance_m).abs();
            assert!(
                gap_m >= half_lengths + CONTACT_MARGIN_M - 1e-9,
                "the queue compressed below its admission clearance: {gap_m}"
            );
        }
    }
}

#[test]
fn a_pedestrian_only_reacts_to_bodies_within_its_sense_radius() {
    let mut sim = sim(HEAD_ON, 7);
    // A pedestrian never reacts to a body beyond the documented sense radius,
    // so it holds its sampled speed and heading until some body is inside it.
    for _ in 0..400 {
        let before = pedestrians(&sim);
        sim.step();
        for sample in pedestrians(&sim) {
            let motion = sample.motion.as_ref().expect("full detail");
            let Some(previous) = before.iter().find(|prior| prior.id == sample.id) else {
                continue;
            };
            let nearest_neighbour_m = before
                .iter()
                .filter(|other| other.id != sample.id)
                .map(|other| (other.position - previous.position).length())
                .fold(f64::INFINITY, f64::min);
            if nearest_neighbour_m > SENSE_RADIUS_M {
                assert!(
                    (motion.speed_mps - 1.25).abs() < 1e-9,
                    "agent {} slowed with no body inside the sense radius",
                    sample.id.get()
                );
                assert!(
                    angle_delta(previous.heading_rad, sample.heading_rad).abs() < 1e-9,
                    "agent {} turned with no body inside the sense radius",
                    sample.id.get()
                );
            }
        }
    }
}

#[test]
fn the_pedestrian_crossing_benchmark_completes_without_tunnelling_or_nan() {
    for seed in 0..3u64 {
        let mut sim = sim(BENCHMARK, seed);
        let mut first_seen: std::collections::BTreeMap<u32, u64> =
            std::collections::BTreeMap::new();
        let mut pedestrian_overlaps = 0u32;
        let mut pedestrian_initiated_contacts = 0u32;
        for _ in 0..4000 {
            let tick = sim.time().tick();
            let before: Vec<AgentSample> = sim.snapshot(SnapshotDetail::Full).agents().to_vec();
            sim.step();
            let after = sim.snapshot(SnapshotDetail::Full);
            for sample in after.agents() {
                let motion = sample.motion.as_ref().expect("full detail");
                if motion.mode != AgentMode::Pedestrian {
                    continue;
                }
                first_seen.entry(sample.id.get()).or_insert(tick);
                let radius_m = motion.body_length_m * 0.5;
                assert!(
                    sample.position.is_finite()
                        && sample.heading_rad.is_finite()
                        && motion.speed_mps.is_finite(),
                    "seed {seed}: non-finite pedestrian state"
                );
                let desired_speed_mps = sim
                    .agent_pedestrian_profile(sample.id)
                    .expect("a demand pedestrian has a profile")
                    .desired_speed_mps;
                if let Some(previous) = before.iter().find(|prior| prior.id == sample.id) {
                    let step_m = (sample.position - previous.position).length();
                    assert!(
                        step_m <= desired_speed_mps * STEP_S + 1e-9,
                        "seed {seed}: agent {} moved {step_m} m in one step",
                        sample.id.get()
                    );
                }
                for other in after.agents() {
                    let other_motion = other.motion.as_ref().expect("full detail");
                    if other_motion.mode == AgentMode::Pedestrian {
                        if other.id <= sample.id {
                            continue;
                        }
                        let touching_m = radius_m + other_motion.body_length_m * 0.5;
                        if (other.position - sample.position).length() < touching_m - 1e-9 {
                            pedestrian_overlaps += 1;
                        }
                        continue;
                    }
                    // A pedestrian's own step must never make first contact
                    // with a body that is not itself already moving onto the
                    // pedestrian. A vehicle-initiated overlap is measured by
                    // this classification but asserted in `vehicle_yielding.rs`,
                    // which owns the vehicle-yielding slice.
                    let clearance_m = circle_box_clearance(
                        sample.position,
                        radius_m,
                        other.position,
                        other.heading_rad,
                        other_motion.body_length_m,
                        other_motion.body_width_m,
                    );
                    let (Some(previous_self), Some(previous_other)) = (
                        before.iter().find(|prior| prior.id == sample.id),
                        before.iter().find(|prior| prior.id == other.id),
                    ) else {
                        continue;
                    };
                    let clearance_before_m = circle_box_clearance(
                        previous_self.position,
                        radius_m,
                        previous_other.position,
                        previous_other.heading_rad,
                        other_motion.body_length_m,
                        other_motion.body_width_m,
                    );
                    if clearance_before_m < 0.0 || clearance_m >= 0.0 {
                        continue;
                    }
                    let stepped_into_it = circle_box_clearance(
                        sample.position,
                        radius_m,
                        previous_other.position,
                        previous_other.heading_rad,
                        other_motion.body_length_m,
                        other_motion.body_width_m,
                    ) < 0.0;
                    let swept_onto_it = circle_box_clearance(
                        previous_self.position,
                        radius_m,
                        other.position,
                        other.heading_rad,
                        other_motion.body_length_m,
                        other_motion.body_width_m,
                    ) < 0.0;
                    if stepped_into_it && !swept_onto_it {
                        pedestrian_initiated_contacts += 1;
                    }
                }
            }
        }
        assert_eq!(
            pedestrian_overlaps, 0,
            "seed {seed}: pedestrians overlapped"
        );
        assert_eq!(
            pedestrian_initiated_contacts, 0,
            "seed {seed}: a pedestrian stepped into a body that was not already moving onto it"
        );

        // No route deadlock in mixed traffic.
        let live_ids = live_ids(&sim);
        for (id, tick) in first_seen {
            if tick < 3000 {
                assert!(
                    !live_ids.contains(&id),
                    "seed {seed}: agent {id} never left the route (route deadlock)"
                );
            }
        }
        // The spacing cap is a documented backstop, not the normal regime.
        assert!(
            sim.pedestrian_cap_steps() < 4000 / 10,
            "seed {seed}: the spacing cap bound {} of 4000 steps",
            sim.pedestrian_cap_steps()
        );
        let summary = sim.finish();
        assert_eq!(summary.dropped(), 0, "seed {seed}: nominal demand was shed");
        assert!(summary.spawned() > 0 && summary.despawned() > 0);
    }
}

#[test]
fn the_pedestrian_controller_reproduces_a_run_for_the_same_seed() {
    fn trace(seed: u64) -> Vec<String> {
        let mut sim = sim(CROSSING, seed);
        let mut frames = Vec::new();
        for _ in 0..600 {
            let mut frame = String::new();
            for sample in pedestrians(&sim) {
                let motion = sample.motion.as_ref().expect("full detail");
                let target = sim
                    .pedestrian_waypoint(sample.id)
                    .expect("a live pedestrian has a target");
                frame.push_str(&format!(
                    "{}:{:?}:{:.12}:{:.12}:{:.12}:{} ",
                    sample.id.get(),
                    sample.position,
                    sample.heading_rad,
                    motion.speed_mps,
                    motion.path_distance_m,
                    target.arc_m(),
                ));
            }
            let events: Vec<Event> = sim.step().events().to_vec();
            frames.push(format!("{frame} | {events:?}"));
        }
        frames
    }

    assert_eq!(trace(11), trace(11));
    assert_ne!(trace(11), trace(12), "different seeds diverge");
}
