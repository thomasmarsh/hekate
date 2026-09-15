//! TAS-127: deterministic simultaneous-claim resolution.
//!
//! The arbitration itself is pinned by the crate's own unit tests, where a claim
//! batch can be fed in every insertion order. This file drives the same
//! determinism through the public seam: three riders contest one corridor
//! through [`Simulation::request_lateral_maneuver`], and reversing the order the
//! requests are recorded in must not change the winner, the loser set, or the
//! transition stream. A second test runs one fixture twice at a fixed seed and
//! asserts the whole transition and [`Event::Maneuver`] streams are identical.
//!
//! The recorded request order is the only order a caller can observe: the
//! kernel builds its claim batch in ascending agent id, so an insertion-order
//! perturbation is reachable only through the crate's unit tests, and a
//! declaration-order reversal permutes the ids themselves and moves each demand
//! source's keyed random stream. Reversing the recorded requests changes
//! neither, so it is the order-independence proof this file owns.

use hekate_model::{CompiledScenario, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, Event, LateralManeuverRequest, ManeuverAbortReason, ManeuverEdge, ManeuverState,
    RunConfig, Simulation, SnapshotDetail,
};

/// The target offset a requested maneuver steers toward: inside the usable
/// corridor of the fixture's 3.0 m band and above its band-edge clearance for
/// the 0.75 m target clearance.
const TARGET_OFFSET_M: f64 = 0.3;

/// A version-2 bikeway with a commit policy and one rider per second, so a test
/// can wait for a passed body and three claimants that would contest one
/// corridor.
///
/// This mirrors the fixture in `maneuver_lifecycle.rs`; it is duplicated rather
/// than widened because a claim permutation needs four live riders, which that
/// file's two-claimant scenarios do not place.
fn bikeway_v2() -> &'static str {
    r#"
{
  schema_version: 2,
  id: 'claims_v2',
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

fn rider_sim() -> Simulation {
    let source = parse_scenario_source_v2(bikeway_v2()).expect("the document is version 2");
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds")
}

/// Step until at least `count` riders are alive, so a test has a body to pass
/// and enough claimants to contest one corridor. Every rider here holds the
/// same desired speed, so no rider selects the maneuver tactic on its own and
/// the recorded requests are the only intents in the run.
fn riders(sim: &mut Simulation, count: usize) -> Vec<AgentId> {
    for _ in 0..400 {
        sim.step();
        let alive: Vec<AgentId> = sim
            .snapshot(SnapshotDetail::Position)
            .agents()
            .iter()
            .map(|sample| sample.id)
            .collect();
        if alive.len() >= count {
            return alive;
        }
    }
    panic!("{count} riders must arrive within 20 s");
}

/// A rider's maneuver state from the full snapshot.
fn maneuver_state(sim: &Simulation, agent: AgentId) -> ManeuverState {
    let frame = sim.snapshot(SnapshotDetail::Full);
    let sample = frame
        .agents()
        .iter()
        .find(|sample| sample.id == agent)
        .expect("the rider is live");
    sample
        .motion
        .as_ref()
        .and_then(|motion| motion.route_state)
        .expect("a steering rider carries route state")
        .maneuver_state
}

/// Record one maneuver request from each claimant for the same passed body.
fn request_all(sim: &mut Simulation, passed: AgentId, claimants: &[AgentId]) {
    for &rider in claimants {
        assert!(
            sim.request_lateral_maneuver(
                rider,
                LateralManeuverRequest {
                    target_offset_m: TARGET_OFFSET_M,
                    passed_body: passed,
                    target_facility: None,
                },
            ),
            "a live rider with route state can request a maneuver"
        );
    }
}

/// Three claimants for one corridor are decided by the batch: the same physical
/// rider commits, both others abort with the documented reason, and reversing
/// the order the requests were recorded in changes nothing. The claimants sit
/// at distinct distances, so the winner is decided by the key's distance clause
/// and the same *physical* rider wins under the reversal.
#[test]
fn a_three_agent_claim_resolves_to_the_same_winner_under_reversed_request_order() {
    let run = |reverse: bool| {
        let mut sim = rider_sim();
        let live = riders(&mut sim, 4);
        let passed = live[0];
        let nearer = live[1];
        let middle = live[2];
        let farther = live[3];
        let order = if reverse {
            [farther, middle, nearer]
        } else {
            [nearer, middle, farther]
        };
        request_all(&mut sim, passed, &order);

        // Both claimants prepare on the first decision and claim on the second.
        let mut transitions = Vec::new();
        for _ in 0..2 {
            let output = sim.step();
            transitions.extend(output.transitions().iter().copied());
        }
        (
            transitions,
            maneuver_state(&sim, nearer),
            maneuver_state(&sim, middle),
            maneuver_state(&sim, farther),
        )
    };

    let (forward, forward_nearer, forward_middle, forward_farther) = run(false);
    let (reversed, reversed_nearer, reversed_middle, reversed_farther) = run(true);

    assert_eq!(
        (forward_nearer, forward_middle, forward_farther),
        (
            ManeuverState::Committed,
            ManeuverState::Aborted,
            ManeuverState::Aborted
        ),
        "the nearest claimant commits and both farther claimants abort"
    );
    assert_eq!(
        (forward_nearer, forward_middle, forward_farther),
        (reversed_nearer, reversed_middle, reversed_farther),
        "the same physical rider wins under the reversed request order"
    );
    assert_eq!(
        forward, reversed,
        "the batch is order independent through the public seam"
    );

    let aborted: Vec<ManeuverAbortReason> = forward
        .iter()
        .filter(|transition| transition.edge == ManeuverEdge::Aborted)
        .filter_map(|transition| transition.reason)
        .collect();
    assert_eq!(
        aborted,
        [
            ManeuverAbortReason::ClaimRejected,
            ManeuverAbortReason::ClaimRejected
        ],
        "both losing claimants abort with the documented reason"
    );
}

/// One fixture run twice at the same seed produces identical transition and
/// [`Event::Maneuver`] streams: the claim path consumes no unordered collection
/// and no incidental random draw, so a fixed seed repeats exactly.
#[test]
fn a_claim_run_is_identical_across_fixed_seed_repetitions() {
    let run = || {
        let mut sim = rider_sim();
        let live = riders(&mut sim, 4);
        request_all(&mut sim, live[0], &[live[1], live[2], live[3]]);

        let mut transitions = Vec::new();
        let mut events = Vec::new();
        for _ in 0..40 {
            let output = sim.step();
            // The within-tick key order still holds with the claim batch in the
            // buffer: ascending agent, then kind order, then the event key.
            for window in output.events().windows(2) {
                assert!(
                    window[0].order_key() <= window[1].order_key(),
                    "the buffer is non-decreasing by the documented order key"
                );
            }
            transitions.extend(output.transitions().iter().copied());
            events.extend(output.events().iter().cloned());
        }
        (transitions, events)
    };

    let (first_transitions, first_events) = run();
    let (second_transitions, second_events) = run();
    assert!(
        !first_transitions.is_empty(),
        "the run recorded claim decisions to compare"
    );
    assert!(
        first_events
            .iter()
            .any(|event| matches!(event, Event::Maneuver { .. })),
        "the repetition covers the maneuver event surface"
    );
    assert_eq!(
        first_transitions, second_transitions,
        "the transition stream repeats exactly at a fixed seed"
    );
    assert_eq!(
        first_events, second_events,
        "the event stream repeats exactly at a fixed seed"
    );
}
