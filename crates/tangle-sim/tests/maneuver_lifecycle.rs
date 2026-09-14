//! TAS-091: the public seam of the lateral maneuver lifecycle.
//!
//! A tactical leaf records a [`LateralManeuverRequest`] for an agent, and the
//! kernel fixes the target and a candidate corridor, arbitrates the step's
//! corridor claims as a batch, and drives the maneuver to its completion,
//! return, or abort. Every state change is reported once through
//! [`StepOutput::transitions`], and the maneuver state is observable on the full
//! snapshot.
//!
//! This test drives the seam through the public API only: a version-2 bikeway
//! whose demand places riders on the compiled facility, and requests recorded
//! for those riders. The claim-level tie-breaks, the target despawn, the hold
//! timeouts, and the committed hazard are pinned by the crate's own unit tests,
//! where a fixture can place bodies exactly; this file pins the seam itself, the
//! recorded transitions, and the order independence a caller can observe.

use tangle_model::{CompiledScenario, parse_scenario_source, parse_scenario_source_v2};
use tangle_sim::{
    AgentId, LateralManeuverRequest, ManeuverAbortReason, ManeuverEdge, ManeuverState, RunConfig,
    Simulation, SnapshotDetail, StepOutput,
};

/// The target offset a requested maneuver steers toward: inside the usable
/// corridor of the fixture's 3.0 m band and above its band-edge clearance for
/// the 0.75 m target clearance.
const TARGET_OFFSET_M: f64 = 0.3;

/// A version-2 bikeway with a commit policy and one rider per second, so two
/// riders are always live with a steady gap between them.
fn bikeway_v2() -> &'static str {
    r#"
{
  schema_version: 2,
  id: 'maneuver_lifecycle_v2',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 600.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.0 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.0 },
  ],
  boundaries: [
    { id: 'world', points: [
      { x: -10.0, y: -10.0 }, { x: 610.0, y: -10.0 },
      { x: 610.0, y: 10.0 }, { x: -10.0, y: 10.0 },
    ] },
  ],
  regions: [
    { id: 'band', points: [
      { x: 0.0, y: -1.5 }, { x: 600.0, y: -1.5 },
      { x: 600.0, y: 1.5 }, { x: 0.0, y: 1.5 },
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
      lateral: { target_clearance_m: 0.75, horizon_s: 2.0 },
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
        rate_per_hour: 3600.0,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
      } } },
  ],
}
"#
}

/// A legacy version-1 walking population: no route state, no compiled facility,
/// and no maneuver policy at all.
fn walking_v1() -> &'static str {
    r#"
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
"#
}

fn rider_sim() -> Simulation {
    let source = parse_scenario_source_v2(bikeway_v2()).expect("the document is version 2");
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds")
}

/// Step until three riders are alive, so a test has a body to pass and two
/// claimants that would contest one corridor.
fn three_riders(sim: &mut Simulation) -> Vec<AgentId> {
    for _ in 0..400 {
        sim.step();
        let alive: Vec<AgentId> = sim
            .snapshot(SnapshotDetail::Position)
            .agents()
            .iter()
            .map(|sample| sample.id)
            .collect();
        if alive.len() >= 3 {
            return alive;
        }
    }
    panic!("three riders must arrive within 20 s");
}

/// A rider's maneuver state, target, and predicted clearance from the full
/// snapshot.
fn route_state(
    sim: &Simulation,
    agent: AgentId,
) -> (ManeuverState, Option<f64>, Option<f64>, Option<f64>) {
    let frame = sim.snapshot(SnapshotDetail::Full);
    let sample = frame
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .expect("the rider is live");
    let route = sample
        .motion
        .as_ref()
        .and_then(|motion| motion.route_state)
        .expect("a steering rider carries route state");
    (
        route.maneuver_state,
        route.target_offset_m,
        route.predicted_min_clearance_m,
        Some(route.d_m),
    )
}

