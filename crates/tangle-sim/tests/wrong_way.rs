//! TAS-117: the wrong-way entry transitions route state onto the connected
//! opposing traversal.
//!
//! A contextual wrong-way decision selects the opposing option, and the kernel's
//! wrong-way pass moves that agent's route state onto the connected opposing
//! traversal through the same one-step ownership move the facility handoffs
//! perform: the travel direction and the route coordinates change, the world
//! pose is the integrated truth and is never moved, and the completion then runs
//! through the ordinary connector handoff. The focused fixtures therefore drive
//! the ordinary seam — demand places riders on a compiled facility, and the
//! kernel's own wrong-way and physical-advance passes produce the transition —
//! and nothing here scripts a trajectory or disables a query.
//!
//! Nothing in a `capability`-less or `wrong_way`-less scenario reaches the
//! decision, so the fixtures author both, and the rider's own compliance and the
//! scenario urgency are fixed so every draw selects the opposing option.

use std::collections::BTreeMap;

use glam::DVec2;
use tangle_model::{CompiledScenario, MovementDirection, parse_scenario_source_v2};
use tangle_sim::{
    FacilityTransitionRecord, ManeuverState, RouteStateSample, RunConfig, Simulation,
    SnapshotDetail, TransitionKind,
};

/// The fixed step the fixtures run at, which is [`RunConfig`]'s own default.
const DT: f64 = 0.05;

/// The rider mode shared by both fixtures: a capsule that steers longitudinally
/// and declares the `reverse_direction` tactic, with a fully non-compliant,
/// maximum-urgency profile, so every draw selects the opposing option.
const RIDER_MODE: &str = r#"
    {
      id: 'rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'reverse_direction' ],
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
        compliance: { min: 0.0, max: 0.0 },
      },
    }"#;

/// The wrong-way policy the fixtures author: every non-random precondition is
/// satisfied for a rider early on its facility, and the full urgency with zero
/// compliance makes every draw select the opposing option.
const WRONG_WAY_POLICY: &str = r#"
  maneuver_policy: {
    wrong_way: { min_time_saving_s: 0.0, max_opposing_density_per_km: 100.0,
      urgency: 1.0 },
  },"#;

