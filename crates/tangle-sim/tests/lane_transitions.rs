//! TAS-095: facility transitions and safe aborts.
//!
//! A route-relative wheeled agent moves between facility bands only at the
//! contract's geometric handoff — the shared boundary between two side-by-side
//! bands (lateral) or a compiled connector coincidence (connector). The handoff
//! updates route and facility ownership in one step while the world pose is
//! unchanged: no despawn, no re-spawn, and no snap to the destination
//! reference, because the destination progress is the projection of that same
//! pose. The step reports each handoff once through
//! [`StepOutput::facility_transitions`]; `permitted: false` is the
//! forbidden-boundary fact.
//!
//! The focused fixtures drive the ordinary seam: demand places riders on a
//! compiled facility, and the kernel's own maneuver pass and physical advance
//! produce the transitions. Nothing here scripts a trajectory.
//!
//! The crossing bound and the outbound leg's hazard response are read from the
//! compiled adjacency: the handoff fires at the compiled shared boundary rather
//! than the source band's half-width, and the committed outbound corridor is
//! bounded by the two bands' combined edges, so a body or an edge beyond the
//! source band still brakes, holds, or aborts it.

use std::collections::BTreeMap;

use glam::DVec2;
use tangle_model::{CompiledScenario, FacilityId, MovementDirection, parse_scenario_source_v2};
use tangle_sim::{
    AgentId, AgentSample, FacilityTransitionRecord, LateralManeuverRequest, ManeuverAbortReason,
    ManeuverState, RouteStateSample, RunConfig, Simulation, SnapshotDetail, StepOutput,
    TransitionKind,
};

/// The default 0.05 s fixed step.
const DT: f64 = 0.05;

/// The source facility's reference length in the connector fixture.
const LENGTH_A: f64 = 100.0;

/// The largest world step observed for a rider, and the facility handoffs it
/// performed, over a run.
#[derive(Default)]
struct Trace {
    /// The facility handoffs each agent performed, in step order.
    transitions: Vec<FacilityTransitionRecord>,
    /// The largest per-step world position delta seen across all agents.
    max_step_m: f64,
}

/// Drive the simulation for `ticks` steps, recording every facility handoff and
/// the largest per-step world displacement.
fn drive(sim: &mut Simulation, ticks: u64) -> Trace {
    let mut trace = Trace::default();
    let mut previous: BTreeMap<AgentId, DVec2> = BTreeMap::new();
    for _ in 0..ticks {
        let handoffs: Vec<FacilityTransitionRecord> = {
            let output: StepOutput<'_> = sim.step();
            output.facility_transitions().to_vec()
        };
        trace.transitions.extend(handoffs);
        let frame = sim.snapshot(SnapshotDetail::Full);
        for sample in frame.agents() {
            if let Some(before) = previous.get(&sample.id) {
                trace.max_step_m = trace.max_step_m.max((sample.position - *before).length());
            }
            previous.insert(sample.id, sample.position);
        }
    }
    trace
}

/// The rider mode template shared by the fixtures: a capsule that steers
/// longitudinally and declares no lateral maneuver, so a connector handoff is
/// observed on the ordinary following path.
const RIDER_MODE: &str = r#"
    {
      id: 'rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 6.0, max: 6.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
    }"#;