/// The edges a step recorded for one agent, in order.
fn edges_for(output: &StepOutput<'_>, agent: AgentId) -> Vec<ManeuverEdge> {
    output
        .transitions()
        .iter()
        .filter(|transition| transition.agent == agent)
        .map(|transition| transition.edge)
        .collect()
}

/// A requested maneuver prepares, commits on the granted claim, and records one
/// transition per edge, with the target fixed and the clearance predicted from
/// the compiled policy.
#[test]
fn a_requested_maneuver_prepares_then_commits_with_one_record_per_edge() {
    let mut sim = rider_sim();
    let riders = three_riders(&mut sim);
    let maneuvering = riders[0];
    let passed = riders[1];

    assert!(
        sim.request_lateral_maneuver(
            maneuvering,
            LateralManeuverRequest {
                target_offset_m: TARGET_OFFSET_M,
                passed_body: passed,
                target_facility: None,
            },
        ),
        "a rider with route state and a compiled policy can request a maneuver"
    );

    // `following -> preparing`: a target and a candidate corridor are fixed, and
    // the maneuver displaces nothing at all.
    let output = sim.step();
    assert_eq!(edges_for(&output, maneuvering), [ManeuverEdge::Attempted]);
    let (state, target, predicted, offset) = route_state(&sim, maneuvering);
    assert_eq!(state, ManeuverState::Preparing);
    assert_eq!(target, Some(TARGET_OFFSET_M));
    assert!(predicted.expect("the attempt predicted a clearance") > 0.75);

    // `preparing -> committed`: the claim was granted, one decision later.
    let output = sim.step();
    assert_eq!(edges_for(&output, maneuvering), [ManeuverEdge::Committed]);
    let (state, ..) = route_state(&sim, maneuvering);
    assert_eq!(state, ManeuverState::Committed);

    // The committed rider steers toward its fixed target, and no later step
    // changes that target.
    for _ in 0..20 {
        sim.step();
    }
    let (state, target, _, after) = route_state(&sim, maneuvering);
    assert_eq!(
        target,
        Some(TARGET_OFFSET_M),
        "the committed target is fixed"
    );
    assert!(
        after.expect("offset") > offset.expect("offset"),
        "the committed rider displaces toward its target"
    );
    assert!(
        matches!(state, ManeuverState::Committed | ManeuverState::Returning),
        "a maneuver with an uncleared obstacle stays committed or returns: {state:?}"
    );
}

/// Two claimants for the same corridor are decided by the batch, and the same
/// claimant wins whichever order the requests were recorded in: a request is an
/// input to the batch, never a priority.
#[test]
fn the_same_claimant_wins_whichever_order_the_requests_arrive_in() {
    let run = |reverse: bool| {
        let mut sim = rider_sim();
        let riders = three_riders(&mut sim);
        let passed = riders[0];
        let nearer = riders[1];
        let farther = riders[2];
        let requests = if reverse {
            [farther, nearer]
        } else {
            [nearer, farther]
        };
        for rider in requests {
            assert!(sim.request_lateral_maneuver(
                rider,
                LateralManeuverRequest {
                    target_offset_m: TARGET_OFFSET_M,
                    passed_body: passed,
                    target_facility: None,
                },
            ));
        }
        // Both prepare on the first decision and claim on the second.
        let mut aborted = Vec::new();
        for _ in 0..2 {
            let output = sim.step();
            aborted.extend(
                output
                    .transitions()
                    .iter()
                    .filter(|transition| transition.edge == ManeuverEdge::Aborted)
                    .map(|transition| transition.reason),
            );
        }
        let (nearer_state, ..) = route_state(&sim, nearer);
        let (farther_state, ..) = route_state(&sim, farther);
        (nearer_state, farther_state, aborted)
    };

    let forward = run(false);
    let reversed = run(true);
    assert_eq!(forward, reversed, "the batch is order independent");
    assert_eq!(forward.0, ManeuverState::Committed);
    assert_eq!(forward.1, ManeuverState::Aborted);
    assert_eq!(
        forward.2,
        [Some(ManeuverAbortReason::ClaimRejected)],
        "the losing claimant aborts with the documented reason"
    );
}

