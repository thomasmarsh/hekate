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
//!
//! TAS-138 adds the ordering evidence for a turned rider: its leaders and its
//! own within-tick records follow the direction it actually travels rather than
//! the direction the facility authors. Those fixtures hold a queue of riders at
//! a pedestrian-occupied crossing — a yield, so no signal and no compliance is
//! involved — and turn the front rider of that queue: the vehicle behind it is
//! then ahead of the turned rider in its actual travel direction and behind it
//! in the authored nominal one, which is exactly the asymmetry each test
//! asserts.

use std::collections::BTreeMap;

use glam::DVec2;
use hekate_model::{
    CompiledScenario, CrossingId, MovementDirection, NominalDirection, PermissionEffect,
    parse_scenario_source_v2,
};
use hekate_sim::{
    AgentId, AgentMode, DespawnReason, Event, FacilityTransitionRecord, ManeuverState,
    NEAR_MISS_THRESHOLD_M, RouteStateSample, RunConfig, Simulation, SnapshotDetail, TransitionKind,
    WrongWayReason,
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
fn placements(sim: &Simulation) -> BTreeMap<hekate_sim::AgentId, Placement> {
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
) -> (hekate_sim::AgentId, Placement) {
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
            if let hekate_sim::Event::Despawned { agent, reason, .. } = event
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
        hekate_model::FacilityId::from_index(0)
    );
    assert_eq!(handoff.to_facility, hekate_model::FacilityId::from_index(1));
    assert_eq!(handoff.from_direction, MovementDirection::Reverse);
    assert_eq!(handoff.to_direction, MovementDirection::Reverse);
    assert_eq!(
        despawned_reason,
        Some(hekate_sim::DespawnReason::ExitedPath),
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
            if let hekate_sim::Event::Despawned { agent, reason, .. } = event
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
        hekate_model::FacilityId::from_index(0)
    );
    assert_eq!(handoff.to_facility, hekate_model::FacilityId::from_index(1));
    assert_eq!(handoff.from_direction, MovementDirection::Forward);
    assert_eq!(handoff.to_direction, MovementDirection::Forward);
    assert_eq!(
        despawned_reason,
        Some(hekate_sim::DespawnReason::ExitedPath),
        "the rider completes the opposing traversal at the far end"
    );
}

/// A recorded handoff emits exactly one `FacilityTransition` event that maps the
/// record's every field, and the event takes its documented place in the step's
/// key order: the emission is the handoff fact the physical-advance stage already
/// produced, so a repeated step cannot duplicate or reorder it.
#[test]
fn a_connector_handoff_emits_one_facility_transition_event_mapping_its_record() {
    let mut sim = build(&forward_rule_scenario(72.0));
    let (rider, _) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(sim.request_wrong_way_entry(rider));

    let mut records: Vec<FacilityTransitionRecord> = Vec::new();
    let mut events: Vec<Event> = Vec::new();
    for _ in 0..600 {
        let output = sim.step();
        for window in output.events().windows(2) {
            assert!(
                window[0].order_key() <= window[1].order_key(),
                "the step buffer is non-decreasing by the documented order key"
            );
        }
        // The rider's own records keep the documented kind order, so the
        // handoff takes its own position in the step's sort: the record is the
        // handoff fact the physical-advance stage already produced, and only
        // the kinds that sort after a handoff (an opposing-traversal interval
        // edge, a close pass) can follow it within one step.
        let kinds: Vec<u8> = output
            .events()
            .iter()
            .filter(|event| event.agent() == rider)
            .map(|event| event.kind().order())
            .collect();
        assert!(
            kinds.windows(2).all(|pair| pair[0] <= pair[1]),
            "the rider's records keep the documented kind order: {kinds:?}"
        );
        records.extend(
            output
                .facility_transitions()
                .iter()
                .filter(|record| record.agent == rider)
                .copied(),
        );
        events.extend(
            output
                .events()
                .iter()
                .filter(|event| event.agent() == rider)
                .cloned(),
        );
        if events
            .iter()
            .any(|event| matches!(event, Event::Despawned { .. }))
        {
            break;
        }
    }

    assert!(
        records
            .iter()
            .any(|record| record.via == TransitionKind::Connector),
        "the wrong-way rider hands off through the reverse connector"
    );
    let handoff = records.first().copied().expect("the handoff was recorded");
    let emitted = events
        .iter()
        .find_map(|event| match event {
            Event::FacilityTransition { agent, .. } if *agent == rider => Some(event.clone()),
            _ => None,
        })
        .expect("the handoff emitted its event");
    assert_eq!(
        emitted,
        Event::FacilityTransition {
            agent: handoff.agent,
            from_facility: handoff.from_facility,
            to_facility: handoff.to_facility,
            from_direction: handoff.from_direction,
            to_direction: handoff.to_direction,
            via: handoff.via,
            side: handoff.side,
            s_m: handoff.s_m,
            d_m: handoff.d_m,
            permitted: handoff.permitted,
        },
        "the event maps the record's every field"
    );
    let handoff_events = events
        .iter()
        .filter(|event| matches!(event, Event::FacilityTransition { .. }))
        .count();
    assert_eq!(
        handoff_events,
        records.len(),
        "exactly one FacilityTransition event per recorded handoff"
    );
}

// ---------------------------------------------------------------------------
// TAS-138: ordering a turned rider's leaders and encounters.
// ---------------------------------------------------------------------------

/// Which authored reference direction a TAS-138 queue fixture uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum QueueReference {
    /// Facility `a` is `forward`-nominal, so a rider enters at the path start
    /// travelling forward and the wrong-way entry turns it onto the reverse
    /// traversal.
    Forward,
    /// Facility `d` is `reverse`-nominal, so a rider enters at the path end
    /// travelling reverse and the wrong-way entry turns it onto the forward
    /// traversal. The authored reference is the forward fixture's mirror.
    Reverse,
}

impl QueueReference {
    /// The per-reference names, portal ends, and connector directions the
    /// fixture authors: the neighbour band lies beyond the traversal the
    /// wrong-way entry leaves, so the queued travel direction and the reference
    /// direction are mirror images of each other.
    fn parts(self) -> QueueParts {
        match self {
            Self::Forward => QueueParts {
                id: "queue_forward_v2",
                facility: "a",
                neighbour: "left",
                guide: "guide_a",
                entry: "a_entry",
                exit: "a_exit",
                entry_end: "start",
                exit_end: "end",
                movement: "through",
                travel: "forward",
                guide_points: "[ { x: 0.0, y: 0.0 }, { x: 220.0, y: 0.0 } ]",
                neighbour_points: "[ { x: -220.0, y: 0.0 }, { x: 0.0, y: 0.0 } ]",
                neighbour_region: "[ { x: -220.0, y: -1.5 }, { x: 0.0, y: -1.5 }, \
                    { x: 0.0, y: 1.5 }, { x: -220.0, y: 1.5 } ]",
                crossing_points: "[ { x: 70.0, y: -5.0 }, { x: 80.0, y: -5.0 }, \
                    { x: 80.0, y: 5.0 }, { x: 70.0, y: 5.0 } ]",
                walk_points: "[ { x: 75.0, y: -1.0 }, { x: 75.0, y: 10.0 } ]",
                connectors: concat!(
                    "    { id: 'neighbour_into_main',\n",
                    "      from: { facility: 'left', direction: 'forward' },\n",
                    "      to: { facility: 'a', direction: 'forward' } },\n",
                    "    { id: 'main_back_to_neighbour',\n",
                    "      from: { facility: 'a', direction: 'reverse' },\n",
                    "      to: { facility: 'left', direction: 'reverse' } },\n",
                ),
            },
            Self::Reverse => QueueParts {
                id: "queue_reverse_v2",
                facility: "d",
                neighbour: "right",
                guide: "guide_d",
                entry: "d_entry",
                exit: "d_exit",
                entry_end: "end",
                exit_end: "start",
                movement: "back",
                travel: "reverse",
                guide_points: "[ { x: 0.0, y: 0.0 }, { x: 220.0, y: 0.0 } ]",
                neighbour_points: "[ { x: 220.0, y: 0.0 }, { x: 440.0, y: 0.0 } ]",
                neighbour_region: "[ { x: 220.0, y: -1.5 }, { x: 440.0, y: -1.5 }, \
                    { x: 440.0, y: 1.5 }, { x: 220.0, y: 1.5 } ]",
                crossing_points: "[ { x: 140.0, y: -5.0 }, { x: 150.0, y: -5.0 }, \
                    { x: 150.0, y: 5.0 }, { x: 140.0, y: 5.0 } ]",
                walk_points: "[ { x: 145.0, y: -1.0 }, { x: 145.0, y: 10.0 } ]",
                connectors: concat!(
                    "    { id: 'neighbour_into_main',\n",
                    "      from: { facility: 'right', direction: 'reverse' },\n",
                    "      to: { facility: 'd', direction: 'reverse' } },\n",
                    "    { id: 'main_back_to_neighbour',\n",
                    "      from: { facility: 'd', direction: 'forward' },\n",
                    "      to: { facility: 'right', direction: 'forward' } },\n",
                ),
            },
        }
    }
}

