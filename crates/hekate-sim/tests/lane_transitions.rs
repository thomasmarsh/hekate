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
use hekate_model::{CompiledScenario, FacilityId, MovementDirection, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, AgentSample, FacilityTransitionRecord, LateralManeuverRequest, ManeuverAbortReason,
    ManeuverEdge, ManeuverState, RouteStateSample, RunConfig, Simulation, SnapshotDetail,
    StepOutput, TransitionKind,
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

/// The target offset a rider in the return-obstruction fixture steers toward:
/// inside the 3.0 m band's usable corridor for its body and clearance.
/// The target offset a rider in the return-obstruction fixture steers toward:
/// inside the 3.0 m band's usable corridor for its body and clearance.
const RETURN_TARGET_M: f64 = 0.5;

/// The rider mode of the return-obstruction fixture: the same capsule as the
/// other fixtures, with a target clearance small enough that the slower lane
/// the rider passes out of and the oncoming lane it returns across both bind
/// inside one band.
const RETURN_RIDER_MODE: &str = r#"
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
      lateral: { target_clearance_m: 0.5, horizon_s: 2.0 },
    }"#;

/// The slower body of the return-obstruction fixture: a capsule that declares no
/// lateral maneuver, so it is never a maneuver candidate and only ever enters a
/// rider's prediction as a predicted body.
const RETURN_COMPANION_MODE: &str = r#"
    {
      id: 'companion',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 1.0, max: 1.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 0.2, max: 0.2 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
    }"#;