/// An aborted claimant never displaced, and a maneuver that ends returns the
/// rider to `following` at its own offset with no target left fixed.
#[test]
fn an_aborted_claimant_returns_to_following_at_its_own_offset() {
    let mut sim = rider_sim();
    let riders = three_riders(&mut sim);
    let passed = riders[0];
    let nearer = riders[1];
    let farther = riders[2];
    for rider in [nearer, farther] {
        assert!(sim.request_lateral_maneuver(
            rider,
            LateralManeuverRequest {
                target_offset_m: TARGET_OFFSET_M,
                passed_body: passed,
                target_facility: None,
            },
        ));
    }
    let mut offset_before = None;
    for _ in 0..2 {
        let (_, _, _, offset) = route_state(&sim, farther);
        offset_before = offset;
        sim.step();
    }
    let (state, ..) = route_state(&sim, farther);
    assert_eq!(state, ManeuverState::Aborted);

    let mut edges = Vec::new();
    let mut followed = false;
    for _ in 0..400 {
        let output = sim.step();
        edges.extend(edges_for(&output, farther));
        if route_state(&sim, farther).0 == ManeuverState::Following {
            followed = true;
            break;
        }
    }
    assert!(followed, "an aborted maneuver returns to following");
    assert_eq!(edges, [ManeuverEdge::Aborted]);
    let (state, target, predicted, offset) = route_state(&sim, farther);
    assert_eq!(state, ManeuverState::Following);
    assert_eq!(target, None, "the aborted target is cleared");
    assert_eq!(predicted, None, "no prediction survives the maneuver");
    assert!(
        (offset.expect("offset") - offset_before.expect("offset")).abs() <= 1e-3,
        "the rider returns to the offset it held before the attempt"
    );
}

/// The request seam is closed for a run that has no maneuver machinery: a
/// version-1 population carries no route state, and the run records no
/// transition at all.
#[test]
fn a_version_one_run_refuses_a_maneuver_request_and_records_nothing() {
    let source = parse_scenario_source(walking_v1()).expect("the document parses");
    let scenario = CompiledScenario::compile(source).expect("the scenario compiles");
    let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds");

    assert!(
        !sim.request_lateral_maneuver(
            AgentId::from_index(0),
            LateralManeuverRequest {
                target_offset_m: TARGET_OFFSET_M,
                passed_body: AgentId::from_index(1),
                target_facility: None,
            },
        ),
        "a legacy path follower has no route state to maneuver with"
    );
    for _ in 0..20 {
        let output = sim.step();
        assert!(
            output.transitions().is_empty(),
            "a run with no maneuver policy records no transition"
        );
    }
}

/// The request seam refuses a target that cannot be a passed obstacle: an agent
/// cannot pass itself, and a slot that cannot maneuver has no request to record.
#[test]
fn the_request_seam_refuses_an_unusable_target() {
    let mut sim = rider_sim();
    let riders = three_riders(&mut sim);
    let rider = riders[0];
    assert!(
        !sim.request_lateral_maneuver(
            rider,
            LateralManeuverRequest {
                target_offset_m: TARGET_OFFSET_M,
                passed_body: rider,
                target_facility: None,
            },
        ),
        "an agent cannot pass itself"
    );
    assert!(
        !sim.request_lateral_maneuver(
            AgentId::from_index(10_000),
            LateralManeuverRequest {
                target_offset_m: TARGET_OFFSET_M,
                passed_body: riders[1],
                target_facility: None,
            },
        ),
        "a slot that is not a live agent has no route state to maneuver with"
    );
}