struct QueueParts {
    id: &'static str,
    facility: &'static str,
    neighbour: &'static str,
    guide: &'static str,
    entry: &'static str,
    exit: &'static str,
    entry_end: &'static str,
    exit_end: &'static str,
    movement: &'static str,
    travel: &'static str,
    guide_points: &'static str,
    neighbour_points: &'static str,
    neighbour_region: &'static str,
    crossing_points: &'static str,
    walk_points: &'static str,
    /// The connector lines joining the two facilities, whose leaving and
    /// entering ends must coincide for the traversal the wrong-way entry lands
    /// on to be physically possible.
    connectors: &'static str,
}

const CROSSING_OCCUPANT_MODE: &str = r#"
    {
      id: 'pedestrian',
      body: { kind: 'circle', radius_m: { min: 0.25, max: 0.25 } },
      motion: 'holonomic_walking',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'path' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 0.01, max: 0.01 },
        compliance: { min: 0.0, max: 0.0 },
      },
    }"#;

fn queue_scenario(reference: QueueReference, rate_per_hour: f64) -> String {
    let parts = reference.parts();
    format!(
        r#"{{
  schema_version: 2,
  id: '{id}',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: '{guide}', points: {guide_points} }},
    {{ id: 'guide_neighbour', points: {neighbour_points} }},
    {{ id: 'walk', points: {walk_points} }},
  ],
  portals: [
    {{ id: '{entry}', path: '{guide}', end: '{entry_end}', width_m: 3.0 }},
    {{ id: '{exit}', path: '{guide}', end: '{exit_end}', width_m: 3.0 }},
    {{ id: 'walk_south', path: 'walk', end: 'start', width_m: 1.0 }},
    {{ id: 'walk_north', path: 'walk', end: 'end', width_m: 1.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -440.0, y: -20.0 }}, {{ x: 640.0, y: -20.0 }},
      {{ x: 640.0, y: 20.0 }}, {{ x: -440.0, y: 20.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band_main', points: [
      {{ x: 0.0, y: -1.5 }}, {{ x: 220.0, y: -1.5 }},
      {{ x: 220.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
    ] }},
    {{ id: 'band_neighbour', points: {neighbour_region} }},
    {{ id: 'crossing_zone', points: {crossing_points} }},
  ],
  facilities: [
    {{ id: '{facility}', region: 'band_main', reference_path: '{guide}',
      width_m: 3.0, nominal_direction: '{travel}',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
    {{ id: '{neighbour}', region: 'band_neighbour', reference_path: 'guide_neighbour',
      width_m: 3.0, nominal_direction: 'forward',
      access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
      speed_policy: {{ limit_mps: null }} }},
  ],
  facility_connectors: [
{connectors}  ],
  movements: [
    {{ id: '{movement}', from: '{entry}', to: '{exit}', path: '{guide}',
      priority: 0, direction: '{travel}' }},
  ],
  crossings: [
    {{ id: 'road_crossing', region: 'crossing_zone', movements: [ '{movement}' ] }},
  ],
  rules: [
    {{ id: 'r_yield', movement: '{movement}', kind: 'yield' }},
  ],
  pedestrian_routes: [
    {{ id: 'creep', from: 'walk_south', to: 'walk_north', path: 'walk' }},
  ],
  mode_templates: [ {RIDER_MODE}, {CROSSING_OCCUPANT_MODE} ],{WRONG_WAY_POLICY}
  demand: [
    {{ id: 'rider_inflow', mode: 'rider',
      spawn: {{ rate: {{
        portal: '{entry}',
        rate_per_hour: {rate_per_hour:?},
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: '{movement}', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'creep_inflow', mode: 'pedestrian',
      spawn: {{ rate: {{
        portal: 'walk_south',
        rate_per_hour: 7200.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ routes: [ {{ route: 'creep', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#,
        id = parts.id,
        guide = parts.guide,
        guide_points = parts.guide_points,
        neighbour_points = parts.neighbour_points,
        neighbour_region = parts.neighbour_region,
        crossing_points = parts.crossing_points,
        walk_points = parts.walk_points,
        entry = parts.entry,
        entry_end = parts.entry_end,
        exit = parts.exit,
        exit_end = parts.exit_end,
        facility = parts.facility,
        neighbour = parts.neighbour,
        travel = parts.travel,
        movement = parts.movement,
        connectors = parts.connectors,
    )
}

/// The documented within-tick order key of one record, as
/// [`Event::order_key`] reports it.
type OrderKey = (u32, u8, u8, u32, u32, u32, u32, u32);

/// Arc length of the crossing region's entry as the forward-reference queue
/// approaches it, and as the reverse-reference queue does. Each fixture places
/// its crossing so that the queue stops well inside the first half of the
/// reference *in its own travel frame*: a forward approach reaches the region at
/// its lowest arc, a reverse one at its highest. The [TAS-117] decision only
/// selects the opposing traversal when it saves time, which it does exactly
/// when the ridden half of the reference is the shorter one.
const FORWARD_YIELD_ENTRY_M: f64 = 70.0;
const REVERSE_YIELD_ENTRY_M: f64 = 150.0;

/// One vehicle's queued position, in its own travel frame.
#[derive(Clone, Copy)]
struct QueuedRider {
    id: AgentId,
    /// Arc length along the queued facility.
    s_m: f64,
    speed_mps: f64,
    body_length_m: f64,
    /// The crossing this vehicle is currently yielding to, if any.
    yield_crossing: Option<CrossingId>,
}

/// The queued vehicles on the fixture's facility, front of the queue first.
///
/// The queue is ordered by distance from the crossing entry along the authored
/// travel direction, so "front" is the vehicle that reached the yield point
/// first in either reference direction.
fn queue(sim: &Simulation, reference: QueueReference) -> Vec<QueuedRider> {
    let entry = match reference {
        QueueReference::Forward => FORWARD_YIELD_ENTRY_M,
        QueueReference::Reverse => REVERSE_YIELD_ENTRY_M,
    };
    let mut queued: Vec<QueuedRider> = sim
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            (motion.mode == AgentMode::Vehicle).then(|| QueuedRider {
                id: sample.id,
                s_m: motion.path_distance_m,
                speed_mps: motion.speed_mps,
                body_length_m: motion.body_length_m,
                yield_crossing: sim.agent_yield_crossing(sample.id),
            })
        })
        .collect();
    queued.sort_by(|first, second| {
        (first.s_m - entry)
            .abs()
            .total_cmp(&(second.s_m - entry).abs())
    });
    queued
}

/// The front of the queue and the vehicle immediately behind it, once the two
/// are at rest and queued bumper to bumper, or `None` while they are not.
fn queued_pair_at(
    sim: &Simulation,
    reference: QueueReference,
) -> Option<(QueuedRider, QueuedRider)> {
    let queued = queue(sim, reference);
    let (front, next) = (queued.first()?, queued.get(1)?);
    let gap_m = (front.s_m - next.s_m).abs() - front.body_length_m;
    (front.speed_mps <= 0.01 && next.speed_mps <= 0.01 && (1.0..=5.0).contains(&gap_m))
        .then_some((*front, *next))
}

/// Step until the queue's front pair is queued bumper to bumper.
fn queued_pair(sim: &mut Simulation, reference: QueueReference) -> (QueuedRider, QueuedRider) {
    for _ in 0..4000 {
        sim.step();
        if let Some(pair) = queued_pair_at(sim, reference) {
            return pair;
        }
    }
    panic!("two riders must queue at the yield point on the {reference:?} fixture");
}

/// Drive the turned rider for `ticks` steps, reporting how fast it travelled,
/// how close it came to the body ahead of it in its actual travel direction,
/// and whether the pair ever touched.
struct TurnTrace {
    max_speed_mps: f64,
    min_gap_m: f64,
    collisions: u64,
}

fn drive_turned_rider(
    sim: &mut Simulation,
    reference: QueueReference,
    subject: AgentId,
    leader: AgentId,
    ticks: u64,
) -> TurnTrace {
    let mut trace = TurnTrace {
        max_speed_mps: 0.0,
        min_gap_m: f64::INFINITY,
        collisions: 0,
    };
    for _ in 0..ticks {
        let events: Vec<Event> = sim.step().events().to_vec();
        for event in &events {
            if let Event::Collision { agent, other, .. } = event
                && (*agent == subject || *agent == leader)
                && (*other == subject || *other == leader)
            {
                trace.collisions += 1;
            }
        }
        let queued = queue(sim, reference);
        let Some(subject_now) = queued.iter().find(|rider| rider.id == subject) else {
            break;
        };
        let Some(leader_now) = queued.iter().find(|rider| rider.id == leader) else {
            break;
        };
        trace.max_speed_mps = trace.max_speed_mps.max(subject_now.speed_mps);
        let gap_m = (subject_now.s_m - leader_now.s_m).abs()
            - (subject_now.body_length_m + leader_now.body_length_m) * 0.5;
        trace.min_gap_m = trace.min_gap_m.min(gap_m);
    }
    trace
}

/// A rider that has entered the opposing traversal is bounded by the queued
/// vehicle behind it in the authored direction and ahead of it in the direction
/// it actually travels, in either authored reference direction.
///
/// The fixture holds a red-free queue at a pedestrian-occupied crossing: the
/// front rider is the subject and the rider behind it is the leader, so the
/// subject's actual travel direction points at the leader while the authored
/// nominal direction points away from it. The turned rider's desired speed is
/// its own 6 m/s, it carries no yield, no stop line, and no lateral maneuver, so
/// the only constraint that can hold it at rest is the leader its actual travel
/// direction names.
#[test]
fn a_turned_rider_is_bounded_by_the_queue_behind_it_in_both_reference_directions() {
    for reference in [QueueReference::Forward, QueueReference::Reverse] {
        let mut sim = build(&queue_scenario(reference, 720.0));
        let (subject, leader) = queued_pair(&mut sim, reference);
        assert_eq!(
            subject.yield_crossing,
            Some(CrossingId::from_index(0)),
            "the fixture holds the front rider at the crossing ({reference:?})"
        );
        assert!(
            subject.speed_mps <= 0.01,
            "the fixture's front rider is at rest: {} m/s ({reference:?})",
            subject.speed_mps
        );

        assert!(
            sim.request_wrong_way_entry(subject.id),
            "a reverse-capable rider on a two-way facility can request the entry ({reference:?})"
        );
        // The entry drops the movement, so the rider's yield ends with it: the
        // turn landed and nothing but the ordinary interaction path holds the
        // rider afterwards.
        let mut turned = false;
        for _ in 0..200 {
            sim.step();
            if sim.agent_yield_crossing(subject.id).is_none() {
                turned = true;
                break;
            }
        }
        assert!(turned, "the wrong-way entry landed ({reference:?})");
        assert_eq!(
            sim.agent_decision(subject.id),
            None,
            "a turned rider carries no signal decision and no stop line ({reference:?})"
        );
        let desired = sim
            .agent_profile(subject.id)
            .expect("a demand vehicle carries a sampled profile")
            .desired_speed_mps;
        assert!(
            (desired - 6.0).abs() < 1e-9,
            "the fixture's rider desires {desired} m/s ({reference:?})"
        );

        let trace = drive_turned_rider(&mut sim, reference, subject.id, leader.id, 200);
        assert_eq!(
            trace.collisions, 0,
            "the turned rider never touches the body ahead of it ({reference:?})"
        );
        assert!(
            trace.min_gap_m > 0.5,
            "the turned rider keeps a positive gap to the body ahead of it: {} m ({reference:?})",
            trace.min_gap_m
        );
        assert!(
            trace.max_speed_mps <= 0.5,
            "the body ahead of it in the travel direction holds the turned rider: {} m/s \
             ({reference:?})",
            trace.max_speed_mps
        );
        assert_eq!(
            sim.emergency_cap_steps(),
            0,
            "no rider leans on the emergency cap ({reference:?})"
        );
    }
}

/// One run of the queue fixture up to and through the wrong-way entry, with the
/// turned rider's own records kept per tick.
struct RecordedTurn {
    /// Every tick's records, in the order the step emitted them.
    ticks: Vec<(u64, Vec<Event>)>,
    /// The turned rider.
    subject: AgentId,
    /// The queued vehicle immediately behind it, which its actual travel
    /// direction points at and its authored nominal direction points away from.
    leader: AgentId,
    /// The turned rider's and that vehicle's arc lengths, per tick.
    arcs: Vec<(u64, f64, f64)>,
}

fn record_turn(reference: QueueReference) -> RecordedTurn {
    let mut sim = build(&queue_scenario(reference, 720.0));
    let mut ticks: Vec<(u64, Vec<Event>)> = Vec::new();
    let mut pair = None;
    let mut requested = false;
    let mut turned = false;
    let mut settled = 0;
    let mut arcs = Vec::new();
    for _ in 0..4000 {
        let tick = sim.time().tick();
        let events: Vec<Event> = sim.step().events().to_vec();
        ticks.push((tick, events));
        if pair.is_none() {
            pair = queued_pair_at(&sim, reference);
        }
        let Some((front, behind)) = pair else {
            continue;
        };
        let queued = queue(&sim, reference);
        if let (Some(subject_now), Some(leader_now)) = (
            queued.iter().find(|rider| rider.id == front.id),
            queued.iter().find(|rider| rider.id == behind.id),
        ) {
            arcs.push((tick, subject_now.s_m, leader_now.s_m));
        }
        let rider = front.id;
        if !requested {
            assert!(
                sim.request_wrong_way_entry(rider),
                "a queued reverse-capable rider can request the entry ({reference:?})"
            );
            requested = true;
            continue;
        }
        if sim.agent_yield_crossing(rider).is_none() {
            turned = true;
        }
        if turned {
            settled += 1;
            if settled >= 100 {
                break;
            }
        }
    }
    assert!(turned, "the wrong-way entry landed ({reference:?})");
    let (front, behind) = pair.expect("the fixture queues a rider");
    RecordedTurn {
        ticks,
        subject: front.id,
        leader: behind.id,
        arcs,
    }
}

/// The turned rider's own records keep the documented within-tick order in
/// both authored reference directions, and no record is ordered by the
/// direction the rider no longer travels.
///
/// The queue fixture turns its front rider onto the opposing traversal of the
/// facility, so the rider the records are about travels the reference's reverse
/// in one fixture and its forward in the other. Its own records must therefore
/// sit in the same order under both authored reference directions while its arc
/// lengths run in opposite senses: the emitted order cannot be a function of
/// where it sits along the reference it no longer rides.
#[test]
fn the_turned_riders_own_records_keep_one_order_in_both_reference_directions() {
    // The one record that carries an arc length is not ordered by it: a spawn at
    // the reference's start and one at its end share a within-tick key, so no
    // record's position in a tick can come from the distance it was reported at.
    let at_start = Event::Spawned {
        agent: AgentId::from_index(0),
        mode: AgentMode::Vehicle,
        path: hekate_model::PathId::from_index(0),
        distance_m: 0.0,
    };
    let at_end = Event::Spawned {
        agent: AgentId::from_index(0),
        mode: AgentMode::Vehicle,
        path: hekate_model::PathId::from_index(0),
        distance_m: 220.0,
    };
    assert_eq!(
        at_start.order_key(),
        at_end.order_key(),
        "no record's within-tick key reads the arc length it carries"
    );

    let forward = record_turn(QueueReference::Forward);
    let reverse = record_turn(QueueReference::Reverse);

    // Every tick's records follow the documented order, in both directions.
    let mut multi_agent_ticks = 0;
    for (run, reference) in [
        (&forward, QueueReference::Forward),
        (&reverse, QueueReference::Reverse),
    ] {
        for (tick, events) in &run.ticks {
            assert!(
                events
                    .windows(2)
                    .all(|pair| pair[0].order_key() <= pair[1].order_key()),
                "tick {tick} is out of order on the {reference:?} fixture: {events:?}"
            );
            let agents: std::collections::BTreeSet<AgentId> =
                events.iter().map(|event| event.agent()).collect();
            if agents.len() > 1 {
                multi_agent_ticks += 1;
            }
        }
    }
    assert!(
        multi_agent_ticks > 0,
        "the fixture must exercise a tick carrying records about more than one agent"
    );

    // In that tick the turned rider's own record comes first, although the body
    // recorded beside it is the *backmarker* in the authored nominal direction.
    // Ordering the tick by nominal progress would report that body first; the
    // documented key — ascending agent id, then kind — reports the turned rider
    // first, so no record is placed by the direction the rider no longer
    // travels.
    for (run, nominal_sign, reference) in [
        (&forward, 1.0, QueueReference::Forward),
        (&reverse, -1.0, QueueReference::Reverse),
    ] {
        let tick = run
            .ticks
            .iter()
            .find(|(_, events)| {
                let agents: std::collections::BTreeSet<AgentId> =
                    events.iter().map(|event| event.agent()).collect();
                agents.contains(&run.subject) && agents.contains(&run.leader)
            })
            .expect("a tick must carry records about the turned rider and its leader");
        assert_eq!(
            tick.1.first().map(|event| event.agent()),
            Some(run.subject),
            "the turned rider's record comes first ({reference:?})"
        );
        let (_, subject_s_m, leader_s_m) = run
            .arcs
            .iter()
            .find(|(arc_tick, ..)| *arc_tick == tick.0)
            .expect("the pair's arc lengths are recorded for that tick");
        assert!(
            nominal_sign * leader_s_m < nominal_sign * subject_s_m,
            "the body recorded beside the turned rider is behind it in the authored \
             nominal direction, and ahead of it only in the travel direction it no longer \
             holds ({reference:?})"
        );
    }

    let own = |run: &RecordedTurn| -> Vec<(u64, Vec<OrderKey>)> {
        run.ticks
            .iter()
            .filter_map(|(tick, events)| {
                let keys: Vec<OrderKey> = events
                    .iter()
                    .filter(|event| event.agent() == run.subject)
                    .map(|event| event.order_key())
                    .collect();
                (!keys.is_empty()).then_some((*tick, keys))
            })
            .collect()
    };
    let forward_records = own(&forward);
    let reverse_records = own(&reverse);
    assert_eq!(
        forward_records, reverse_records,
        "the turned rider's own records order the same way in both authored reference directions"
    );
    assert!(
        forward_records
            .iter()
            .map(|(_, keys)| keys.len())
            .sum::<usize>()
            >= 2,
        "the fixture must record at least two of the turned rider's own records: {forward_records:?}"
    );

    // The probe is not vacuous: the same order arises from mirrored arc lengths,
    // so the rider's records were reported at positions that run the opposite
    // way along the reference.
    let spawned_at = |run: &RecordedTurn| -> f64 {
        run.ticks
            .iter()
            .flat_map(|(_, events)| events.iter())
            .find_map(|event| match event {
                Event::Spawned {
                    agent, distance_m, ..
                } if *agent == run.subject => Some(*distance_m),
                _ => None,
            })
            .expect("the turned rider was announced by a spawned record")
    };
    assert_ne!(
        spawned_at(&forward),
        spawned_at(&reverse),
        "the two fixtures must report the rider at mirrored arc lengths"
    );
}

// ---------------------------------------------------------------------------
// TAS-118: an occupied opposing corridor and the ordinary lifecycle.
// ---------------------------------------------------------------------------
//
// The contract makes an occupied opposing corridor a decision input, never a
// precondition failure: the agent still selects the opposing traversal, and the
// ordinary leader, collision, clearance, and safety machinery bounds the
// attempt. These fixtures put a body in the corridor a turned rider drives into
// — a rider that entered the same facility after it and still travels the
// nominal direction, which is exactly the traffic the opposing traversal meets
// head-on — and assert that the outcome comes from that ordinary machinery: no
// flag is authored, nothing scripts a trajectory, and no query is disabled.
//
// [`RIDER_MODE`] turns around, so the turned rider and the body it turned into
// are head-on on one reference: the rider's arc length falls while the body
// ahead of it in that travel direction climbs. The sections below assert the
// bounded brake, the collision scan's visibility of the pair, the identity and
// lifecycle the turn preserves, the capability gate on a shared facility, and
// the disconnected rejection.

/// One live rider's state on a facility, read from a full snapshot.
#[derive(Clone, Copy)]
struct LiveRider {
    id: AgentId,
    /// Arc length along the facility reference in metres.
    s_m: f64,
    speed_mps: f64,
    position: DVec2,
    body_length_m: f64,
}

/// Every live rider on `path`, ascending by arc length.
fn live_riders(sim: &Simulation, path: usize) -> Vec<LiveRider> {
    let mut riders: Vec<LiveRider> = sim
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .filter_map(|sample| {
            let motion = sample.motion.as_ref()?;
            (motion.path.index() == path).then_some(LiveRider {
                id: sample.id,
                s_m: motion.path_distance_m,
                speed_mps: motion.speed_mps,
                position: sample.position,
                body_length_m: motion.body_length_m,
            })
        })
        .collect();
    riders.sort_by(|first, second| first.s_m.total_cmp(&second.s_m));
    riders
}

/// The bumper-to-bumper gap between two bodies on one path, in metres.
fn bumper_gap_m(first: LiveRider, second: LiveRider) -> f64 {
    (first.s_m - second.s_m).abs() - (first.body_length_m + second.body_length_m) * 0.5
}

/// Step until exactly two riders ride `path`, the leading one lies inside
/// `subject_window`, and its bumper gap to the other lies inside `gap_window`;
/// return the leading rider and the body it turns into.
///
/// The pair is deterministic for the fixture's fixed seed: the leading rider is
/// the earlier arrival and the other entered `path` later, so it is the
/// oncoming occupancy of the traversal the leading rider is about to enter.
fn leading_pair_on(
    sim: &mut Simulation,
    path: usize,
    subject_window: std::ops::RangeInclusive<f64>,
    gap_window: std::ops::RangeInclusive<f64>,
) -> (LiveRider, LiveRider) {
    for _ in 0..8000 {
        sim.step();
        let riders = live_riders(sim, path);
        if let [occupancy, subject] = riders[..] {
            let gap_m = bumper_gap_m(subject, occupancy);
            if subject_window.contains(&subject.s_m) && gap_window.contains(&gap_m) {
                return (subject, occupancy);
            }
        }
    }
    panic!(
        "two riders must ride path {path} with the leading one in {subject_window:?} \
         and a bumper gap in {gap_window:?}"
    );
}

/// The contact and near-miss records one tick's events carry about one pair.
#[derive(Default)]
struct PairScan {
    /// Reported clearance and edge flag of every contact record.
    contacts: Vec<(f64, bool)>,
    /// Reported clearance and edge flag of every near-miss record.
    near_misses: Vec<(f64, bool)>,
}

impl PairScan {
    /// The records `events` carry about the pair `first`/`second`, in either
    /// argument order.
    fn of(events: &[Event], first: AgentId, second: AgentId) -> Self {
        let is_pair = |agent: &AgentId, other: &AgentId| {
            (*agent == first && *other == second) || (*agent == second && *other == first)
        };
        let mut scan = Self::default();
        for event in events {
            match event {
                Event::Collision {
                    agent,
                    other,
                    clearance_m,
                    contacting,
                } if is_pair(agent, other) => scan.contacts.push((*clearance_m, *contacting)),
                Event::NearMiss {
                    agent,
                    other,
                    clearance_m,
                    entering,
                } if is_pair(agent, other) => scan.near_misses.push((*clearance_m, *entering)),
                _ => {}
            }
        }
        scan
    }

    /// Extend this scan with another tick's records.
    fn extend(&mut self, other: Self) {
        self.contacts.extend(other.contacts);
        self.near_misses.extend(other.near_misses);
    }

    /// Whether the scan opened the contact band for the pair.
    fn opened_contact(&self) -> bool {
        self.contacts.iter().any(|(_, contacting)| *contacting)
    }

    /// Whether the scan opened the near-miss band for the pair.
    fn opened_near_miss(&self) -> bool {
        self.near_misses.iter().any(|(_, entering)| *entering)
    }

    /// The least clearance the scan reported for the pair, or `None` when it
    /// reported no record at all.
    fn minimum_clearance_m(&self) -> Option<f64> {
        self.contacts
            .iter()
            .chain(&self.near_misses)
            .map(|(clearance_m, _)| *clearance_m)
            .reduce(f64::min)
    }
}

/// An occupied opposing corridor bounds the turned rider through the ordinary
/// leader constraint.
///
/// The fixture leaves the ordinary model room to stop, so nothing but the
/// leader ahead of the turned rider holds it: the rider brakes to a standstill
/// behind the oncoming body, keeps a positive gap, and never comes within the
/// collision scan's near-miss band. The anti-overlap position cap never binds,
/// which is what makes this a bounded brake of the ordinary machinery rather
/// than the kernel's documented backstop.
#[test]
fn an_occupied_opposing_corridor_bounds_the_turned_rider() {
    let mut sim = build(&forward_rule_scenario(720.0));
    let (subject, occupancy) = leading_pair_on(&mut sim, 0, 30.0..=48.0, 25.0..=60.0);
    let desired_speed_mps = sim
        .agent_profile(subject.id)
        .expect("a demand rider carries a sampled profile")
        .desired_speed_mps;
    assert!(
        (desired_speed_mps - 6.0).abs() < 1e-9,
        "the fixture's rider desires {desired_speed_mps} m/s"
    );
    assert!(
        sim.request_wrong_way_entry(subject.id),
        "a reverse-capable rider on a two-way facility can request the entry"
    );

    let mut previous_position = subject.position;
    let mut subject_s_m = subject.s_m;
    let mut occupancy_s_m = occupancy.s_m;
    let mut min_gap_m = f64::INFINITY;
    let mut min_speed_mps = f64::INFINITY;
    let mut scan = PairScan::default();
    let mut turned = false;
    for _ in 0..400 {
        let events: Vec<Event> = sim.step().events().to_vec();
        scan.extend(PairScan::of(&events, subject.id, occupancy.id));
        let riders = live_riders(&sim, 0);
        let (Some(subject_now), Some(occupancy_now)) = (
            riders.iter().find(|rider| rider.id == subject.id),
            riders.iter().find(|rider| rider.id == occupancy.id),
        ) else {
            panic!("both riders stay alive through the encounter");
        };
        // The turn lands on the first step after the request: the rider's arc
        // length falls while the body it turned into still climbs the
        // reference, so the two travel one corridor head-on.
        if subject_now.s_m < subject_s_m {
            turned = true;
        }
        assert!(
            turned,
            "the requested entry turns the rider onto the opposing traversal"
        );
        assert!(
            subject_now.s_m <= subject_s_m + 1e-9,
            "the turned rider never travels the nominal direction again: {} -> {}",
            subject_s_m,
            subject_now.s_m
        );
        assert!(
            occupancy_now.s_m >= occupancy_s_m - 1e-9,
            "the body it turned into keeps its nominal traversal: {} -> {}",
            occupancy_s_m,
            occupancy_now.s_m
        );
        assert!(
            (subject_now.position - previous_position).length() <= 6.0 * DT + 1e-6,
            "no rider teleported through the turn"
        );
        previous_position = subject_now.position;
        subject_s_m = subject_now.s_m;
        occupancy_s_m = occupancy_now.s_m;
        min_gap_m = min_gap_m.min(bumper_gap_m(*subject_now, *occupancy_now));
        min_speed_mps = min_speed_mps.min(subject_now.speed_mps);
    }

    assert!(
        subject_s_m < subject.s_m - 5.0,
        "the turned rider travels the opposing traversal: {} -> {}",
        subject.s_m,
        subject_s_m
    );
    assert!(
        occupancy_s_m > occupancy.s_m + 5.0,
        "the occupancy travels the nominal traversal: {} -> {}",
        occupancy.s_m,
        occupancy_s_m
    );
    // Nothing else can hold the rider: the turn leaves the movement behind, so
    // it carries no crossing yield, no signal decision, and no lateral
    // maneuver, and its own desired speed is the fixture's free-flow 6 m/s. The
    // body ahead of it in its actual travel direction is the only constraint.
    assert_eq!(sim.agent_yield_crossing(subject.id), None);
    assert_eq!(sim.agent_decision(subject.id), None);
    assert_eq!(
        sim.agent_route(subject.id),
        None,
        "the turn leaves the movement behind"
    );
    assert_eq!(
        sim.snapshot(SnapshotDetail::Full)
            .agents()
            .iter()
            .find(|sample| sample.id == subject.id)
            .and_then(|sample| sample.motion.as_ref())
            .and_then(|motion| motion.route_state)
            .map(|state| state.maneuver_state),
        Some(ManeuverState::Following),
        "the turn performs no lateral maneuver"
    );
    assert!(
        min_speed_mps <= 0.01,
        "the leader ahead holds the turned rider at rest: least speed {min_speed_mps} m/s"
    );
    assert!(
        min_gap_m > 1.0,
        "the turned rider keeps a positive gap to the body it meets: {min_gap_m} m"
    );
    assert!(
        scan.contacts.is_empty() && scan.near_misses.is_empty(),
        "the ordinary leader constraint holds the pair clear of the scan's bands: {scan_contacts:?}",
        scan_contacts = scan.contacts
    );
    assert_eq!(
        sim.emergency_cap_steps(),
        0,
        "the ordinary leader constraint, not the anti-overlap cap, bounds the approach"
    );
}

/// A wrong-way rider that turns into an oncoming body with no room to stop is
/// still bounded and stays visible to the ordinary collision scan.
///
/// Two 6 m/s bodies closing head-on 17 m apart cannot stop inside that gap under
/// the fixture's 2 m/s² comfortable braking, so the encounter is unavoidable
/// whatever the leader constraint says. The ordinary machinery still carries it:
/// the collision scan reports the pair in its near-miss band and then in its
/// contact band, the anti-overlap cap keeps the bodies from overlapping, and
/// the interaction metrics observe the pair under its ordinary agent-pair
/// dimension. Nothing here disables the scan or excludes the pair from it.
#[test]
fn the_collision_scan_reports_the_encounter_the_turned_rider_cannot_avoid() {
    let mut sim = build(&forward_rule_scenario(720.0));
    let (subject, occupancy) = leading_pair_on(&mut sim, 0, 30.0..=42.0, 8.0..=18.0);
    assert!(
        sim.request_wrong_way_entry(subject.id),
        "the entry is decided before any occupancy is read"
    );

    let mut scan = PairScan::default();
    let mut min_gap_m = f64::INFINITY;
    for _ in 0..300 {
        let events: Vec<Event> = sim.step().events().to_vec();
        scan.extend(PairScan::of(&events, subject.id, occupancy.id));
        let riders = live_riders(&sim, 0);
        let (Some(subject_now), Some(occupancy_now)) = (
            riders.iter().find(|rider| rider.id == subject.id),
            riders.iter().find(|rider| rider.id == occupancy.id),
        ) else {
            break;
        };
        min_gap_m = min_gap_m.min(bumper_gap_m(*subject_now, *occupancy_now));
    }

    assert!(
        scan.opened_near_miss(),
        "the collision scan reports the pair inside its near-miss band"
    );
    assert!(
        scan.opened_contact(),
        "the collision scan reports the pair inside its contact band"
    );
    assert!(
        scan.minimum_clearance_m()
            .is_some_and(|clearance| clearance >= -1e-9),
        "the pair never overlaps: least reported clearance {:?}",
        scan.minimum_clearance_m()
    );
    assert!(
        min_gap_m >= -1e-9,
        "the anti-overlap cap holds the bodies apart: least measured gap {min_gap_m} m"
    );
    // The pair is observed under its ordinary agent-pair dimension, not a
    // wrong-way one.
    let separation_m = sim
        .interaction_metrics()
        .pair_minimum_separation_m(subject.id, occupancy.id)
        .expect("the encounter is inside the interaction range");
    assert!(
        separation_m <= NEAR_MISS_THRESHOLD_M,
        "the pair's recorded separation is {separation_m} m"
    );
}

/// One fixture run from admission to the wrong-way completion, with every
/// record it emitted.
struct EntryRun {
    /// The rider the entry was requested for.
    rider: AgentId,
    /// The rider's sampled profile at the decision instant, before the turn.
    profile: hekate_sim::VehicleProfile,
    /// Every tick's records, in the order the step emitted them.
    ticks: Vec<(u64, Vec<Event>)>,
    /// The reason the rider despawned, once it did.
    despawned: Option<DespawnReason>,
}

/// Drive `forward_rule_scenario` as a lone rider: request the wrong-way entry
/// the first time the rider is inside `window`, then keep stepping until that
/// rider despawns, collecting every record the run emitted.
fn run_lone_entry(window: std::ops::RangeInclusive<f64>) -> (Simulation, EntryRun) {
    let mut sim = build(&forward_rule_scenario(72.0));
    let mut rider = None;
    let mut profile = None;
    let mut ticks: Vec<(u64, Vec<Event>)> = Vec::new();
    let mut despawned = None;
    for _ in 0..4000 {
        let tick = sim.time().tick();
        let events: Vec<Event> = sim.step().events().to_vec();
        ticks.push((tick, events));
        let Some(rider) = rider else {
            let riders = live_riders(&sim, 0);
            if let [only] = riders[..]
                && window.contains(&only.s_m)
            {
                profile = sim.agent_profile(only.id);
                assert!(
                    sim.request_wrong_way_entry(only.id),
                    "a lone reverse-capable rider can request the entry"
                );
                rider = Some(only.id);
            }
            continue;
        };
        if let Some(reason) = ticks.last().and_then(|(_, events)| {
            events.iter().find_map(|event| match event {
                Event::Despawned { agent, reason, .. } if *agent == rider => Some(*reason),
                _ => None,
            })
        }) {
            despawned = Some(reason);
            break;
        }
    }
    let rider = rider.expect("the fixture admits a rider");
    let profile = profile.expect("the rider carries a sampled profile");
    (
        sim,
        EntryRun {
            rider,
            profile,
            ticks,
            despawned,
        },
    )
}

/// A wrong-way rider keeps its stable identity, mode, sampled profile, ordinary
/// metric dimensions, event order, and spawn-to-despawn lifecycle.
///
/// The turn changes the travel direction and the route coordinates and nothing
/// else: one spawn and one despawn record name the same `AgentId`, the mode and
/// the profile admission sampled are unchanged, every tick's records keep the
/// documented within-tick order, and the completed trip is served under the
/// ordinary mode and run dimensions.
#[test]
fn a_wrong_way_rider_keeps_its_identity_profile_and_lifecycle() {
    let (sim, run) = run_lone_entry(20.0..=30.0);
    let rider = run.rider;

    let spawned: Vec<Event> = run
        .ticks
        .iter()
        .flat_map(|(_, events)| events.iter())
        .filter(|event| matches!(event, Event::Spawned { agent, .. } if *agent == rider))
        .cloned()
        .collect();
    assert_eq!(
        spawned.len(),
        1,
        "the rider is admitted once and never re-spawned: {spawned:?}"
    );
    let Event::Spawned { mode, path, .. } = spawned[0] else {
        unreachable!("the filtered record is a spawn");
    };
    assert_eq!(mode, AgentMode::Vehicle);
    assert_eq!(path, hekate_model::PathId::from_index(0));
    assert_eq!(
        run.despawned,
        Some(DespawnReason::ExitedPath),
        "the turned rider completes the opposing traversal at the far end"
    );

    // The mode and the profile are exactly what admission chose: the turn
    // neither re-modes the rider nor re-samples its profile.
    assert_eq!(sim.agent_mode(rider), Some(AgentMode::Vehicle));
    assert_eq!(
        sim.agent_profile(rider),
        Some(run.profile),
        "the rider keeps the profile it was admitted with"
    );

    // Every tick's records keep the documented within-tick order, and the
    // turned rider's own records are the same two lifecycle edges.
    for (tick, events) in &run.ticks {
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].order_key() <= pair[1].order_key()),
            "tick {tick} is out of order: {events:?}"
        );
    }
    assert!(
        run.ticks
            .iter()
            .flat_map(|(_, events)| events.iter())
            .filter(|event| {
                matches!(
                    event,
                    Event::Despawned { agent, .. } if *agent == rider
                )
            })
            .count()
            == 1,
        "the rider despawns once"
    );

    // The trip is served under the ordinary mode dimension, exactly as any
    // other vehicle's completed trip is.
    let operation = sim.interaction_metrics().operation();
    let served = operation.values().served_agents;
    assert!(
        served >= 1,
        "the completed trip is served in the run dimension: {served}"
    );
    assert_eq!(
        operation.mode_values(AgentMode::Vehicle).served_agents,
        served,
        "the turned rider is served under its ordinary mode dimension"
    );
}

/// The rider's own mode without `reverse_direction`: the same family, motion,
/// speed, and longitudinal tactics, one capability short, with a longer body so
/// the two participants are distinguishable from the public seam.
fn nominal_only_mode() -> String {
    RIDER_MODE
        .replace(", 'reverse_direction'", "")
        .replace("id: 'rider'", "id: 'nominal_only'")
        .replace(
            "length_m: { min: 1.8, max: 1.8 }",
            "length_m: { min: 2.6, max: 2.6 }",
        )
        .replace(
            "radius_m: { min: 0.35, max: 0.35 }",
            "radius_m: { min: 0.45, max: 0.45 }",
        )
}

/// [`forward_rule_scenario`] with a second participant on facility `a`: one
/// wrong-way-capable mode and one mode that carries no `reverse_direction`.
///
/// Both modes reach the same facility through the same movement, so the
/// capability gate is the only thing that separates them. The anchors below are
/// the fixture's own text, and the result is asserted to carry both templates
/// and both facility accesses, so a fixture edit fails here rather than
/// silently leaving one participant.
fn mixed_capability_scenario(rate_per_hour: f64) -> String {
    // The fixture's demand tail, which the second participant's source is
    // spliced into; the anchor is built from it, so the two cannot drift apart.
    let demand_tail =
        "        choice: { movements: [ { movement: 'through', weight: 1.0 } ] },\n      } } },\n";
    let demand_close = format!("{demand_tail}  ],");
    let second_demand = r#"    { id: 'nominal_only_inflow', mode: 'nominal_only',
      spawn: { rate: {
        portal: 'a_entry',
        rate_per_hour: RATE_PER_HOUR,
        interval_s: { start_s: 0.0, end_s: null },
        choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
      } } },
"#
    .replace("RATE_PER_HOUR", &format!("{rate_per_hour:?}"));
    let mixed = forward_rule_scenario(rate_per_hour)
        .replacen(
            "access: { modes: [ 'rider' ] }",
            "access: { modes: [ 'rider', 'nominal_only' ] }",
            1,
        )
        .replacen(
            "    } ],\n  maneuver_policy:",
            &("    },\n".to_string() + &nominal_only_mode() + " ],\n  maneuver_policy:"),
            1,
        )
        .replacen(
            &demand_close,
            &(demand_tail.to_string() + &second_demand + "  ],"),
            1,
        );
    assert!(
        mixed.contains("access: { modes: [ 'rider', 'nominal_only' ] }")
            && mixed.contains("id: 'nominal_only'")
            && mixed.contains("mode: 'nominal_only'"),
        "the mixed-capability fixture must author the second participant"
    );
    mixed
}

/// A wrong-way-capable participant and a participant that is not, on one
/// facility: the capability gate is per agent and never leaks.
///
/// This is the mixed-capability acceptance input. The capable rider's request
/// is accepted and its traversal turns; the nominal-only participant's request
/// is rejected outright and it never travels the opposing traversal, while both
/// share the facility, the movement, and the profile stream. Neither mode
/// declares a lateral tactic, so the refusal's only possible outcome is that
/// the participant keeps its nominal traversal.
#[test]
fn a_nominal_only_participant_sharing_the_facility_never_reaches_the_entry() {
    let mut sim = build(&mixed_capability_scenario(240.0));
    let mut placed = None;
    for _ in 0..8000 {
        sim.step();
        let riders = live_riders(&sim, 0);
        if let [behind, ahead] = riders[..] {
            let capable = sim.agent_profile(ahead.id).map(|profile| profile.length_m);
            let nominal = sim.agent_profile(behind.id).map(|profile| profile.length_m);
            if capable == Some(1.8) && nominal == Some(2.6) && (20.0..=45.0).contains(&ahead.s_m) {
                placed = Some((ahead, behind));
                break;
            }
        }
    }
    let (rider, nominal_only) = placed
        .expect("the fixture must place the reverse-capable rider ahead of the nominal-only rider");

    assert!(
        sim.request_wrong_way_entry(rider.id),
        "the reverse-capable rider can request the entry"
    );
    assert!(
        !sim.request_wrong_way_entry(nominal_only.id),
        "a mode without `reverse_direction` never reaches the entry"
    );

    // The rejection is total: on the very next step the capable rider is
    // already travelling the opposing traversal while the refused participant
    // still climbs the reference, and it never reverses afterwards.
    sim.step();
    let after = live_riders(&sim, 0);
    let rider_now = after
        .iter()
        .find(|other| other.id == rider.id)
        .expect("the turned rider stays alive");
    let nominal_now = after
        .iter()
        .find(|other| other.id == nominal_only.id)
        .expect("the refused rider stays alive");
    assert!(
        rider_now.s_m < rider.s_m,
        "the accepted request turns the capable rider: {} -> {}",
        rider.s_m,
        rider_now.s_m
    );
    assert!(
        nominal_now.s_m > nominal_only.s_m,
        "the refused participant keeps its nominal traversal: {} -> {}",
        nominal_only.s_m,
        nominal_now.s_m
    );

    let mut s_m = nominal_now.s_m;
    for _ in 0..100 {
        sim.step();
        let Some(follow) = live_riders(&sim, 0)
            .iter()
            .find(|other| other.id == nominal_only.id)
            .copied()
        else {
            break;
        };
        assert!(
            follow.s_m >= s_m - 1e-9,
            "the refused participant never travels the opposing traversal: {s_m} -> {}",
            follow.s_m
        );
        s_m = follow.s_m;
    }
}

/// A disconnected opposing traversal is refused, and the refusal is total: the
/// rider keeps its nominal traversal and completes its nominal route with the
/// ordinary spawn-to-despawn lifecycle.
#[test]
fn a_disconnected_opposing_traversal_is_refused_and_the_rider_completes_its_nominal_route() {
    let disconnected = forward_rule_scenario(72.0).replace(A_BACK_TO_LEFT_ENTRY, "");
    let mut sim = build(&disconnected);
    let (rider, before) = lone_rider_on(&mut sim, 0, 20.0..=30.0);
    assert!(
        !sim.request_wrong_way_entry(rider),
        "a facility with no connected opposing traversal has no wrong-way entry"
    );

    let mut despawned = None;
    let mut s_m = before.s_m;
    for _ in 0..600 {
        let events: Vec<Event> = sim.step().events().to_vec();
        for event in &events {
            if let Event::Despawned { agent, reason, .. } = event
                && *agent == rider
            {
                despawned = Some(*reason);
            }
        }
        if let Some(after) = sim
            .snapshot(SnapshotDetail::Full)
            .agents()
            .iter()
            .find(|sample| sample.id == rider)
            .and_then(|sample| sample.motion.as_ref())
            .map(|motion| motion.path_distance_m)
        {
            assert!(
                after >= s_m - 1e-9,
                "the refused rider keeps its nominal traversal: {s_m} -> {after}"
            );
            s_m = after;
        }
        if despawned.is_some() {
            break;
        }
    }
    assert_eq!(
        despawned,
        Some(DespawnReason::ExitedPath),
        "the refused rider completes its nominal route"
    );
    assert!(
        s_m > before.s_m,
        "the rider travelled forward on the facility"
    );
}

/// No authored flag disables the collision, clearance, safety, or leader
/// queries.
///
/// The wrong-way policy carries the contract's three thresholds and nothing
/// else, and the version-2 document rejects an unknown key instead of ignoring
/// it, so a flag that would switch a query off cannot be authored at all. The
/// probes are the whole-token flag names in the exact place a wrong-way flag
/// would live.
#[test]
fn no_authored_flag_can_disable_a_query() {
    for flag in ["disable_collision: true,", "skip_leader_query: true,"] {
        let flagged = forward_rule_scenario(72.0).replace(
            "      urgency: 1.0 },",
            &format!("      urgency: 1.0,\n      {flag} }},"),
        );
        assert!(
            flagged.contains(flag),
            "the probe must reach the wrong-way policy: {flag}"
        );
        assert!(
            parse_scenario_source_v2(&flagged).is_err(),
            "a flag that would disable a query is rejected, not accepted: {flag}"
        );
    }
}

/// One `OpposingTraversal` boundary as a test reads it.
#[derive(Debug, Clone, PartialEq)]
struct IntervalBoundary {
    tick: u64,
    entering: bool,
    facility: usize,
    movement: Option<usize>,
    direction: MovementDirection,
    nominal_direction: NominalDirection,
    perceived_rule: Option<PermissionEffect>,
    reason: WrongWayReason,
    violating: bool,
}

/// Run `sim` for at most `steps`, returning `rider`'s opposing-traversal
/// boundaries in emission order, the tick of each of its facility handoffs, and
/// its despawn tick.
fn rider_boundaries(
    sim: &mut Simulation,
    rider: AgentId,
    steps: u64,
) -> (Vec<IntervalBoundary>, Vec<u64>, Option<u64>) {
    let mut boundaries = Vec::new();
    let mut handoffs = Vec::new();
    let mut despawned = None;
    for _ in 0..steps {
        let output = sim.step();
        let tick = output.time().tick();
        for event in output.events() {
            if event.agent() != rider {
                continue;
            }
            match event {
                Event::OpposingTraversal {
                    facility,
                    movement,
                    direction,
                    nominal_direction,
                    perceived_rule,
                    reason,
                    violating,
                    entering,
                    ..
                } => boundaries.push(IntervalBoundary {
                    tick,
                    entering: *entering,
                    facility: facility.index(),
                    movement: movement.map(hekate_model::MovementId::index),
                    direction: *direction,
                    nominal_direction: *nominal_direction,
                    perceived_rule: *perceived_rule,
                    reason: *reason,
                    violating: *violating,
                }),
                Event::FacilityTransition { .. } => handoffs.push(tick),
                Event::Despawned { .. } => despawned = Some(tick),
                _ => {}
            }
        }
        if despawned.is_some() {
            break;
        }
    }
    (boundaries, handoffs, despawned)
}

/// The interval boundaries of one turned rider in the forward-rule fixture.
fn turned_rider_boundaries() -> (Vec<IntervalBoundary>, Vec<u64>, Option<u64>) {
    let mut sim = build(&forward_rule_scenario(72.0));
    let (rider, _) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(sim.request_wrong_way_entry(rider));
    rider_boundaries(&mut sim, rider, 600)
}

/// A clear opposing entry records exactly one interval per object the rider
/// traverses, opened and closed at the contract's own boundaries: the interval
/// opens on the first step the rider's centre is inside the facility's extent
/// and its direction is against the rule, and it closes on the handoff that
/// takes it to a different object, then once more at its route exit. The record
/// carries the decision's perceived rule, reason code, affected facility and
/// movement, and the legality of the traversal.
#[test]
fn the_interval_opens_at_the_entry_and_closes_at_the_object_boundary() {
    let (boundaries, handoffs, despawned) = turned_rider_boundaries();

    assert_eq!(
        boundaries.len(),
        4,
        "two intervals, each with an open and a close boundary: {boundaries:?}"
    );
    let open = &boundaries[0];
    assert!(open.entering);
    assert_eq!(open.facility, 0);
    assert_eq!(open.direction, MovementDirection::Reverse);
    assert_eq!(open.nominal_direction, NominalDirection::Forward);
    assert_eq!(open.perceived_rule, None, "no statement binds the pair");
    assert_eq!(open.reason, WrongWayReason::NoncompliantChoice);
    assert!(open.violating, "no statement makes the traversal legal");
    assert_eq!(
        open.movement,
        Some(0),
        "the affected movement is the connector the rider entered on"
    );

    let close = &boundaries[1];
    assert!(!close.entering);
    assert_eq!(close.facility, 0, "the same object it opened on");
    assert_eq!(
        close.tick, handoffs[0],
        "the connector handoff is the close boundary"
    );
    assert!(open.tick < close.tick, "the interval spans at least a step");
    assert_eq!(
        close.reason, open.reason,
        "the record is the one it latched"
    );
    assert_eq!(close.violating, open.violating);

    let opened_again = &boundaries[2];
    assert!(opened_again.entering);
    assert_eq!(
        opened_again.facility, 1,
        "the destination object opens its own interval"
    );
    assert_eq!(
        opened_again.tick, close.tick,
        "the handoff is both boundaries"
    );

    let closed_at_exit = &boundaries[3];
    assert!(!closed_at_exit.entering);
    assert_eq!(closed_at_exit.facility, 1);
    assert_eq!(
        closed_at_exit.tick,
        despawned.expect("the rider despawns at the end of its route"),
        "the route exit is the final close boundary"
    );
}

/// A rejected decision creates no interval: the request is recorded and the
/// kernel evaluates the decision, but the rider never traverses the opposing
/// direction, so neither boundary is emitted and the rider completes its
/// nominal route.
#[test]
fn a_rejected_decision_creates_no_interval() {
    let refused = forward_rule_scenario(72.0).replace("urgency: 1.0", "urgency: 0.0");
    let mut sim = build(&refused);
    let (rider, _) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(
        sim.request_wrong_way_entry(rider),
        "the request itself is recorded"
    );

    let (boundaries, handoffs, despawned) = rider_boundaries(&mut sim, rider, 600);

    assert!(
        boundaries.is_empty(),
        "a rejection that selects the nominal option emits no boundary: {boundaries:?}"
    );
    assert_eq!(
        handoffs.len(),
        0,
        "the rejected rider never enters the opposing traversal that continues onto the connector"
    );
    assert!(
        despawned.is_some(),
        "the rider completes its nominal route and despawns"
    );
}

/// A scenario that authors neither the wrong-way policy nor the capability it
/// gates is inapplicable: the entry seam refuses the request, and no interval
/// boundary exists for any traversal, so the run's stream is the one a scenario
/// with none of the increment-2 wrong-way shapes produces.
#[test]
fn an_inapplicable_scenario_emits_no_opposing_traversal() {
    let inapplicable = forward_rule_scenario(72.0)
        .replace(", 'reverse_direction'", "")
        .replace(WRONG_WAY_POLICY, "");
    let mut sim = build(&inapplicable);
    let (rider, _) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(
        !sim.request_wrong_way_entry(rider),
        "no `maneuver_policy.wrong_way`, no entry"
    );

    let (boundaries, _, despawned) = rider_boundaries(&mut sim, rider, 600);

    assert!(boundaries.is_empty(), "{boundaries:?}");
    assert!(despawned.is_some(), "the rider completes its nominal route");
}

/// A `permit` statement makes the opposing traversal legal, so its interval is
/// recorded under that rule and never as a violation.
#[test]
fn a_permitted_opposing_traversal_is_recorded_legal() {
    let permitted = forward_rule_scenario(72.0).replace(
        "  movements: [",
        "  permissions: [ { id: 'contraflow_a', kind: 'nominal_direction', \
         holder: 'rider', target: 'a', effect: 'permit' } ],\n  movements: [",
    );
    let mut sim = build(&permitted);
    let (rider, _) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(sim.request_wrong_way_entry(rider));

    let (boundaries, _, _) = rider_boundaries(&mut sim, rider, 600);

    let open = boundaries.first().expect("the permitted entry opens one");
    assert_eq!(open.perceived_rule, Some(PermissionEffect::Permit));
    assert_eq!(open.reason, WrongWayReason::LegalPermission);
    assert!(
        !open.violating,
        "a permitted opposing traversal is a legal interval"
    );
}

/// The interval boundaries are a pure function of the run, so an identical
/// replay reproduces every one of them with the same tick, object, and record.
#[test]
fn the_interval_boundaries_survive_replay() {
    let first = turned_rider_boundaries();
    let second = turned_rider_boundaries();

    assert_eq!(first, second, "the same run reproduces the same boundaries");
    assert_eq!(first.0.len(), 4, "the replayed run reports both intervals");
}

/// The run ending is a close boundary: an interval still open on the last step
/// closes at the final simulation time, and the run-end closure emits no event
/// because no tick remains to carry one.
#[test]
fn an_interval_open_at_the_run_end_closes_at_the_final_simulation_time() {
    let mut sim = build(&forward_rule_scenario(72.0));
    let (rider, _) = lone_rider_on(&mut sim, 0, 5.0..=20.0);
    assert!(sim.request_wrong_way_entry(rider));

    let mut opened = 0;
    let mut last_tick = 0;
    for _ in 0..600 {
        let output = sim.step();
        last_tick = output.time().tick();
        opened += output
            .events()
            .iter()
            .filter(|event| matches!(event, Event::OpposingTraversal { entering: true, .. }))
            .count();
        if opened > 0 {
            break;
        }
    }
    assert_eq!(opened, 1, "the entry opens exactly one interval");
    assert!(
        sim.opposing_traversal_tracker().intervals().is_empty(),
        "the interval is still open"
    );

    sim.close_open_opposing_traversals();
    let intervals = sim.opposing_traversal_tracker().intervals();
    assert_eq!(intervals.len(), 1);
    assert_eq!(intervals[0].start_tick, last_tick);
    assert_eq!(
        intervals[0].end_tick, last_tick,
        "the close time is the run's final simulation time"
    );
    assert_eq!(intervals[0].duration_s(), 0.0);
}