/// The return-obstruction fixture: a lateral-capable rider stream on a compiled
/// bikeway, a slower stream on a parallel lane the rider passes, and an oncoming
/// stream on a second parallel lane the rider meets after the pass.
///
/// Every body rides a path of its own, so no companion is ever a leader and none
/// displaces the rider: the only way the rider can read one is through the
/// maneuver's ordinary prediction. The slower lane lies 2.0 m to the rider's
/// right, outside the corridor the rider returns across, and the oncoming lane
/// lies 1.0 m to its right, inside it, so the return is clear until the rider
/// reaches the oncoming lane's stretch and clear again once it has left it.
fn return_obstruction_scenario() -> String {
    format!(
        r#"{{
  schema_version: 2,
  id: 'return_obstruction_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 600.0, y: 0.0 }} ] }},
    {{ id: 'slow_lane', points: [ {{ x: 40.0, y: -2.0 }}, {{ x: 640.0, y: -2.0 }} ] }},
    {{ id: 'oncoming_lane', points: [ {{ x: 100.0, y: -1.0 }}, {{ x: 160.0, y: -1.0 }} ] }},
  ],
  portals: [
    {{ id: 'entry', path: 'guide', end: 'start', width_m: 3.0 }},
    {{ id: 'exit', path: 'guide', end: 'end', width_m: 3.0 }},
    {{ id: 'slow_entry', path: 'slow_lane', end: 'start', width_m: 3.0 }},
    {{ id: 'slow_exit', path: 'slow_lane', end: 'end', width_m: 3.0 }},
    {{ id: 'onc_entry', path: 'oncoming_lane', end: 'end', width_m: 3.0 }},
    {{ id: 'onc_exit', path: 'oncoming_lane', end: 'start', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 650.0, y: -10.0 }},
      {{ x: 650.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 600.0, y: -1.5 }},
      {{ x: 600.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'bikeway', region: 'band', reference_path: 'guide',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
      direction: 'forward' }},
    {{ id: 'slow_through', from: 'slow_entry', to: 'slow_exit',
      path: 'slow_lane', priority: 0, direction: 'forward' }},
    {{ id: 'onc_through', from: 'onc_entry', to: 'onc_exit',
      path: 'oncoming_lane', priority: 0, direction: 'reverse' }},
  ],
  mode_templates: [ {RETURN_RIDER_MODE}, {RETURN_COMPANION_MODE} ],
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
    {{ id: 'slow_inflow', mode: 'companion',
      spawn: {{ rate: {{
        portal: 'slow_entry',
        rate_per_hour: 180.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'slow_through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'oncoming_inflow', mode: 'companion',
      spawn: {{ rate: {{
        portal: 'onc_entry',
        rate_per_hour: 360.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'onc_through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

/// The authored shape of the cross-facility pass fixture.
///
/// `source_reference_y_m` and `destination_reference_y_m` place the two
/// compiled reference paths inside the two side-by-side regions, so a caller can
/// move a reference away from the middle of its region: the compiled shared
/// boundary comes from the regions while each band's compiled band comes from
/// `width_m` around the reference, and the two disagree exactly when a reference
/// is offset inside its region.
struct CrossingPass {
    /// The source reference path's lateral position in metres.
    source_reference_y_m: f64,
    /// The destination reference path's lateral position in metres.
    destination_reference_y_m: f64,
    /// Whether the destination band carries the obstruction stream the return
    /// leg crosses back through.
    obstruct_return: bool,
}

impl Default for CrossingPass {
    fn default() -> Self {
        Self {
            source_reference_y_m: 0.0,
            destination_reference_y_m: 3.0,
            obstruct_return: false,
        }
    }
}

/// The stretch of the destination band the obstruction stream occupies.
///
/// The stretch starts downstream of the outbound crossing and of the entry leg
/// into the destination band, so only the return leg reads it: the entry leg
/// holds its crossing without a prediction, and the committed leg's own lane
/// stays clear of the stream's.
const RETURN_OBSTRUCTION_ZONE_M: std::ops::RangeInclusive<f64> = 45.0..=85.0;

/// Two side-by-side bands joined by an adjacency, with a lateral-capable rider
/// inflow on the source band, a slower body stream on a parallel lane outside
/// the source band, and, when the shape asks for it, a stream of rider-speed
/// bodies along the stretch of the destination band a return crosses back
/// through.
///
/// The source lane is 72 m apart in the rider's own inflow, so a returned rider
/// reads no follower of its own in the corridor it returns through: every body
/// the return can read is one the fixture placed there.
fn crossing_pass_scenario(pass: CrossingPass) -> String {
    let CrossingPass {
        source_reference_y_m: source_y_m,
        destination_reference_y_m: destination_y_m,
        obstruct_return,
    } = pass;
    let rider_mode = lane_rider_mode(6.0);
    // The obstruction lane lies in the destination band, one lane inside the
    // committed offset, so the committed leg's own lane clearance stays above
    // the mode's target and only the return corridor reaches the stream.
    let block_lane_y_m = destination_y_m - 1.8;
    let (block_lane, block_portals, block_movement, block_demand) = if obstruct_return {
        (
            r#"    { id: 'block_lane', points: [ { x: BLOCK_START, y: BLOCK_Y }, { x: BLOCK_END, y: BLOCK_Y } ] },
"#,
            r#"    { id: 'block_entry', path: 'block_lane', end: 'start', width_m: 3.0 },
    { id: 'block_exit', path: 'block_lane', end: 'end', width_m: 3.0 },
"#,
            r#"    { id: 'block_through', from: 'block_entry', to: 'block_exit',
      path: 'block_lane', priority: 0, direction: 'forward' },
"#,
            r#"    { id: 'block_inflow', mode: 'companion',
      spawn: { rate: {
        portal: 'block_entry',
        rate_per_hour: 1500.0,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'block_through', weight: 1.0 } ] },
      } } },
"#,
        )
    } else {
        ("", "", "", "")
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'crossing_pass_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: {source_y_m:?} }}, {{ x: 400.0, y: {source_y_m:?} }} ] }},
    {{ id: 'slow_lane', points: [ {{ x: 40.0, y: SLOW_Y }}, {{ x: 640.0, y: SLOW_Y }} ] }},
    {{ id: 'guide_b', points: [ {{ x: 0.0, y: {destination_y_m:?} }}, {{ x: 400.0, y: {destination_y_m:?} }} ] }},
{block_lane}  ],
  portals: [
    {{ id: 'entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
    {{ id: 'slow_entry', path: 'slow_lane', end: 'start', width_m: 3.0 }},
    {{ id: 'slow_exit', path: 'slow_lane', end: 'end', width_m: 3.0 }},
{block_portals}  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 650.0, y: -10.0 }},
      {{ x: 650.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 400.0, y: -1.5 }},
      {{ x: 400.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_b', points: [
      {{ x: 0.0, y: 1.5 }}, {{ x: 400.0, y: 1.5 }},
      {{ x: 400.0, y: 4.5 }}, {{ x: 0.0, y: 4.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider', 'companion', 'through_rider' ] }},
      lateral_use: 'shared', lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'b', region: 'band_b', reference_path: 'guide_b',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider', 'companion', 'through_rider' ] }},
      lateral_use: 'shared', lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_adjacencies: [
    {{ id: 'a_beside_b', first: 'a', second: 'b', side: 'left' }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
    {{ id: 'slow_through', from: 'slow_entry', to: 'slow_exit',
      path: 'slow_lane', priority: 0, direction: 'forward' }},
{block_movement}  ],
  mode_templates: [ {rider_mode}, {RETURN_COMPANION_MODE}, {THROUGH_RIDER_MODE} ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 300.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'slow_inflow', mode: 'companion',
      spawn: {{ rate: {{
        portal: 'slow_entry',
        rate_per_hour: 300.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'slow_through', weight: 1.0 }} ] }},
      }} }} }},
{block_demand}  ],
}}
"#
    )
    .replace("SLOW_Y", &format!("{:?}", source_y_m - 2.0))
    .replace("BLOCK_Y", &format!("{block_lane_y_m:?}"))
    .replace("BLOCK_START", &format!("{:?}", RETURN_OBSTRUCTION_ZONE_M.start()))
    .replace("BLOCK_END", &format!("{:?}", RETURN_OBSTRUCTION_ZONE_M.end()))
}

/// One agent's compiled guide path, arc-length progress, and world position.
struct Placement {
    id: AgentId,
    path: usize,
    s_m: f64,
    x_m: f64,
}

/// The placed agents with a compiled guide path, in spawn order.
fn placements(sim: &Simulation) -> Vec<Placement> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            Some(Placement {
                id: sample.id,
                path: motion.path.index(),
                s_m: motion.path_distance_m,
                x_m: sample.position.x,
            })
        })
        .collect()
}

/// The stretch of the fixture the oncoming stream occupies, so the test can name
/// where the rider's return is obstructed and where it is clear again.
const ONCOMING_ZONE_M: std::ops::RangeInclusive<f64> = 100.0..=160.0;

/// Step until a lateral-capable rider has a slower body 10..=13 m ahead on the
/// slower lane, so its committed leg predicts clear and the pass it completes
/// carries it far enough to displace before it returns.
///
/// Returns the rider and the slower body it passes.
fn rider_before_a_slow_body(sim: &mut Simulation) -> Option<(AgentId, AgentId)> {
    for _ in 0..900 {
        sim.step();
        let placed = placements(sim);
        let frame = sim.snapshot(SnapshotDetail::Full);
        for rider in placed.iter().filter(|agent| agent.path == 0) {
            if route_state(frame.agents(), rider.id)
                .and_then(|route| route.target_clearance_m)
                .is_none()
            {
                continue;
            }
            let slow = placed
                .iter()
                .filter(|agent| agent.path == 1)
                .find(|agent| (10.0..=13.0).contains(&(agent.s_m - rider.s_m)));
            if let Some(slow) = slow {
                return Some((rider.id, slow.id));
            }
        }
    }
    None
}