/// A forward-nominal facility `a` beside a reverse continuance `left`, joined by
/// two connectors.
///
/// `left_into_a` makes `a`'s forward traversal physically possible and
/// `a_back_to_left` makes its reverse traversal possible, so the compiled
/// topology connects the opposing traversal of `a`. Riders enter at `a`'s start
/// travelling forward; a wrong-way entry turns them around, and the ordinary
/// connector handoff then carries them onto `left`'s reverse traversal, where
/// they complete the route.
fn forward_rule_scenario(rate_per_hour: f64) -> String {
    format!(
        r#"{{
  schema_version: 2,
  id: 'wrong_way_forward_rule_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_a', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 100.0, y: 0.0 }} ] }},
    {{ id: 'guide_left', points: [ {{ x: -100.0, y: 0.0 }}, {{ x: 0.0, y: 0.0 }} ] }},
  ],
  portals: [
    {{ id: 'a_entry', path: 'guide_a', end: 'start', width_m: 3.0 }},
    {{ id: 'a_exit', path: 'guide_a', end: 'end', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -110.0, y: -10.0 }}, {{ x: 110.0, y: -10.0 }},
      {{ x: 110.0, y: 10.0 }}, {{ x: -110.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_a', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 100.0, y: -1.5 }},
      {{ x: 100.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_left', points: [
      {{ x: -100.0, y: -1.5 }}, {{ x: 0.0, y: -1.5 }},
      {{ x: 0.0, y: 1.5 }}, {{ x: -100.0, y: 1.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'a', region: 'band_a', reference_path: 'guide_a',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'left', region: 'band_left', reference_path: 'guide_left',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_connectors: [
    {{ id: 'left_into_a',
      from: {{ facility: 'left', direction: 'forward' }},
      to: {{ facility: 'a', direction: 'forward' }} }},
    {{ id: 'a_back_to_left',
      from: {{ facility: 'a', direction: 'reverse' }},
      to: {{ facility: 'left', direction: 'reverse' }} }},
  ],
  movements: [
    {{ id: 'through', from: 'a_entry', to: 'a_exit', path: 'guide_a', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [ {RIDER_MODE} ],{WRONG_WAY_POLICY}
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'a_entry',
        rate_per_hour: {rate_per_hour:?},
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

/// A reverse-nominal facility `d` beside a forward continuance `right`, joined
/// by two connectors.
///
/// `d_into_right` makes `d`'s forward traversal physically possible and
/// `right_into_d` makes its reverse traversal possible, so the compiled topology
/// connects the opposing traversal of `d`. Riders enter at `d`'s end travelling
/// reverse; a wrong-way entry turns them around, and the ordinary connector
/// handoff then carries them onto `right`'s forward traversal, where they
/// complete the route. This is the mirror image of the forward-rule fixture: the
/// authored reference direction is the reverse one and the wrong-way travel is
/// forward.
fn reverse_rule_scenario(rate_per_hour: f64) -> String {
    format!(
        r#"{{
  schema_version: 2,
  id: 'wrong_way_reverse_rule_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'guide_d', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 100.0, y: 0.0 }} ] }},
    {{ id: 'guide_right', points: [ {{ x: 100.0, y: 0.0 }}, {{ x: 200.0, y: 0.0 }} ] }},
  ],
  portals: [
    {{ id: 'd_entry', path: 'guide_d', end: 'end', width_m: 3.0 }},
    {{ id: 'd_exit', path: 'guide_d', end: 'start', width_m: 3.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -10.0, y: -10.0 }}, {{ x: 210.0, y: -10.0 }},
      {{ x: 210.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_d', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 100.0, y: -1.5 }},
      {{ x: 100.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_right', points: [
      {{ x: 100.0, y: -1.5 }}, {{ x: 200.0, y: -1.5 }},
      {{ x: 200.0, y: 1.5 }}, {{ x: 100.0, y: 1.5 }},
    ] }},
  ],
  facilities: [
    {{ id: 'd', region: 'band_d', reference_path: 'guide_d',
      width_m: 3.0, nominal_direction: 'reverse',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
    {{ id: 'right', region: 'band_right', reference_path: 'guide_right',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_connectors: [
    {{ id: 'd_into_right',
      from: {{ facility: 'd', direction: 'forward' }},
      to: {{ facility: 'right', direction: 'forward' }} }},
    {{ id: 'right_into_d',
      from: {{ facility: 'right', direction: 'reverse' }},
      to: {{ facility: 'd', direction: 'reverse' }} }},
  ],
  movements: [
    {{ id: 'back', from: 'd_entry', to: 'd_exit', path: 'guide_d', priority: 0,
      direction: 'reverse' }},
  ],
  mode_templates: [ {RIDER_MODE} ],{WRONG_WAY_POLICY}
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: 'd_entry',
        rate_per_hour: {rate_per_hour:?},
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'back', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#
    )
}

/// Build a simulation from a version-2 document.
fn build(scenario: &str) -> Simulation {
    let source = parse_scenario_source_v2(scenario).expect("the document is version 2");
    let compiled = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    Simulation::new(compiled, RunConfig::new(0)).expect("the simulation builds")
}

/// One rider's compiled guide path, arc-length position, world position, and
/// route state.
struct Placement {
    path: usize,
    s_m: f64,
    position: DVec2,
    route: Option<RouteStateSample>,
}

/// The placed agents with a compiled guide path, keyed by stable id.
fn placements(sim: &Simulation) -> BTreeMap<tangle_sim::AgentId, Placement> {
    sim.snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            Some((
                sample.id,
                Placement {
                    path: motion.path.index(),
                    s_m: motion.path_distance_m,
                    position: sample.position,
                    route: motion.route_state,
                },
            ))
        })
        .collect()
}

/// Step until exactly one rider is alive with arc length in `window` on
/// `path`, returning it and its placement.
///
/// The fixtures run one rider at a time: the demand rate is low enough that no
/// second arrival reaches the facility during the window, so the wrong-way rider
/// is never blocked by oncoming traffic on the stretch it turns back across.
fn lone_rider_on(
    sim: &mut Simulation,
    path: usize,
    window: std::ops::RangeInclusive<f64>,
) -> (tangle_sim::AgentId, Placement) {
    for _ in 0..4000 {
        sim.step();
        let placed = placements(sim);
        let mut matches = placed
            .iter()
            .filter(|(_, placement)| placement.path == path && window.contains(&placement.s_m));
        if let (Some((&id, placement)), None) = (matches.next(), matches.next()) {
            let position = placement.position;
            let s_m = placement.s_m;
            let route = placement.route;
            return (
                id,
                Placement {
                    path,
                    s_m,
                    position,
                    route,
                },
            );
        }
    }
    panic!("a lone rider must reach the window on path {path}");
}

/// A clear opposing entry turns a rider around on its own facility and leaves
/// the world pose continuous: the arc length strictly decreases, the facility
/// and the guide path are unchanged until the ordinary connector handoff, no
/// lateral maneuver runs, and no step exceeds the mode's motion limit.
#[test]
fn a_clear_opposing_entry_turns_the_rider_around() {
    let mut sim = build(&forward_rule_scenario(72.0));
    let (rider, before) = lone_rider_on(&mut sim, 0, 20.0..=30.0);
    assert!(before.route.is_some(), "the rider carries route state");
    assert!(
        sim.request_wrong_way_entry(rider),
        "a reverse-capable rider on a two-way facility can request the entry"
    );

    // The pass runs at the start of the next step, so the very next step is
    // already travelled in the opposing direction: the arc length strictly
    // decreases on every step the rider still rides its own facility.
    let mut previous = before.position;
    let mut s_m = before.s_m;
    let mut steps = 0;
    loop {
        sim.step();
        let placed = placements(&sim);
        let Some(after) = placed.get(&rider) else {
            panic!("the rider stays alive through the entry");
        };
        if after.path != before.path {
            break;
        }
        assert!(
            after.s_m < s_m,
            "the rider travels the opposing direction: {} -> {}",
            s_m,
            after.s_m
        );
        assert_eq!(
            after.route.map(|route| route.maneuver_state),
            Some(ManeuverState::Following),
            "the entry performs no lateral maneuver"
        );
        assert!(
            (after.position - previous).length() <= 6.0 * DT + 1e-6,
            "no rider teleported through the entry"
        );
        previous = after.position;
        s_m = after.s_m;
        steps += 1;
        if steps >= 600 {
            break;
        }
    }
    assert!(
        steps >= 20,
        "the rider travels the opposing direction across the facility: {steps} steps"
    );
    assert_eq!(sim.emergency_cap_steps(), 0);
}

/// The connector that makes `a`'s opposing traversal physically possible; the
/// disconnected negative case removes exactly this entry.
const A_BACK_TO_LEFT_ENTRY: &str = "    { id: 'a_back_to_left',\n      from: { facility: 'a', direction: 'reverse' },\n      to: { facility: 'left', direction: 'reverse' } },\n";

/// The entry seam refuses a rider that can never reach the decision: with no
/// `reverse_direction` capability, or with no connected opposing traversal, the
/// request records nothing and the rider keeps its nominal direction.
#[test]
fn the_entry_is_refused_without_the_capability_or_an_opposing_traversal() {
    // The policy is authored, but the mode carries no `reverse_direction`.
    let no_capability = forward_rule_scenario(72.0).replace(", 'reverse_direction'", "");
    let mut sim = build(&no_capability);
    let (rider, _) = lone_rider_on(&mut sim, 0, 20.0..=30.0);
    assert!(
        !sim.request_wrong_way_entry(rider),
        "no `reverse_direction` capability means no wrong-way entry"
    );
    let before = placements(&sim).get(&rider).map(|placement| placement.s_m);
    sim.step();
    let after = placements(&sim).get(&rider).map(|placement| placement.s_m);
    assert!(
        matches!((before, after), (Some(before), Some(after)) if after > before),
        "a refused request leaves the rider travelling its nominal direction"
    );

    // The capability and policy are authored, but the compiled topology connects
    // no opposing traversal: the reverse connector is removed, so `a`'s reverse
    // direction is not physically possible.
    let disconnected = forward_rule_scenario(72.0).replace(A_BACK_TO_LEFT_ENTRY, "");
    let mut sim = build(&disconnected);
    let (rider, _) = lone_rider_on(&mut sim, 0, 20.0..=30.0);
    assert!(
        !sim.request_wrong_way_entry(rider),
        "a facility with no connected opposing traversal has no wrong-way entry"
    );
}

/// The wrong-way rider completes its route in the reverse reference direction:
/// the ordinary connector handoff carries it onto the connected opposing
/// traversal of the neighbouring facility, and it then leaves the world at that
/// facility's far end.
#[test]
fn a_wrong_way_rider_completes_the_route_in_the_reverse_direction() {
    let mut sim = build(&forward_rule_scenario(72.0));
    let (rider, before) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(sim.request_wrong_way_entry(rider));

    let mut handoffs: Vec<FacilityTransitionRecord> = Vec::new();
    let mut despawned_reason = None;
    for _ in 0..600 {
        let output = sim.step();
        handoffs.extend(
            output
                .facility_transitions()
                .iter()
                .filter(|record| record.agent == rider)
                .copied(),
        );
        for event in output.events() {
            if let tangle_sim::Event::Despawned { agent, reason, .. } = event
                && *agent == rider
            {
                despawned_reason = Some(*reason);
            }
        }
        if despawned_reason.is_some() {
            break;
        }
    }
    let handoff = handoffs
        .iter()
        .find(|record| record.via == TransitionKind::Connector)
        .expect("the wrong-way rider hands off through the reverse connector");
    assert_eq!(
        handoff.from_facility,
        tangle_model::FacilityId::from_index(0)
    );
    assert_eq!(handoff.to_facility, tangle_model::FacilityId::from_index(1));
    assert_eq!(handoff.from_direction, MovementDirection::Reverse);
    assert_eq!(handoff.to_direction, MovementDirection::Reverse);
    assert_eq!(
        despawned_reason,
        Some(tangle_sim::DespawnReason::ExitedPath),
        "the rider completes the opposing traversal at the far end"
    );
    assert!(before.s_m > 0.0);
}

/// A rider whose authored rule direction is the reference's reverse travels the
/// opposite way after the entry and completes the route forward: the mirrored
/// case, so progress and completion are exercised in both authored reference
/// directions.
#[test]
fn a_reverse_rule_rider_completes_the_route_in_the_forward_direction() {
    let mut sim = build(&reverse_rule_scenario(72.0));
    // Riders enter at the reference end and travel reverse, so their arc length
    // decreases; wait until the lone rider is well inside the facility.
    let (rider, before) = lone_rider_on(&mut sim, 0, 80.0..=95.0);
    assert!(sim.request_wrong_way_entry(rider));

    let mut handoffs: Vec<FacilityTransitionRecord> = Vec::new();
    let mut despawned_reason = None;
    let mut increased = 0;
    let mut s_m = before.s_m;
    for _ in 0..600 {
        let output = sim.step();
        handoffs.extend(
            output
                .facility_transitions()
                .iter()
                .filter(|record| record.agent == rider)
                .copied(),
        );
        for event in output.events() {
            if let tangle_sim::Event::Despawned { agent, reason, .. } = event
                && *agent == rider
            {
                despawned_reason = Some(*reason);
            }
        }
        if let Some(after) = placements(&sim).get(&rider) {
            if after.s_m > s_m {
                increased += 1;
            }
            s_m = after.s_m;
        }
        if despawned_reason.is_some() {
            break;
        }
    }
    assert!(increased > 0, "the rider travels the opposing direction");
    let handoff = handoffs
        .iter()
        .find(|record| record.via == TransitionKind::Connector)
        .expect("the wrong-way rider hands off through the forward connector");
    assert_eq!(
        handoff.from_facility,
        tangle_model::FacilityId::from_index(0)
    );
    assert_eq!(handoff.to_facility, tangle_model::FacilityId::from_index(1));
    assert_eq!(handoff.from_direction, MovementDirection::Forward);
    assert_eq!(handoff.to_direction, MovementDirection::Forward);
    assert_eq!(
        despawned_reason,
        Some(tangle_sim::DespawnReason::ExitedPath),
        "the rider completes the opposing traversal at the far end"
    );
}
