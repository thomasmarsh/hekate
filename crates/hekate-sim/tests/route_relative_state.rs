//! TAS-088: route-relative lateral agent state on a compiled facility.
//!
//! A steering mode that rides a compiled facility carries route coordinates —
//! its world pose projected onto the facility reference — plus the maneuver
//! state and the mode's resolved target clearance and horizon. The state is
//! initialized at spawn from the compiled projection, reprojected after each
//! integration, and exposed through a full snapshot. A pedestrian and a legacy
//! version-1 path-following agent carry no route state, so their output is
//! unchanged.
//!
//! The projection-signs unit check in the direction of each traversal lives in
//! `crate::agent`'s tests (`projection_mirrors_the_signed_offset_in_the_travel_frame`),
//! because the route frame is crate-internal.

use hekate_model::{
    AgentFamily, CompiledScenario, DemandId, DemandSpawnSource, FacilityDirection,
    MovementDirection, MovementId, PermissionEffect, PermissionKind, PermissionSource,
    parse_scenario_source, parse_scenario_source_v2,
};
use hekate_sim::{AgentId, AgentMode, Event, ManeuverState, RunConfig, Simulation, SnapshotDetail};

/// A version-2 scenario: one capsule mode on a compiled `bikeway` facility that
/// declares a lateral policy and a `pass` tactic, so the mode also compiles a
/// resolved target clearance and horizon. Its demand puts the mode on the
/// facility's reference path.
const ROUTE_STATE_V2: &str = r#"
{
  schema_version: 2,
  id: 'route_state_v2',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  boundaries: [
    { id: 'world', points: [
      { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
      { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
    ] },
  ],
  regions: [
    { id: 'band', points: [
      { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
      { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
    ] },
  ],
  facilities: [
    { id: 'bikeway', region: 'band', reference_path: 'guide',
      width_m: 3.0, nominal_direction: 'forward',
      access: { modes: [ 'rider' ] }, lateral_use: 'shared',
      lateral_policy: { passing_side: 'left' },
      speed_policy: { limit_mps: null } },
  ],
  movements: [
    { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
      direction: 'forward' },
  ],
  mode_templates: [
    {
      id: 'rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'pass' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 6.0, max: 6.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
      lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
    },
  ],
  permissions: [],
  maneuver_policy: {
    commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
  },
  demand: [
    { id: 'rider_inflow', mode: 'rider',
      spawn: { rate: {
        portal: 'entry',
        rate_per_hour: 900.0,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
      } } },
  ],
}
"#;

/// The target clearance the `rider` template authors.
const TARGET_CLEARANCE_M: f64 = 0.75;

/// The horizon the `rider` template authors.
const HORIZON_S: f64 = 4.0;

/// A legacy version-1 walking scenario, so absent route state can be observed.
const WALKING_V1: &str = r#"
{
  schema_version: 1,
  id: 'walking_guide_v1',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] } ],
  portals: [
    { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  population: {
    vehicle_count: 2,
    vehicle_speed_mps: 12.0,
    vehicle_spacing_m: 20.0,
    vehicle_length_m: 4.5,
    vehicle_width_m: 1.8,
  },
}
"#;

fn route_state_sim(seed: u64) -> Simulation {
    let source = parse_scenario_source_v2(ROUTE_STATE_V2).expect("the document is version 2");
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    Simulation::new(scenario, RunConfig::new(seed)).expect("the simulation builds")
}

fn walking_sim(seed: u64) -> Simulation {
    let source = parse_scenario_source(WALKING_V1).expect("the document parses");
    let scenario = CompiledScenario::compile(source).expect("the scenario compiles");
    Simulation::new(scenario, RunConfig::new(seed)).expect("the simulation builds")
}

/// A version-2 scenario whose rider enters at the reference end and travels the
/// facility's reverse traversal, against the facility's authored forward nominal
/// direction, with a `permit` statement binding the pair.
fn opposing_source() -> hekate_model::ScenarioSourceV2 {
    let mut source = parse_scenario_source_v2(ROUTE_STATE_V2).expect("the document is version 2");
    source.movements[0].from = "exit".to_owned();
    source.movements[0].to = "entry".to_owned();
    source.movements[0].direction = MovementDirection::Reverse;
    match &mut source.demand[0].spawn {
        DemandSpawnSource::Rate(rate) => rate.portal = "exit".to_owned(),
        DemandSpawnSource::Population(_) => panic!("the fixture's demand is a rate"),
    }
    source.permissions.push(PermissionSource {
        id: "contraflow".to_owned(),
        kind: PermissionKind::NominalDirection,
        holder: "rider".to_owned(),
        target: "bikeway".to_owned(),
        effect: PermissionEffect::Permit,
    });
    source
}

/// Build the opposing-traversal simulation of [`opposing_source`].
fn opposing_sim(seed: u64) -> Simulation {
    let scenario = CompiledScenario::compile_v2(opposing_source()).expect("the scenario compiles");
    Simulation::new(scenario, RunConfig::new(seed)).expect("the simulation builds")
}

/// Step until a live agent carries the wrong-way rule state and return it.
fn live_opposing_agent(sim: &mut Simulation) -> (AgentId, MovementDirection) {
    for _ in 0..1200 {
        sim.step();
        let frame = sim.snapshot(SnapshotDetail::Full);
        for sample in frame.agents() {
            let Some(route) = sample.motion.as_ref().and_then(|motion| motion.route_state) else {
                continue;
            };
            if let Some(direction) = route.opposing_direction {
                return (sample.id, direction);
            }
        }
    }
    panic!("a rider must reach the opposing traversal within 60 s");
}

/// Step until a steering agent is alive and return its position in the
/// full-snapshot agent order.
fn live_steering_agent(sim: &mut Simulation) -> usize {
    for _ in 0..1200 {
        sim.step();
        let found = sim
            .snapshot(SnapshotDetail::Full)
            .agents()
            .iter()
            .position(|sample| {
                sample
                    .motion
                    .as_ref()
                    .is_some_and(|motion| motion.mode == AgentMode::Vehicle)
            });
        if let Some(position) = found {
            return position;
        }
    }
    panic!("a steering agent must arrive within 60 s");
}

/// The scenario's demand selects a compiled facility route: the facility's
/// reference path is the movement's path and the mode may use the facility.
/// The mode is a wheeled family, so spawn initialization projects the entry
/// pose onto that reference.
#[test]
fn a_steering_agent_spawns_with_route_coordinates_from_the_facility_projection() {
    let sim = route_state_sim(0);
    let mode_id = sim
        .scenario()
        .demand_mode(DemandId::from_index(0))
        .expect("the demand names a mode template");
    let template = sim
        .scenario()
        .mode_template(mode_id)
        .expect("the mode template compiles");
    assert_eq!(template.family(), Some(AgentFamily::WheeledCapsule));

    let movement = sim
        .scenario()
        .movement(MovementId::from_index(0))
        .expect("the movement compiles");
    let facility = sim
        .scenario()
        .facilities()
        .iter()
        .find(|facility| facility.reference_path() == Some(movement.path()))
        .expect("the facility references the movement path");
    assert!(facility.permits_mode(mode_id));

    let mut sim = sim;
    let position = live_steering_agent(&mut sim);
    let frame = sim.snapshot(SnapshotDetail::Full);
    let sample = &frame.agents()[position];
    let motion = sample.motion.as_ref().expect("full detail carries motion");
    let route = motion
        .route_state
        .expect("a steering body carries route state");

    assert!(
        (route.s_m - motion.path_distance_m).abs() < 1e-9,
        "the spawn projection must agree with the reported progress: {} vs {}",
        route.s_m,
        motion.path_distance_m
    );
    assert!(
        route.d_m.abs() < 1e-9,
        "a body on the reference centreline has zero lateral offset, got {}",
        route.d_m
    );
    assert_eq!(route.maneuver_state, ManeuverState::Following);
    assert_eq!(route.target_clearance_m, Some(TARGET_CLEARANCE_M));
    assert_eq!(route.horizon_s, Some(HORIZON_S));
    // No maneuver is selected in this leaf, so its target state stays absent.
    assert_eq!(route.target_offset_m, None);
    assert_eq!(route.target_facility, None);
    assert_eq!(route.predicted_min_clearance_m, None);
    // Nominal travel carries no wrong-way rule state, so both columns stay
    // absent rather than defaulted.
    assert_eq!(route.perceived_rule, None);
    assert_eq!(route.opposing_direction, None);
}

/// The pipeline keeps the tactical coordinates consistent with the integrated
/// world pose: after each step the reprojection tracks the reported progress
/// and the offset stays on the reference centreline, in stable agent order.
#[test]
fn the_pipeline_reprojects_route_coordinates_after_integration() {
    let mut sim = route_state_sim(1);
    live_steering_agent(&mut sim);

    for _ in 0..40 {
        sim.step();
    }

    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let mut previous_id = None;
    let mut progressed = false;
    for sample in snapshot.agents() {
        // The snapshot reports live agents in stable ascending spawn order.
        if let Some(previous) = previous_id {
            assert!(sample.id > previous, "snapshot order must be stable");
        }
        previous_id = Some(sample.id);
        let motion = sample.motion.as_ref().expect("full detail carries motion");
        let Some(route) = motion.route_state else {
            continue;
        };
        assert_eq!(route.maneuver_state, ManeuverState::Following);
        assert!(
            (route.s_m - motion.path_distance_m).abs() < 1e-9,
            "reprojection must track the integrated pose: {} vs {}",
            route.s_m,
            motion.path_distance_m
        );
        assert!(route.d_m.abs() < 1e-9, "offset {}", route.d_m);
        progressed |= route.s_m > 0.0;
    }
    assert!(progressed, "the agents must have progressed");
}

/// The lateral policy the contract assigns this leaf reaches the store through
/// the compiled template, not a mode name: the scenario's maneuver policy is
/// present, the mode's lateral policy resolves, and the store records the
/// target clearance and horizon on every steering agent.
#[test]
fn the_records_carry_the_compiled_maneuver_policy() {
    let mut sim = route_state_sim(2);
    live_steering_agent(&mut sim);
    for _ in 0..20 {
        sim.step();
    }
    let mut observed = 0;
    for sample in sim.snapshot(SnapshotDetail::Full).agents() {
        let motion = sample.motion.as_ref().expect("full detail carries motion");
        if let Some(route) = motion.route_state {
            assert_eq!(route.target_clearance_m, Some(TARGET_CLEARANCE_M));
            assert_eq!(route.horizon_s, Some(HORIZON_S));
            observed += 1;
        }
    }
    assert!(observed > 0, "a steering agent must carry the policy");
}

/// A legacy version-1 path-following population carries no route state, so its
/// snapshot output is unchanged by this leaf.
#[test]
fn a_legacy_version_one_population_carries_no_route_state() {
    let mut sim = walking_sim(3);
    sim.step();
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    assert!(!snapshot.agents().is_empty(), "the population is placed");
    for sample in snapshot.agents() {
        let motion = sample.motion.as_ref().expect("full detail carries motion");
        assert_eq!(motion.mode, AgentMode::Vehicle);
        assert_eq!(motion.body_kind, hekate_model::BodyKind::Box);
        assert!(
            motion.route_state.is_none(),
            "a version-1 path-following body carries no route state"
        );
    }
}

/// The observer view carries route state only at full detail, so the position
/// view a renderer takes stays as small as it was.
#[test]
fn route_state_is_exposed_only_at_full_detail() {
    let mut sim = route_state_sim(4);
    live_steering_agent(&mut sim);
    sim.step();

    let full = sim.snapshot(SnapshotDetail::Full);
    let position = sim.snapshot(SnapshotDetail::Position);
    assert_eq!(full.agents().len(), position.agents().len());
    assert!(full.agents().iter().any(|sample| {
        sample
            .motion
            .as_ref()
            .is_some_and(|m| m.route_state.is_some())
    }));
    for sample in position.agents() {
        assert!(sample.motion.is_none());
    }
}

/// A passenger-car-shaped mode reaches the same route-coordinate path as a
/// narrow capsule: the store dispatches on the compiled physical family, never
/// on a mode or template id, so both wheeled families share one representation.
#[test]
fn a_box_body_mode_shares_the_route_coordinate_representation() {
    use hekate_model::{ModeBodySource, ProfileRangeSource};

    let mut source = parse_scenario_source_v2(ROUTE_STATE_V2).expect("the document is version 2");
    let template = &mut source.mode_templates[0];
    template.body = ModeBodySource::Box {
        length_m: ProfileRangeSource { min: 4.5, max: 4.5 },
        width_m: ProfileRangeSource { min: 1.8, max: 1.8 },
    };
    // A box on the wheeled family uses neither the steering-rate nor the
    // lateral-clearance parameter, and the lateral maneuver set is capsule-side.
    template.lateral = None;
    template
        .tactics
        .retain(|tactic| *tactic != hekate_model::TacticKind::Pass);
    template.profiles.remove("steering_rate_max_rad_s");
    template.profiles.remove("lateral_accel_max_mps2");
    template.profiles.remove("lateral_clearance_m");
    source.maneuver_policy = None;

    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    let mode_id = scenario
        .demand_mode(DemandId::from_index(0))
        .expect("the demand names a mode template");
    assert_eq!(
        scenario
            .mode_template(mode_id)
            .expect("the mode compiles")
            .family(),
        Some(AgentFamily::WheeledBox)
    );

    let mut sim = Simulation::new(scenario, RunConfig::new(6)).expect("the simulation builds");
    let position = live_steering_agent(&mut sim);
    let frame = sim.snapshot(SnapshotDetail::Full);
    let motion = frame.agents()[position]
        .motion
        .as_ref()
        .expect("full detail carries motion");
    assert!(
        motion.route_state.is_some(),
        "a wheeled box mode must carry route coordinates through the same path"
    );
}

/// A steering mode that authors no `lateral` object still carries its route
/// coordinates but records an absent target clearance and horizon, so the
/// absent-state rule holds for a mode with no free lateral motion.
#[test]
fn a_steering_mode_without_a_lateral_policy_carries_absent_targets() {
    let mut source = parse_scenario_source_v2(ROUTE_STATE_V2).expect("the document is version 2");
    // Drop the lateral object and the `pass` tactic so no maneuver policy is
    // required and the template compiles to the Increment 1 capability set.
    source.mode_templates[0].lateral = None;
    source.mode_templates[0]
        .tactics
        .retain(|tactic| *tactic != hekate_model::TacticKind::Pass);
    // The lateral-maneuver profile parameter is only legal for a lateral mode.
    source.mode_templates[0]
        .profiles
        .remove("lateral_accel_max_mps2");
    source.maneuver_policy = None;
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    let mode_id = scenario
        .demand_mode(DemandId::from_index(0))
        .expect("the demand names a mode template");
    let template = scenario
        .mode_template(mode_id)
        .expect("the mode template compiles");
    assert_eq!(template.lateral(), None);

    let mut sim = Simulation::new(scenario, RunConfig::new(5)).expect("the simulation builds");
    let position = live_steering_agent(&mut sim);
    let frame = sim.snapshot(SnapshotDetail::Full);
    let sample = &frame.agents()[position];
    let route = sample
        .motion
        .as_ref()
        .expect("full detail carries motion")
        .route_state
        .expect("a wheeled mode still carries route coordinates");
    assert_eq!(route.target_clearance_m, None);
    assert_eq!(route.horizon_s, None);
}

/// A body travelling against its object's rule direction carries the wrong-way
/// rule state: the direction it travels — the direction opposing the rule — and
/// the permission statement it perceived. Both ride the same route state the
/// agent already has, so a consumer reads why the body is on an opposing
/// traversal without a second lookup.
#[test]
fn an_opposing_traversal_carries_its_perceived_rule_and_direction() {
    let mut sim = opposing_sim(0);
    let (agent, direction) = live_opposing_agent(&mut sim);

    assert_eq!(direction, MovementDirection::Reverse);
    let frame = sim.snapshot(SnapshotDetail::Full);
    let sample = frame
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .expect("the observed agent is live");
    let route = sample
        .motion
        .as_ref()
        .expect("full detail carries motion")
        .route_state
        .expect("a steering body carries route state");
    assert_eq!(route.opposing_direction, Some(MovementDirection::Reverse));
    assert_eq!(
        route.perceived_rule,
        Some(PermissionEffect::Permit),
        "the applicable statement is the perceived rule"
    );
    assert!(
        route.s_m >= 0.0,
        "the state rides the object's extent: {}",
        route.s_m
    );
}

/// The rule state is derived from the traversal, not from a decision: the same
/// authored reverse traversal under an `either` facility carries no rule
/// direction and therefore no state, exactly as it opens no interval, so the
/// columns stay absent rather than defaulting to a direction.
#[test]
fn an_either_object_carries_no_rule_state() {
    let mut source = opposing_source();
    source.facilities[0].nominal_direction = FacilityDirection::Either;
    // A `nominal_direction` statement is about an opposite direction an
    // `either` object does not have, so the fixture authors none.
    source.permissions.clear();
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    let mut sim = Simulation::new(scenario, RunConfig::new(1)).expect("the simulation builds");

    let mut observed = 0;
    for _ in 0..1200 {
        sim.step();
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let Some(route) = sample.motion.as_ref().and_then(|motion| motion.route_state) else {
                continue;
            };
            assert_eq!(
                route.opposing_direction, None,
                "an object with no rule direction carries no rule state"
            );
            assert_eq!(route.perceived_rule, None);
            observed += 1;
        }
        if observed > 0 {
            break;
        }
    }
    assert!(observed > 0, "a rider must reach the facility");
}

/// The sampled rule state and the opposing-traversal boundary record the same
/// instant: on the step an entry boundary is emitted for a body, that body's
/// sample carries the boundary's own direction and perceived rule, so the two
/// surfaces cannot drift apart.
#[test]
fn the_sampled_rule_state_agrees_with_the_interval_boundary() {
    let mut sim = opposing_sim(2);
    let mut checked = 0;
    for _ in 0..1200 {
        let output = sim.step();
        let tick = output.time().tick();
        let entries: Vec<(AgentId, MovementDirection, Option<PermissionEffect>)> = output
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::OpposingTraversal {
                    agent,
                    direction,
                    perceived_rule,
                    entering: true,
                    ..
                } => Some((*agent, *direction, *perceived_rule)),
                _ => None,
            })
            .collect();
        if entries.is_empty() {
            continue;
        }
        let frame = sim.snapshot(SnapshotDetail::Full);
        for (agent, direction, perceived_rule) in entries {
            let sample = frame
                .agents()
                .iter()
                .find(|sample| sample.id == agent)
                .expect("an entering body is live at the boundary tick");
            let route = sample
                .motion
                .as_ref()
                .expect("full detail carries motion")
                .route_state
                .expect("a steering body carries route state");
            assert_eq!(route.opposing_direction, Some(direction), "at tick {tick}");
            assert_eq!(route.perceived_rule, perceived_rule, "at tick {tick}");
            checked += 1;
        }
        break;
    }
    assert!(checked > 0, "the fixture must enter an opposing traversal");
}