/// A returning rider whose return corridor an oncoming body occupies holds the
/// offset it occupies instead of settling into `following`: the documented
/// return-obstruction policy, driven through the public seam.
///
/// The rider passes a slower body, completes the maneuver, and returns to its
/// own offset — until its return corridor reaches the oncoming stream, whose
/// bodies keep the prediction's swept clearance below the target. It holds
/// through that stretch and settles only once it has left it. No step exceeds
/// the mode's motion limit and no step needs a kernel position cap.
#[test]
fn a_returning_riders_obstructed_target_holds() {
    let mut sim = build(&return_obstruction_scenario());
    let (rider, slow) =
        rider_before_a_slow_body(&mut sim).expect("a rider approaches a slower body");
    assert!(sim.request_lateral_maneuver(
        rider,
        LateralManeuverRequest {
            target_offset_m: RETURN_TARGET_M,
            passed_body: slow,
            target_facility: None,
        },
    ));

    let mut edges: Vec<(ManeuverState, ManeuverState, ManeuverEdge)> = Vec::new();
    let mut returning = false;
    let mut obstructed_steps = 0;
    let mut settled_in_the_zone = false;
    let mut followed = false;
    let mut max_step_m: f64 = 0.0;
    let mut previous: Option<DVec2> = None;
    for _ in 0..900 {
        let output = sim.step();
        edges.extend(
            output
                .transitions()
                .iter()
                .filter(|transition| transition.agent == rider)
                .map(|transition| (transition.from, transition.to, transition.edge)),
        );
        let frame = sim.snapshot(SnapshotDetail::Full);
        let placed = placements(&sim);
        let Some(sample) = frame.agents().iter().find(|sample| sample.id == rider) else {
            break;
        };
        if let Some(before) = previous {
            max_step_m = max_step_m.max((sample.position - before).length());
        }
        previous = Some(sample.position);
        let state = route_state(frame.agents(), rider).expect("the rider carries route state");
        if state.maneuver_state == ManeuverState::Returning && state.d_m.abs() > 0.1 {
            returning = true;
        }
        // The oncoming body the prediction can reach: one within the maneuver
        // horizon of the rider, on the lane inside its return corridor.
        let approaching = placed
            .iter()
            .filter(|agent| agent.path == 2)
            .map(|agent| agent.x_m - sample.position.x)
            .any(|gap| (0.0..=12.0).contains(&gap));
        if approaching {
            if state.maneuver_state != ManeuverState::Returning {
                settled_in_the_zone = true;
            } else {
                obstructed_steps += 1;
            }
        }
        if state.maneuver_state == ManeuverState::Following
            && ONCOMING_ZONE_M.contains(&sample.position.x)
        {
            settled_in_the_zone = true;
        }
        if state.maneuver_state == ManeuverState::Following && sample.position.x > 170.0 {
            followed = true;
        }
    }
    assert!(returning, "the rider completes the pass and returns");
    assert!(
        obstructed_steps >= 20,
        "an oncoming body occupies the return corridor for a stretch: {obstructed_steps} steps"
    );
    assert!(
        !settled_in_the_zone,
        "a return whose corridor is occupied never settles into following: {edges:?}"
    );
    assert!(
        followed,
        "the return settles once the rider has left the oncoming stretch"
    );
    assert!(
        max_step_m <= 6.0 * DT + 1e-6,
        "no rider teleported: max step {max_step_m} m"
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
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
    assert_eq!(handoff.side, hekate_sim::PassSide::Left);
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
            .is_some_and(|route| route.maneuver_state == hekate_sim::ManeuverState::Following)
            && !edges.is_empty()
        {
            break;
        }
    }
    use hekate_sim::{ManeuverEdge, ManeuverState};
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
        Some(hekate_sim::ManeuverReason::BoundaryForbidden),
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

/// The compiled-path index of the obstruction lane in the cross-facility pass
/// fixture: its paths are `guide_a`, `slow_lane`, `guide_b`, and the obstruction
/// lane is appended last when the shape asks for it.
const BLOCK_LANE_PATH: usize = 3;

/// The longitudinal reach of the obstruction a return reads: the compiled
/// crossing corridor sweeps the stretch of the destination band a few body
/// lengths ahead of the rider, so a body there is a body the corridor holds the
/// return for, and one outside it is not.
const RETURN_CORRIDOR_REACH_M: std::ops::RangeInclusive<f64> = 3.0..=13.0;

/// A requested cross-facility change of lane completes the pass it was made for
/// and returns over the compiled shared boundary: the outbound crossing carries
/// the rider into the adjacent band, and the return crossing carries it back to
/// the band it was attempted from, where it settles at the offset it held there.
///
/// Both crossings fire at the adjacency's compiled shared boundary rather than
/// at either band's half-width: the source reference runs half a metre above the
/// middle of its region and the destination reference half a metre above the
/// middle of its, so the boundary is 1.0 m from the source reference and 2.0 m
/// from the destination reference — neither is the 1.5 m half-width of the two
/// compiled bands.
#[test]
fn a_cross_facility_change_of_lane_returns_over_the_shared_boundary() {
    let mut sim = build(&crossing_pass_scenario(CrossingPass {
        source_reference_y_m: 0.5,
        destination_reference_y_m: 3.5,
        obstruct_return: false,
    }));
    let (rider, slow) =
        rider_before_a_slow_body(&mut sim).expect("a rider approaches a slower body");
    assert!(sim.request_lateral_maneuver(
        rider,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: slow,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));

    let trace = drive(&mut sim, 900);
    let crossings: Vec<FacilityTransitionRecord> = trace
        .transitions
        .iter()
        .filter(|record| record.via == TransitionKind::Lateral)
        .copied()
        .collect();
    assert_eq!(
        crossings.len(),
        2,
        "the change of lane crosses out and back exactly once: {crossings:?}"
    );

    let outbound = crossings[0];
    assert_eq!(outbound.from_facility, FacilityId::from_index(0));
    assert_eq!(outbound.to_facility, FacilityId::from_index(1));
    assert_eq!(outbound.side, hekate_sim::PassSide::Left);
    assert!(outbound.permitted, "the destination permits forward travel");
    assert!(
        (outbound.d_m - 1.0).abs() < 0.05,
        "the outbound crossing fires at the compiled shared boundary: {}",
        outbound.d_m
    );

    let returning = crossings[1];
    assert_eq!(returning.from_facility, FacilityId::from_index(1));
    assert_eq!(returning.to_facility, FacilityId::from_index(0));
    assert_eq!(
        returning.side,
        hekate_sim::PassSide::Right,
        "the return crosses the boundary from the other band's own frame"
    );
    assert!(
        returning.permitted,
        "the source band permits forward travel"
    );
    assert!(
        (returning.d_m + 2.0).abs() < 0.05,
        "the return crossing fires at the compiled shared boundary: {}",
        returning.d_m
    );
    assert!(
        returning.d_m < -1.5 - 0.1,
        "the destination band's half-width is not the crossing bound: {}",
        returning.d_m
    );
    // A settled rider is inside the settle tolerance of its target offset, so
    // its last bounded-steering step carries that residual alongside the step
    // its speed allows: the bound is one step at the mode's speed plus the
    // tolerance the settle edge itself accepts.
    assert!(
        trace.max_step_m <= 6.0 * DT + hekate_sim::SETTLE_TOLERANCE_M,
        "no rider teleported across either handoff: max step {} m",
        trace.max_step_m
    );

    // The rider carries the whole lifecycle through and settles at the offset it
    // held in the source band before the attempt: it is never despawned or
    // re-spawned, and the return leaves it on the band it started from rather
    // than in the band it passed through.
    let frame = sim.snapshot(SnapshotDetail::Full);
    let sample = frame
        .agents()
        .iter()
        .find(|sample| sample.id == rider)
        .expect("the rider is still in the world");
    let state = route_state(frame.agents(), rider).expect("the rider carries route state");
    assert_eq!(state.maneuver_state, ManeuverState::Following);
    assert!(
        state.d_m.abs() <= 0.05,
        "the return settles at the offset the rider held: {}",
        state.d_m
    );
    assert!(
        (sample.position.y - 0.5).abs() <= 0.05,
        "the rider is back on the source band's reference: {}",
        sample.position.y
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
}

/// A requested cross-facility change of lane whose return corridor a body
/// occupies holds the offset it occupies and re-decides, rather than steering
/// back into the body: the return-obstruction policy a same-facility maneuver
/// already follows, read through the compiled crossing corridor instead of the
/// destination band's own interval.
///
/// The obstruction stream rides the stretch of the destination band the
/// crossing corridor sweeps back through, so only a return reads it: the
/// outbound crossing is upstream of the stretch, the entry leg ascends away from
/// it at its far end, and the committed leg's own lane stays two metres clear of
/// it.
#[test]
fn a_cross_facility_returns_obstructed_corridor_holds_and_re_decides() {
    let mut sim = build(&crossing_pass_scenario(CrossingPass {
        obstruct_return: true,
        ..CrossingPass::default()
    }));
    let (rider, slow) =
        rider_before_a_slow_body(&mut sim).expect("a rider approaches a slower body");
    assert!(sim.request_lateral_maneuver(
        rider,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: slow,
            target_facility: Some(FacilityId::from_index(1)),
        },
    ));

    let mut crossed_out = false;
    let mut returned = false;
    let mut held_steps = 0;
    let mut held_offset_m = None;
    let mut max_step_m: f64 = 0.0;
    let mut previous: Option<DVec2> = None;
    for _ in 0..1200 {
        let output = sim.step();
        crossed_out |= output.facility_transitions().iter().any(|record| {
            record.agent == rider
                && record.via == TransitionKind::Lateral
                && record.from_facility == FacilityId::from_index(0)
        });
        returned |= output.facility_transitions().iter().any(|record| {
            record.agent == rider
                && record.via == TransitionKind::Lateral
                && record.from_facility == FacilityId::from_index(1)
        });
        let frame = sim.snapshot(SnapshotDetail::Full);
        let Some(sample) = frame.agents().iter().find(|sample| sample.id == rider) else {
            break;
        };
        if let Some(before) = previous {
            max_step_m = max_step_m.max((sample.position - before).length());
        }
        previous = Some(sample.position);
        if returned {
            break;
        }
        let Some(state) = route_state(frame.agents(), rider) else {
            break;
        };
        // The obstruction the return leg reads: a body on the stream's stretch
        // inside the crossing corridor's own longitudinal reach.
        let obstructing = placements(&sim).iter().any(|body| {
            body.path == BLOCK_LANE_PATH
                && RETURN_CORRIDOR_REACH_M.contains(&(body.x_m - sample.position.x))
        });
        if !crossed_out || state.maneuver_state != ManeuverState::Returning || !obstructing {
            continue;
        }
        held_steps += 1;
        let held_m = *held_offset_m.get_or_insert(state.d_m);
        assert!(
            (state.d_m - held_m).abs() <= 0.02,
            "the return holds the offset it occupies while the obstruction is in its corridor: {} not {held_m}",
            state.d_m
        );
    }
    assert!(crossed_out, "the rider crossed into the adjacent band");
    assert!(
        returned,
        "a return whose corridor clears crosses back over the shared boundary"
    );
    assert!(
        held_steps >= 20,
        "the obstructed return holds until its corridor clears: {held_steps} steps"
    );
    assert!(
        max_step_m <= 6.0 * DT + hekate_sim::SETTLE_TOLERANCE_M,
        "no rider teleported across either handoff: max step {max_step_m} m"
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
}

// ---------------------------------------------------------------------------
// Current and destination leader/follower constraints (TAS-114)
// ---------------------------------------------------------------------------

/// The compiled-path index of the constraint fixture's paths: the source band's
/// reference, the parallel lane the rider passes, and the destination band's
/// reference.
const CONSTRAINT_SOURCE_PATH: usize = 0;
const CONSTRAINT_PASSED_PATH: usize = 1;
const CONSTRAINT_DESTINATION_PATH: usize = 2;

/// The body length every constraint fixture mode carries, so a centre-to-centre
/// separation of this many metres is a touching pair.
const CONSTRAINT_BODY_LENGTH_M: f64 = 1.8;

/// The fastest free-flow speed any constraint fixture stream carries: the
/// `fast` companion, at twice the rider's own speed.
const CONSTRAINT_MAX_SPEED_M: f64 = 12.0;

/// The band a constraint fixture's companion rides.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConstraintBand {
    /// The band the rider starts on, on the rider's own reference path.
    Source,
    /// The band across the shared boundary.
    Destination,
}

/// The companion a constraint fixture places on a band: a body that declares no
/// lateral maneuver, so it is never a maneuver candidate and only ever enters a
/// rider's leader selection or its committed prediction as an ordinary body.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Companion {
    /// A body slower than the rider: a leader ahead the rider closes on.
    Slow,
    /// A body at the rider's own free-flow speed.
    Steady,
    /// A body faster than the rider: one that can close in from behind.
    Fast,
}