/// Two collinear facilities joined by a connector, with a rider inflow.
fn connector_scenario(with_connector: bool) -> String {
    let connectors = if with_connector {
        r#"
  facility_connectors: [
    { id: 'a_to_b',
      from: { facility: 'a', direction: 'forward' },
      to: { facility: 'b', direction: 'forward' } },
  ],"#
    } else {
        ""
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'connector_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: {LENGTH_A}, y: 0.0 }} ] }},
    {{ id: 'guide_b', points: [ {{ x: {LENGTH_A}, y: 0.0 }}, {{ x: 200.0, y: 0.0 }} ] }},
  ],
  portals: [
    {{ id: 'entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'a_exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
    {{ id: 'exit', path: 'guide_b', end: 'end', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 210.0, y: -10.0 }},
      {{ x: 210.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: {LENGTH_A}, y: -1.5 }},
      {{ x: {LENGTH_A}, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_b', points: [
      {{ x: {LENGTH_A}, y: -1.5 }}, {{ x: 200.0, y: -1.5 }},
      {{ x: 200.0, y: 1.5 }}, {{ x: {LENGTH_A}, y: 1.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'b', region: 'band_b', reference_path: 'guide_b',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],{connectors}
  movements: [
    {{ id: 'through', from: 'entry', to: 'a_exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [ {RIDER_MODE} ],
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 360.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

fn build(scenario: &str) -> Simulation {
    let source = parse_scenario_source_v2(scenario).expect("the document is version 2");
    let compiled = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    Simulation::new(compiled, RunConfig::new(0)).expect("the simulation builds")
}

/// A rider mode that declares a `pass` tactic and a `lateral` policy, so a
/// requested cross-facility change of lane flows through the ordinary maneuver
/// pass and bounded steering, with the compiled prediction horizon chosen by the
/// caller.
fn lane_rider_mode(horizon_s: f64) -> String {
    format!(
        r#"
    {{
      id: 'rider',
      body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }},
        radius_m: {{ min: 0.35, max: 0.35 }} }},
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'pass' ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: 6.0, max: 6.0 }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_accel_max_mps2: {{ min: 2.0, max: 2.0 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
      lateral: {{ target_clearance_m: 0.75, horizon_s: {horizon_s:?} }},
    }}"#
    )
}

/// The same body and speed as the rider mode, with no `lateral` policy and no
/// lateral-acceleration limit: a wheeled mode whose route state carries no
/// lateral capability, so the shared maneuver stage must leave it alone.
const THROUGH_RIDER_MODE: &str = r#"
    {
      id: 'through_rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 6.0, max: 6.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 0.2, max: 0.2 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
    }"#;

/// The authored shape of the adjacent-bands fixture.
///
/// The two bands are side by side with the destination to the source's left.
/// `source_reference_y_m` and `destination_reference_y_m` place the two compiled
/// reference paths, so a caller can move a reference away from the middle of its
/// region: the compiled shared boundary comes from the regions, while the
/// compiled band edge comes from `width_m` around the reference, and the two
/// disagree exactly when a reference is offset inside its region.
struct AdjacentBands<'a> {
    /// The destination facility's authored nominal direction.
    destination_nominal: &'a str,
    /// The source reference path's lateral position in metres.
    source_reference_y_m: f64,
    /// The destination reference path's lateral position in metres.
    destination_reference_y_m: f64,
    /// The destination facility's compiled width in metres.
    destination_width_m: f64,
    /// The rider mode's compiled prediction horizon in seconds.
    rider_horizon_s: f64,
}

impl Default for AdjacentBands<'_> {
    fn default() -> Self {
        Self {
            destination_nominal: "forward",
            source_reference_y_m: 0.0,
            destination_reference_y_m: 3.0,
            destination_width_m: 3.0,
            rider_horizon_s: 2.0,
        }
    }
}

/// Two side-by-side bands (A at `y = 0`, B at `y = 3`) joined by an adjacency,
/// with a rider inflow on A.
///
/// `destination_nominal` fixes the destination facility's authored nominal
/// direction, so a reverse value makes the forward continuation a forbidden
/// boundary crossing.
fn adjacent_bands_scenario(destination_nominal: &str) -> String {
    adjacent_bands_document(AdjacentBands {
        destination_nominal,
        ..AdjacentBands::default()
    })
}

/// The adjacent-bands document for a chosen authored shape.
fn adjacent_bands_document(bands: AdjacentBands<'_>) -> String {
    let AdjacentBands {
        destination_nominal,
        source_reference_y_m,
        destination_reference_y_m,
        destination_width_m,
        rider_horizon_s,
    } = bands;
    let rider_mode = lane_rider_mode(rider_horizon_s);
    format!(
        r#"{{
  schema_version: 2,
  id: 'adjacent_bands_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: {source_reference_y_m:?} }}, {{ x: 200.0, y: {source_reference_y_m:?} }} ] }},
    {{ id: 'guide_b', points: [ {{ x: 0.0, y: {destination_reference_y_m:?} }}, {{ x: 200.0, y: {destination_reference_y_m:?} }} ] }},
  ],
  portals: [
    {{ id: 'entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 210.0, y: -10.0 }},
      {{ x: 210.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 200.0, y: -1.5 }},
      {{ x: 200.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_b', points: [
      {{ x: 0.0, y: 1.5 }}, {{ x: 200.0, y: 1.5 }},
      {{ x: 200.0, y: 4.5 }}, {{ x: 0.0, y: 4.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'b', region: 'band_b', reference_path: 'guide_b',
      width_m: {destination_width_m:?}, nominal_direction: '{destination_nominal}',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_adjacencies: [
    {{ id: 'a_beside_b', first: 'a', second: 'b', side: 'left' }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [ {rider_mode} ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 3600.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

/// The crossing-hazard fixture: the adjacent bands with a dense stream of
/// lateral-incapable wheeled bodies on the destination band, and, when
/// `source_through` is set, the same mode interleaved into the source band's
/// inflow so one facility carries both a lateral-capable and a lateral-incapable
/// mode.
///
/// The destination stream runs at twice the source inflow's rate with a short
/// time gap, so a body is always within one body length of a source rider and
/// the two move at the same speed. The rider mode predicts six seconds ahead, so
/// its outbound corridor reaches well into the destination band.
fn crossing_hazard_scenario(source_through: bool) -> String {
    let rider_mode = lane_rider_mode(6.0);
    let source_through_demand = if source_through {
        r#"    { id: 'through_inflow', mode: 'through_rider',
      spawn: { rate: {
        portal: 'entry',
        rate_per_hour: 3600.0,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
      } } },
"#
    } else {
        ""
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'crossing_hazard_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 200.0, y: 0.0 }} ] }},
    {{ id: 'guide_b', points: [ {{ x: 0.0, y: 3.0 }}, {{ x: 200.0, y: 3.0 }} ] }},
  ],
  portals: [
    {{ id: 'entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
    {{ id: 'b_entry', path: 'guide_b', end: 'start', width_m: 3.0 }},
    {{ id: 'b_exit', path: 'guide_b', end: 'end', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 210.0, y: -10.0 }},
      {{ x: 210.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 200.0, y: -1.5 }},
      {{ x: 200.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_b', points: [
      {{ x: 0.0, y: 1.5 }}, {{ x: 200.0, y: 1.5 }},
      {{ x: 200.0, y: 4.5 }}, {{ x: 0.0, y: 4.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider', 'through_rider' ] }}, lateral_use: 'shared',
      lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'b', region: 'band_b', reference_path: 'guide_b',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider', 'through_rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_adjacencies: [
    {{ id: 'a_beside_b', first: 'a', second: 'b', side: 'left' }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
    {{ id: 'b_through', from: 'b_entry', to: 'b_exit', path: 'guide_b', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [ {rider_mode}, {THROUGH_RIDER_MODE} ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 3600.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
{source_through_demand}    {{ id: 'destination_inflow', mode: 'through_rider',
      spawn: {{ rate: {{
        portal: 'b_entry',
        rate_per_hour: 7200.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'b_through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

/// Step until two riders are alive, returning them in ascending id order.
fn two_riders(sim: &mut Simulation) -> (AgentId, AgentId) {
    for _ in 0..900 {
        sim.step();
        let alive: Vec<AgentId> = sim
            .snapshot(SnapshotDetail::Position)
            .agents()
            .iter()
            .map(|sample| sample.id)
            .collect();
        if alive.len() >= 2 {
            return (alive[0], alive[1]);
        }
    }
    panic!("two riders must arrive within 45 s");
}

/// A requested cross-facility change of lane crosses the shared boundary and
/// hands the rider off to the adjacent band: the record names the lateral
/// handoff, and the world pose is continuous (no despawn, re-spawn, or snap).
#[test]
fn an_adjacent_lane_change_hands_the_rider_off_at_the_shared_boundary() {
    let mut sim = build(&adjacent_bands_scenario("forward"));
    let (leader, follower) = two_riders(&mut sim);
    assert!(
        sim.request_lateral_maneuver(
            follower,
            LateralManeuverRequest {
                // The destination band's own centreline.
                target_offset_m: 0.0,
                passed_body: leader,
                target_facility: Some(FacilityId::from_index(1)),
            },
        ),
        "a lateral-capable rider can request a change of lane"
    );

    let trace = drive(&mut sim, 400);
    let handoff = trace
        .transitions
        .iter()
        .find(|record| record.via == TransitionKind::Lateral)
        .copied()
        .expect("the committed rider crosses into the adjacent band");
    assert_eq!(handoff.from_facility, FacilityId::from_index(0));
    assert_eq!(handoff.to_facility, FacilityId::from_index(1));
    assert_eq!(handoff.side, tangle_sim::PassSide::Left);
    assert!(handoff.permitted, "the destination permits forward travel");
    assert!(
        handoff.d_m >= 1.5 - 1e-3,
        "the handoff fires as the body centre reaches the shared boundary: {}",
        handoff.d_m
    );
    assert!(
        trace.max_step_m <= 6.0 * DT + 1e-6,
        "no rider teleported across the handoff: max step {} m",
        trace.max_step_m
    );
}

/// An eligible within-facility maneuver completes and returns: the rider
/// attempts, commits, returns once the passed body is cleared, and reaches
/// `following` back at its own offset — the complete-and-return half of the
/// lifecycle, on the same facility.
#[test]
fn an_eligible_maneuver_completes_and_returns_to_following() {
    let mut sim = build(&adjacent_bands_scenario("forward"));
    let (leader, follower) = two_riders(&mut sim);
    // The passed body is behind the rider, so the completion guard already holds
    // once the claim is granted: commit, return, and settle.
    assert!(sim.request_lateral_maneuver(
        leader,
        LateralManeuverRequest {
            target_offset_m: 0.3,
            passed_body: follower,
            target_facility: None,
        },
    ));

    let mut edges = Vec::new();
    for _ in 0..300 {
        let output = sim.step();
        edges.extend(
            output
                .transitions()
                .iter()
                .filter(|transition| transition.agent == leader)
                .map(|transition| (transition.from, transition.to, transition.edge)),
        );
        if sim
            .snapshot(SnapshotDetail::Full)
            .agents()
            .iter()
            .find(|sample| sample.id == leader)
            .and_then(|sample| sample.motion.as_ref())
            .and_then(|motion| motion.route_state)
            .is_some_and(|route| route.maneuver_state == tangle_sim::ManeuverState::Following)
            && !edges.is_empty()
        {
            break;
        }
    }
    use tangle_sim::{ManeuverEdge, ManeuverState};
    assert_eq!(
        edges,
        [
            (
                ManeuverState::Following,
                ManeuverState::Preparing,
                ManeuverEdge::Attempted
            ),
            (
                ManeuverState::Preparing,
                ManeuverState::Committed,
                ManeuverEdge::Committed
            ),
            (
                ManeuverState::Committed,
                ManeuverState::Returning,
                ManeuverEdge::Completed
            ),
            (
                ManeuverState::Returning,
                ManeuverState::Following,
                ManeuverEdge::Completed
            ),
        ],
        "the full following -> preparing -> committed -> returning -> following table"
    );
}

/// A change of lane crosses at the *compiled* shared boundary, not at the
/// source band's half-width: the fixture's source reference runs half a metre
/// above the middle of its region, so the two differ by half a metre, and the
/// handoff fires at the region boundary they agree on.
#[test]
fn the_handoff_fires_at_the_compiled_shared_boundary() {
    let mut sim = build(&adjacent_bands_document(AdjacentBands {
        source_reference_y_m: 0.5,
        ..AdjacentBands::default()
    }));
    let (leader, follower) = two_riders(&mut sim);
    assert!(sim.request_lateral_maneuver(
        follower,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: leader,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));

    let trace = drive(&mut sim, 400);
    let handoff = trace
        .transitions
        .iter()
        .find(|record| record.via == TransitionKind::Lateral)
        .copied()
        .expect("the committed rider crosses into the adjacent band");
    // The two bands meet on `y = 1.5` and the source reference is `y = 0.5`, so
    // the shared boundary is 1.0 m from the reference in the rider's travel
    // frame — not the 1.5 m half-width of the compiled band. The handoff fires on
    // the first step the body centre reaches it, so it overshoots by at most one
    // step's lateral displacement.
    assert!(
        (handoff.d_m - 1.0).abs() < 0.05,
        "the handoff fires at the compiled shared boundary: {}",
        handoff.d_m
    );
    assert!(
        handoff.d_m < 1.5 - 0.1,
        "the source band's half-width is not the crossing bound: {}",
        handoff.d_m
    );
    assert!(
        trace.max_step_m <= 6.0 * DT + 1e-6,
        "no rider teleported across the handoff: max step {} m",
        trace.max_step_m
    );
}

/// The route-relative tactical state of one agent from a snapshot's samples.
fn route_state(agents: &[AgentSample], agent: AgentId) -> Option<RouteStateSample> {
    agents
        .iter()
        .find(|sample| sample.id == agent)
        .and_then(|sample| sample.motion.as_ref())
        .and_then(|motion| motion.route_state)
}

/// A destination band whose compiled edge closes before the crossing target is
/// reachable aborts the committed outbound leg on that edge.
///
/// The destination reference runs close to the shared boundary, so its compiled
/// band sits mostly beyond the rider: the destination still holds the requested
/// target, so the attempt is admissible, but the outbound corridor — which is
/// long enough to reach the crossing target — arrives at that edge with less
/// than the policy's minimum predicted clearance. The ordered response ends the
/// maneuver without crossing.
#[test]
fn a_closed_destination_band_edge_aborts_the_outbound_leg() {
    let mut sim = build(&adjacent_bands_document(AdjacentBands {
        destination_reference_y_m: 1.6,
        destination_width_m: 2.4,
        rider_horizon_s: 12.0,
        ..AdjacentBands::default()
    }));
    let (leader, follower) = two_riders(&mut sim);
    assert!(sim.request_lateral_maneuver(
        follower,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: leader,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));

    let mut aborts: Vec<Option<ManeuverAbortReason>> = Vec::new();
    let mut crossed = false;
    for _ in 0..400 {
        let output = sim.step();
        crossed |= output
            .facility_transitions()
            .iter()
            .any(|record| record.agent == follower && record.via == TransitionKind::Lateral);
        aborts.extend(
            output
                .transitions()
                .iter()
                .filter(|transition| {
                    transition.agent == follower && transition.to == ManeuverState::Aborted
                })
                .map(|transition| transition.reason),
        );
        if crossed || !aborts.is_empty() {
            break;
        }
    }
    assert!(
        !crossed,
        "a closed destination band edge never suffers an unavoidable crossing"
    );
    assert_eq!(
        aborts,
        vec![Some(ManeuverAbortReason::ClearanceLost)],
        "the band edge closes the outbound leg's predicted corridor"
    );
}

/// Step until a lateral-capable rider (a route state carrying a target
/// clearance) is alive, returning it.
fn first_lateral_capable(sim: &mut Simulation) -> Option<AgentId> {
    for _ in 0..900 {
        sim.step();
        let frame = sim.snapshot(SnapshotDetail::Full);
        let found = frame.agents().iter().find_map(|sample| {
            route_state(frame.agents(), sample.id)
                .filter(|route| route.target_clearance_m.is_some())
                .map(|_| sample.id)
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

/// The live body with the greatest progress ahead of `agent` in the world's `x`
/// direction, or `None` when nothing is ahead of it.
///
/// A passed body behind the rider completes the maneuver as soon as it commits,
/// so this fixture needs one ahead of it.
fn body_ahead(sim: &Simulation, agent: AgentId) -> Option<AgentId> {
    let frame = sim.snapshot(SnapshotDetail::Position);
    let own_x = frame
        .agents()
        .iter()
        .find(|sample| sample.id == agent)?
        .position
        .x;
    frame
        .agents()
        .iter()
        .filter(|sample| sample.id != agent && sample.position.x > own_x)
        .min_by(|left, right| left.position.x.total_cmp(&right.position.x))
        .map(|sample| sample.id)
}

/// A requested cross-facility change of lane whose destination band carries a
/// body alongside the rider aborts on the outbound leg's predicted clearance,
/// and never crosses.
///
/// The fixture streams lateral-incapable wheeled bodies along the destination
/// band at the same speed as the rider, so a body is always within one body
/// length of the rider while the outbound corridor — which reaches into the
/// destination band over its horizon — sweeps past it. The within-facility read
/// of the same body cannot see it that way: the body is beyond the source band's
/// own edge, which is exactly why the outbound leg needs the compiled
/// destination band and its shared boundary.
#[test]
fn a_destination_band_body_aborts_the_outbound_leg() {
    let mut sim = build(&crossing_hazard_scenario(false));
    let rider = first_lateral_capable(&mut sim).expect("a lateral-capable rider arrives");
    let passed = body_ahead(&sim, rider).expect("another body is ahead");
    assert!(sim.request_lateral_maneuver(
        rider,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: passed,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));

    let mut aborts: Vec<Option<ManeuverAbortReason>> = Vec::new();
    let mut crossed = false;
    for _ in 0..600 {
        let output = sim.step();
        crossed |= output
            .facility_transitions()
            .iter()
            .any(|record| record.agent == rider && record.via == TransitionKind::Lateral);
        aborts.extend(
            output
                .transitions()
                .iter()
                .filter(|transition| {
                    transition.agent == rider && transition.to == ManeuverState::Aborted
                })
                .map(|transition| transition.reason),
        );
        if crossed || !aborts.is_empty() {
            break;
        }
    }
    assert!(
        !crossed,
        "a destination band body holds the crossing back rather than overlapping it"
    );
    assert_eq!(
        aborts,
        vec![Some(ManeuverAbortReason::ClearanceLost)],
        "the outbound leg aborts on its predicted clearance"
    );
    // Ownership never moved: the rider still owns the source band, so the
    // aborted maneuver steers back to the offset it held.
    let route = route_state(sim.snapshot(SnapshotDetail::Full).agents(), rider)
        .expect("the rider carries route state");
    assert_eq!(route.maneuver_state, ManeuverState::Aborted);
}

/// One facility carrying a lateral-capable mode and a lateral-incapable one runs
/// the shared maneuver stage over both without touching the incapable agents.
///
/// The incapable mode's route state carries no lateral capability, so it is
/// never a maneuver candidate and never holds a maneuver; the capable rider's
/// own request still flows through the same stage.
#[test]
fn a_lateral_incapable_mode_on_a_shared_facility_never_maneuvers() {
    let mut sim = build(&crossing_hazard_scenario(true));
    let rider = first_lateral_capable(&mut sim).expect("a lateral-capable rider arrives");
    let passed = body_ahead(&sim, rider).expect("another body is ahead");
    assert!(sim.request_lateral_maneuver(
        rider,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: passed,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));

    let mut rider_maneuvers = 0;
    for _ in 0..300 {
        let output = sim.step();
        let maneuvering: Vec<AgentId> = output
            .transitions()
            .iter()
            .map(|transition| transition.agent)
            .collect();
        let frame = sim.snapshot(SnapshotDetail::Full);
        for agent in maneuvering {
            if agent == rider {
                rider_maneuvers += 1;
            } else {
                assert!(
                    route_state(frame.agents(), agent)
                        .is_some_and(|route| route.target_clearance_m.is_some()),
                    "only a lateral-capable body may maneuver"
                );
            }
        }
        for sample in frame.agents() {
            if let Some(route) = route_state(frame.agents(), sample.id)
                && route.target_clearance_m.is_none()
            {
                assert_eq!(
                    route.maneuver_state,
                    ManeuverState::Following,
                    "a lateral-incapable body never holds a maneuver"
                );
            }
        }
    }
    assert!(
        rider_maneuvers > 0,
        "the requested rider's maneuver ran through the shared stage"
    );
}

/// A change of lane into a band too narrow to hold the target is infeasible:
/// the destination band's own usable interval closes the corridor, so no
/// crossing is attempted.
#[test]
fn a_destination_band_too_narrow_closes_the_crossing() {
    let scenario = adjacent_bands_scenario("forward").replace(
        "      width_m: 3.0, nominal_direction: 'forward',\n      access: { modes: [ 'rider' ] }, lateral_use: 'shared',\n      speed_policy: { limit_mps: null } },\n  ],\n  facility_adjacencies",
        "      width_m: 1.5, nominal_direction: 'forward',\n      access: { modes: [ 'rider' ] }, lateral_use: 'shared',\n      speed_policy: { limit_mps: null } },\n  ],\n  facility_adjacencies",
    );
    let mut sim = build(&scenario);
    let (leader, follower) = two_riders(&mut sim);
    assert!(sim.request_lateral_maneuver(
        follower,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: leader,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));
    let trace = drive(&mut sim, 60);
    assert!(
        trace
            .transitions
            .iter()
            .all(|record| record.via == TransitionKind::Connector),
        "no lateral crossing is recorded for a band that cannot hold the target: {:?}",
        trace.transitions
    );
}

/// A requested change of lane into a band whose traversal the applicable rule
/// does not permit is prevented with the inspectable `boundary_forbidden`
/// reason, and no crossing is recorded.
#[test]
fn a_forbidden_lane_change_is_prevented_with_the_boundary_reason() {
    let mut sim = build(&adjacent_bands_scenario("reverse"));
    let (leader, follower) = two_riders(&mut sim);
    assert!(sim.request_lateral_maneuver(
        follower,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: leader,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));
    // The intent is decided on the next step: the destination traversal does not
    // permit the continuation, so the crossing is prevented and the reason is
    // recorded on the attempt.
    let output = sim.step();
    assert!(
        output.facility_transitions().is_empty(),
        "a forbidden destination is never crossed"
    );
    assert_eq!(
        sim.lateral_maneuver_reason(follower),
        Some(tangle_sim::ManeuverReason::BoundaryForbidden),
        "the preventable crossing records the contract's reason"
    );
    // And no later step invents the crossing either.
    let trace = drive(&mut sim, 40);
    assert!(
        trace.transitions.is_empty(),
        "no crossing is recorded for the forbidden destination: {:?}",
        trace.transitions
    );
}

/// A connector hands a rider off at its coincidence: route and facility
/// ownership change in one step, the record names the connector handoff, and the
/// world pose is continuous (no despawn, re-spawn, or snap).
#[test]
fn a_connector_hands_a_rider_off_at_its_coincidence() {
    let mut sim = build(&connector_scenario(true));
    let trace = drive(&mut sim, 900);

    let handoff = trace
        .transitions
        .iter()
        .find(|record| record.via == TransitionKind::Connector)
        .expect("a rider hands off through the connector");
    assert_eq!(handoff.from_direction, MovementDirection::Forward);
    assert_eq!(handoff.to_direction, MovementDirection::Forward);
    assert!(handoff.permitted, "the destination permits forward travel");
    assert!(
        (handoff.s_m - LENGTH_A).abs() < 1e-3,
        "the handoff happens at the source traversal's end: {}",
        handoff.s_m
    );
    assert!(
        trace.max_step_m <= 6.0 * DT + 1e-6,
        "no rider teleported across the handoff: max step {} m",
        trace.max_step_m
    );
    // Every rider that reaches the source end keeps travelling on the
    // destination, so the inflow produces several handoffs.
    assert!(
        trace.transitions.len() >= 2,
        "the inflow produces several connector handoffs: {}",
        trace.transitions.len()
    );
}

/// A rider on a facility with no connector continuation leaves the world at its
/// route end exactly as before: no handoff is invented without authored
/// topology.
#[test]
fn no_change_without_an_authored_connector() {
    let mut sim = build(&connector_scenario(false));
    let trace = drive(&mut sim, 900);
    assert!(
        trace.transitions.is_empty(),
        "no authored connector means no facility transition: {:?}",
        trace.transitions
    );
}

/// A mode that authors no lateral policy is not a maneuver candidate: the
/// request seam refuses it and the run records no transition, exactly as
/// Increment 1.
#[test]
fn no_change_without_an_authored_lateral_capability() {
    let mut sim = build(&connector_scenario(true));
    let (leader, follower) = two_riders(&mut sim);
    assert!(
        !sim.request_lateral_maneuver(
            follower,
            LateralManeuverRequest {
                target_offset_m: 0.0,
                passed_body: leader,
                target_facility: Some(FacilityId::from_index(1)),
            },
        ),
        "a rider with no authored lateral policy cannot request a maneuver"
    );
    let trace = drive(&mut sim, 120);
    assert!(
        trace.transitions.len() <= 1,
        "only the connector handoff occurs, no lateral maneuver: {:?}",
        trace.transitions
    );
    assert!(
        trace
            .transitions
            .iter()
            .all(|record| record.via == TransitionKind::Connector),
        "no lateral handoff is recorded without a lateral policy"
    );
}