impl Companion {
    /// The mode template id this companion is spawned from.
    fn mode_id(self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Steady => "steady",
            Self::Fast => "fast",
        }
    }

    /// The companion's free-flow speed in metres per second.
    fn speed_mps(self) -> f64 {
        match self {
            Self::Slow => 1.0,
            Self::Steady => 6.0,
            Self::Fast => 12.0,
        }
    }

    /// The companion mode template source at that speed.
    fn mode_source(self) -> String {
        companion_mode(self.mode_id(), self.speed_mps())
    }
}

/// A companion mode: the same capsule as the rider with no `lateral` object, so
/// it never maneuvers and never steers, and a shorter time gap so it keeps
/// station rather than opening a large following distance.
fn companion_mode(id: &str, speed_mps: f64) -> String {
    format!(
        r#"
    {{
      id: '{id}',
      body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }},
        radius_m: {{ min: 0.35, max: 0.35 }} }},
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: {speed_mps:?}, max: {speed_mps:?} }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
    }}"#
    )
}

/// The authored shape of the leader/follower constraint fixture: two
/// side-by-side bands joined by an adjacency, a lateral-capable rider stream on
/// the source band, a slower body stream on a parallel lane the rider passes,
/// and one companion stream on a chosen band.
struct CrossingConstraint {
    /// The band the companion stream rides, or `None` for no companion at all.
    companion_band: Option<ConstraintBand>,
    /// Which companion the stream places.
    companion: Companion,
    /// The companion stream's arrival rate in vehicles per hour.
    companion_rate_per_hour: f64,
    /// The rider mode's compiled prediction horizon in seconds.
    rider_horizon_s: f64,
    /// The rider stream's arrival rate in vehicles per hour: a dense rate queues
    /// riders on one band, so a rider has a same-band follower right behind it.
    rider_rate_per_hour: f64,
}

impl Default for CrossingConstraint {
    fn default() -> Self {
        Self {
            companion_band: None,
            companion: Companion::Slow,
            companion_rate_per_hour: 120.0,
            rider_horizon_s: 6.0,
            rider_rate_per_hour: 300.0,
        }
    }
}

/// The two side-by-side bands with the companion stream the shape asks for.
///
/// The source band is `y = 0..3` around its reference at `y = 0`, the
/// destination band `y = 3..6` around its reference at `y = 3`, and the band the
/// rider passes runs outside both at `y = -2`. Every companion rides a band's
/// own reference path, so it carries route state on that facility exactly as a
/// rider does.
fn crossing_constraint_scenario(shape: CrossingConstraint) -> String {
    let CrossingConstraint {
        companion_band,
        companion,
        companion_rate_per_hour,
        rider_horizon_s,
        rider_rate_per_hour,
    } = shape;
    let rider_mode = lane_rider_mode(rider_horizon_s);
    let modes = [Companion::Slow, Companion::Steady, Companion::Fast]
        .into_iter()
        .map(|kind| kind.mode_source())
        .collect::<Vec<_>>()
        .join(", ");
    let companion_stream = companion_band.map(|band| match band {
        ConstraintBand::Source => ("entry", "through"),
        ConstraintBand::Destination => ("b_entry", "b_through"),
    });
    let companion_demand = match companion_stream {
        None => String::new(),
        Some((portal, movement)) => {
            let mode = companion.mode_id();
            format!(
                r#"    {{ id: 'companion_inflow', mode: '{mode}',
      spawn: {{ rate: {{
        portal: '{portal}',
        rate_per_hour: {companion_rate_per_hour:?},
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: '{movement}', weight: 1.0 }} ] }},
      }} }} }},
"#
            )
        }
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'crossing_constraint_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 400.0, y: 0.0 }} ] }},
    {{ id: 'pass_lane', points: [ {{ x: 40.0, y: -2.0 }}, {{ x: 640.0, y: -2.0 }} ] }},
    {{ id: 'guide_b', points: [ {{ x: 0.0, y: 3.0 }}, {{ x: 400.0, y: 3.0 }} ] }},
  ],
  portals: [
    {{ id: 'entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
    {{ id: 'pass_entry', path: 'pass_lane', end: 'start', width_m: 3.0 }},
    {{ id: 'pass_exit', path: 'pass_lane', end: 'end', width_m: 3.0 }},
    {{ id: 'b_entry', path: 'guide_b', end: 'start', width_m: 3.0 }},
    {{ id: 'b_exit', path: 'guide_b', end: 'end', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 650.0, y: -10.0 }},
      {{ x: 650.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 400.0, y: -1.5 }},
      {{ x: 400.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_b', points: [
      {{ x: 0.0, y: 1.5 }}, {{ x: 400.0, y: 1.5 }},
      {{ x: 400.0, y: 4.5 }}, {{ x: 0.0, y: 4.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider', 'slow', 'steady', 'fast' ] }},
      lateral_use: 'shared', lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'b', region: 'band_b', reference_path: 'guide_b',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider', 'slow', 'steady', 'fast' ] }},
      lateral_use: 'shared', lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_adjacencies: [
    {{ id: 'a_beside_b', first: 'a', second: 'b', side: 'left' }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
    {{ id: 'pass_through', from: 'pass_entry', to: 'pass_exit',
      path: 'pass_lane', priority: 0, direction: 'forward' }},
    {{ id: 'b_through', from: 'b_entry', to: 'b_exit', path: 'guide_b', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [ {rider_mode}, {modes} ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: {rider_rate_per_hour:?},
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'pass_inflow', mode: 'slow',
      spawn: {{ rate: {{
        portal: 'pass_entry',
        rate_per_hour: 300.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'pass_through', weight: 1.0 }} ] }},
      }} }} }},
{companion_demand}  ],
}}
"#
    )
}

/// The configuration a constraint test requests its crossing from: the rider,
/// the slower body it passes, and the companion the fixture placed.
#[derive(Clone, Copy)]
struct ConstraintBodies {
    rider: AgentId,
    passed: AgentId,
    companion: Option<AgentId>,
}

/// Drive the fixture until the rider, the body it passes, and the companion are
/// all in place: a lateral-capable rider with a slower body within
/// `PASSED_WINDOW_M` ahead along the world axis and, when a companion path is
/// named, one within `companion_window` of the rider's own world `x` — positive
/// ahead, negative behind.
fn rider_before_constraints(
    sim: &mut Simulation,
    companion_path: Option<usize>,
    companion_window: &std::ops::RangeInclusive<f64>,
) -> ConstraintBodies {
    for _ in 0..6000 {
        sim.step();
        let placed = placements(sim);
        let frame = sim.snapshot(SnapshotDetail::Full);
        for rider in placed
            .iter()
            .filter(|agent| agent.path == CONSTRAINT_SOURCE_PATH)
        {
            if route_state(frame.agents(), rider.id)
                .and_then(|route| route.target_clearance_m)
                .is_none()
            {
                continue;
            }
            let Some(passed) = placed
                .iter()
                .filter(|agent| agent.path == CONSTRAINT_PASSED_PATH)
                .find(|agent| PASSED_WINDOW_M.contains(&(agent.x_m - rider.x_m)))
            else {
                continue;
            };
            let companion = match companion_path {
                None => None,
                Some(path) => placed
                    .iter()
                    .filter(|agent| agent.path == path)
                    .find(|agent| companion_window.contains(&(agent.x_m - rider.x_m)))
                    .map(|agent| agent.id),
            };
            if companion_path.is_some() && companion.is_none() {
                continue;
            }
            return ConstraintBodies {
                rider: rider.id,
                passed: passed.id,
                companion,
            };
        }
    }
    panic!("the fixture never placed the requested rider and companion configuration");
}

/// The slower body the rider passes sits this far ahead of the rider in metres
/// when a constraint test requests its crossing.
const PASSED_WINDOW_M: std::ops::RangeInclusive<f64> = 10.0..=13.0;

/// Drive the fixture until a lateral-capable rider has a same-band body within
/// `leader_m` ahead of it and one within `behind_m` behind it, returning the
/// rider, the body ahead it passes, and the body behind it.
fn rider_between_same_band_bodies(
    sim: &mut Simulation,
    leader_m: &std::ops::RangeInclusive<f64>,
    behind_m: &std::ops::RangeInclusive<f64>,
) -> ConstraintBodies {
    for _ in 0..6000 {
        sim.step();
        let placed = placements(sim);
        let frame = sim.snapshot(SnapshotDetail::Full);
        for rider in placed
            .iter()
            .filter(|agent| agent.path == CONSTRAINT_SOURCE_PATH)
        {
            if route_state(frame.agents(), rider.id)
                .and_then(|route| route.target_clearance_m)
                .is_none()
            {
                continue;
            }
            let on_band =
                |agent: &&Placement| agent.id != rider.id && agent.path == CONSTRAINT_SOURCE_PATH;
            let Some(ahead) = placed
                .iter()
                .filter(on_band)
                .find(|agent| leader_m.contains(&(agent.x_m - rider.x_m)))
            else {
                continue;
            };
            let Some(behind) = placed
                .iter()
                .filter(on_band)
                .find(|agent| behind_m.contains(&(agent.x_m - rider.x_m)))
            else {
                continue;
            };
            return ConstraintBodies {
                rider: rider.id,
                passed: ahead.id,
                companion: Some(behind.id),
            };
        }
    }
    panic!("the fixture never placed a rider between same-band bodies");
}

/// Request the fixture's cross-facility change of lane for a rider, naming the
/// body it passes and the destination band whose own centreline is the target.
fn request_crossing(sim: &mut Simulation, bodies: ConstraintBodies) -> bool {
    sim.request_lateral_maneuver(
        bodies.rider,
        LateralManeuverRequest {
            target_offset_m: 0.0,
            passed_body: bodies.passed,
            target_facility: Some(FacilityId::from_index(1)),
        },
    )
}

/// What one rider's crossing approach did over a run: the lowest speed it held
/// while it still rode the source band with a committed maneuver in flight,
/// whether a lateral handoff fired, every abort reason it recorded, and the
/// largest single-body world displacement seen.
#[derive(Default)]
struct CrossingApproach {
    /// The lowest source-band committed speed, or `None` when the rider never
    /// held a committed maneuver on the source band.
    min_committed_speed_mps: Option<f64>,
    /// Whether the rider crossed the shared boundary laterally.
    crossed: bool,
    /// Every `aborted` edge the rider took, with its reason.
    aborts: Vec<Option<ManeuverAbortReason>>,
    /// The largest per-step world displacement of any live body.
    max_step_m: f64,
}

/// Step `sim` for `ticks`, recording the rider's committed approach.
fn drive_crossing_approach(
    sim: &mut Simulation,
    bodies: ConstraintBodies,
    ticks: u64,
) -> CrossingApproach {
    let mut approach = CrossingApproach::default();
    let mut previous: BTreeMap<AgentId, DVec2> = BTreeMap::new();
    for _ in 0..ticks {
        let output = sim.step();
        approach.crossed |= output
            .facility_transitions()
            .iter()
            .any(|record| record.agent == bodies.rider && record.via == TransitionKind::Lateral);
        approach.aborts.extend(
            output
                .transitions()
                .iter()
                .filter(|transition| {
                    transition.agent == bodies.rider && transition.to == ManeuverState::Aborted
                })
                .map(|transition| transition.reason),
        );
        let frame = sim.snapshot(SnapshotDetail::Full);
        for sample in frame.agents() {
            if let Some(before) = previous.get(&sample.id) {
                approach.max_step_m = approach
                    .max_step_m
                    .max((sample.position - *before).length());
            }
            previous.insert(sample.id, sample.position);
        }
        let Some(sample) = frame
            .agents()
            .iter()
            .find(|sample| sample.id == bodies.rider)
        else {
            break;
        };
        let rides_source = sample
            .motion
            .as_ref()
            .is_some_and(|motion| motion.path.index() == CONSTRAINT_SOURCE_PATH);
        let committed = route_state(frame.agents(), bodies.rider)
            .is_some_and(|state| state.maneuver_state == ManeuverState::Committed);
        if rides_source
            && committed
            && let Some(motion) = sample.motion.as_ref()
        {
            approach.min_committed_speed_mps = Some(
                approach
                    .min_committed_speed_mps
                    .map_or(motion.speed_mps, |speed: f64| speed.min(motion.speed_mps)),
            );
        }
    }
    approach
}

/// A leader on the rider's own band constrains the committed change of lane: the
/// rider keeps following the slower body ahead of it on the band it rides for
/// the whole approach, so the crossing never becomes a step the leader does not
/// bound.
///
/// The rider is the follower half of the pair here, and the pair rides the band
/// the rider still owns: the leader read for the band it rides is the one
/// Increment 1 already selected, and this pins that a cross-facility change of
/// lane never drops it.
#[test]
fn a_body_ahead_on_the_riders_own_band_slows_the_committed_change_of_lane() {
    let mut sim = build(&crossing_constraint_scenario(CrossingConstraint {
        companion_band: Some(ConstraintBand::Source),
        companion: Companion::Slow,
        ..CrossingConstraint::default()
    }));
    let bodies = rider_before_constraints(&mut sim, Some(CONSTRAINT_SOURCE_PATH), &(15.0..=20.0));
    assert!(request_crossing(&mut sim, bodies));

    let approach = drive_crossing_approach(&mut sim, bodies, 600);
    let slowest = approach
        .min_committed_speed_mps
        .expect("the rider holds a committed maneuver on the source band");
    assert!(
        slowest < 5.0,
        "the body ahead on the rider's own band bounds the committed approach: {slowest} m/s"
    );
    assert!(
        approach.crossed,
        "the rider still crosses: a leader on its own band slows the approach, it does not cancel it"
    );
    assert!(
        approach.max_step_m <= CONSTRAINT_MAX_SPEED_M * DT + hekate_sim::SETTLE_TOLERANCE_M,
        "no body moved further than the fixture's fastest stream allows: {} m",
        approach.max_step_m
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
}

/// A leader on the destination band constrains the committed change of lane
/// *before* the handoff: the rider slows for a body ahead of it on the band it
/// has not reached yet, so the destination leader is active across the whole
/// approach rather than only once ownership has moved.
///
/// The same fixture without the destination body is the control: nothing else is
/// within the rider's reach there, so its committed speed stays at free flow.
#[test]
fn a_body_ahead_on_the_destination_band_slows_the_committed_change_of_lane() {
    let mut sim = build(&crossing_constraint_scenario(CrossingConstraint {
        companion_band: Some(ConstraintBand::Destination),
        companion: Companion::Slow,
        ..CrossingConstraint::default()
    }));
    let bodies =
        rider_before_constraints(&mut sim, Some(CONSTRAINT_DESTINATION_PATH), &(15.0..=20.0));
    assert!(request_crossing(&mut sim, bodies));
    let approach = drive_crossing_approach(&mut sim, bodies, 600);
    let slowest = approach
        .min_committed_speed_mps
        .expect("the rider holds a committed maneuver on the source band");
    assert!(
        slowest < 5.0,
        "the body ahead on the destination band bounds the committed approach: {slowest} m/s"
    );

    // The control: the identical fixture and request with no destination body.
    let mut control = build(&crossing_constraint_scenario(CrossingConstraint::default()));
    let control_bodies = rider_before_constraints(&mut control, None, &(0.0..=0.0));
    assert!(request_crossing(&mut control, control_bodies));
    let control_approach = drive_crossing_approach(&mut control, control_bodies, 600);
    let control_slowest = control_approach
        .min_committed_speed_mps
        .expect("the control rider holds a committed maneuver on the source band");
    assert!(
        control_slowest >= 5.9,
        "nothing else bounds the control rider's committed approach: {control_slowest} m/s"
    );
    assert!(
        control_approach.crossed,
        "the control rider with no destination body crosses"
    );
    assert!(
        approach.max_step_m <= CONSTRAINT_MAX_SPEED_M * DT + hekate_sim::SETTLE_TOLERANCE_M,
        "no body moved further than the fixture's fastest stream allows: {} m",
        approach.max_step_m
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
    assert_eq!(control.emergency_cap_steps(), 0);
}

/// A body closing in from behind on the destination band holds the committed
/// change of lane: the rider never crosses into the band in front of it.
///
/// The body rides the far side of the shared boundary at twice the rider's
/// speed, so it closes on the rider's own progress and then sweeps past it
/// alongside the crossing. The committed crossing's swept clearance against it —
/// the rear clearance of the body closing from behind and the side clearance
/// while it is alongside — never reaches the mode's target, so the ordered
/// response aborts the maneuver and the handoff never fires. The control is the
/// same fixture and request with no such body, which crosses.
#[test]
fn a_body_closing_from_behind_on_the_destination_band_holds_the_committed_change_of_lane() {
    let mut sim = build(&crossing_constraint_scenario(CrossingConstraint {
        companion_band: Some(ConstraintBand::Destination),
        companion: Companion::Fast,
        companion_rate_per_hour: 900.0,
        ..CrossingConstraint::default()
    }));
    let bodies =
        rider_before_constraints(&mut sim, Some(CONSTRAINT_DESTINATION_PATH), &(-8.0..=-2.0));
    assert!(request_crossing(&mut sim, bodies));
    let approach = drive_crossing_approach(&mut sim, bodies, 600);
    assert!(
        !approach.crossed,
        "a body closing from behind on the destination band holds the crossing back: {:?}",
        approach.aborts
    );
    assert_eq!(
        approach.aborts,
        vec![Some(ManeuverAbortReason::ClearanceLost)],
        "the outbound leg aborts on its predicted clearance"
    );

    let mut control = build(&crossing_constraint_scenario(CrossingConstraint::default()));
    let control_bodies = rider_before_constraints(&mut control, None, &(0.0..=0.0));
    assert!(request_crossing(&mut control, control_bodies));
    let control_approach = drive_crossing_approach(&mut control, control_bodies, 600);
    assert!(
        control_approach.crossed,
        "with no body behind it on the destination band the rider crosses"
    );
    assert!(
        approach.max_step_m <= CONSTRAINT_MAX_SPEED_M * DT + hekate_sim::SETTLE_TOLERANCE_M,
        "no body moved further than the fixture's fastest stream allows: {} m",
        approach.max_step_m
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
    assert_eq!(control.emergency_cap_steps(), 0);
}

/// A body behind the rider on the rider's own band stays the rider's follower
/// through the crossing, and the crossing is not held for it.
///
/// The two bodies ride the same band's reference in one queue, so the body
/// behind selects the rider as its leader for as long as they share the band:
/// it never passes the rider and never overlaps it, and its constraint is the
/// ordinary same-path leader read. The portal admits such a follower at the safe
/// following speed for its gap, so it cannot close inside the crossing's target
/// clearance: the committed crossing stays feasible and the handoff fires. The
/// pair is the own-band half of the leader/follower pair the crossing must keep
/// active, and the crossing neither drops the follower nor invents a hold for
/// it.
#[test]
fn a_body_behind_on_the_riders_own_band_stays_its_follower_through_the_crossing() {
    let mut sim = build(&crossing_constraint_scenario(CrossingConstraint {
        rider_rate_per_hour: 3600.0,
        ..CrossingConstraint::default()
    }));
    let bodies = rider_between_same_band_bodies(&mut sim, &(5.0..=20.0), &(-3.2..=-2.2));
    let follower = bodies
        .companion
        .expect("the fixture placed the body behind");
    assert!(request_crossing(&mut sim, bodies));

    let mut crossed = false;
    let mut closest_m = f64::INFINITY;
    let mut previous: Option<DVec2> = None;
    let mut max_step_m: f64 = 0.0;
    for _ in 0..600 {
        let output = sim.step();
        crossed |= output
            .facility_transitions()
            .iter()
            .any(|record| record.agent == bodies.rider && record.via == TransitionKind::Lateral);
        let frame = sim.snapshot(SnapshotDetail::Full);
        let Some(rider) = frame
            .agents()
            .iter()
            .find(|sample| sample.id == bodies.rider)
        else {
            break;
        };
        if let Some(before) = previous {
            max_step_m = max_step_m.max((rider.position - before).length());
        }
        previous = Some(rider.position);
        // While both bodies still ride the rider's own band, the follower's own
        // leader selection keeps it behind the rider at the separation the
        // portal admitted it with.
        let Some(follower_sample) = frame.agents().iter().find(|sample| sample.id == follower)
        else {
            continue;
        };
        let same_band = follower_sample
            .motion
            .as_ref()
            .is_some_and(|motion| motion.path.index() == CONSTRAINT_SOURCE_PATH)
            && rider
                .motion
                .as_ref()
                .is_some_and(|motion| motion.path.index() == CONSTRAINT_SOURCE_PATH);
        if !same_band {
            continue;
        }
        let separation_m = rider.position.x - follower_sample.position.x;
        closest_m = closest_m.min(separation_m);
        assert!(
            separation_m > CONSTRAINT_BODY_LENGTH_M,
            "the body behind the rider never passes it or overlaps it while they share the band: {separation_m} m"
        );
    }
    assert!(
        crossed,
        "a well-formed follower behind on the rider's own band does not hold the crossing"
    );
    assert!(
        closest_m < f64::INFINITY,
        "the fixture observed the pair on the rider's own band"
    );
    assert!(
        max_step_m <= CONSTRAINT_MAX_SPEED_M * DT + hekate_sim::SETTLE_TOLERANCE_M,
        "no body moved further than the fixture's fastest stream allows: {max_step_m} m"
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
}
