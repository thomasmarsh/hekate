//! Phase 1 Increment 4 slice C: typed safety events, their within-tick order,
//! and their once-per-transition lifecycle.
//!
//! The unit tests in `crate::safety` measure the observation pass directly on
//! constructed bodies. These tests measure the same machine through the public
//! kernel API on scenarios that exercise it, so a fixture cannot drift from the
//! contract it asserts:
//!
//! - equal-tick records follow the documented order: ascending `AgentId`, then
//!   event kind, then the variant's stable key, then its edge flag;
//! - every variant is edge-triggered, so a pair, an agent, or an agent-and-region
//!   alternates begin/end and never repeats a begin without an intervening end;
//! - the pair edges agree with an independent swept query built from the
//!   observed frames, so a reported edge is neither spurious nor missed;
//! - both modes emit through one stream, deterministically per seed, and the
//!   union covers all six new record families;
//! - violations reuse the recorded decision state and are emitted once per
//!   crossing action.

use std::collections::{BTreeMap, BTreeSet};

use tangle_model::{CompiledScenario, CrossingId, parse_scenario_source};
use tangle_sim::{
    AgentId, AgentMode, AgentSample, BodyShape, ComplianceReason, ControlTransitionKind,
    EVENT_VERSION, Event, EventKind, PedestrianComplianceReason, RegionKey, RunConfig,
    SignalAction, Simulation, SnapshotDetail, SweptBody, ViolationKind, band_entry,
    body_clearance_m, time_of_impact,
};

/// The checked-in mixed benchmark: both modes, one crossing, one `yield` rule.
const MIXED: &str = include_str!("../../../scenarios/benchmarks/mixed_interaction_v1.json5");
/// The checked-in perpendicular-conflict benchmark: an authored conflict region.
const CONFLICT: &str =
    include_str!("../../../scenarios/benchmarks/perpendicular_conflict_v1.json5");

fn scenario(text: &str) -> CompiledScenario {
    let source = parse_scenario_source(text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(text: &str, seed: u64) -> Simulation {
    Simulation::new(scenario(text), RunConfig::new(seed)).expect("simulation builds")
}

/// Every record of one run, with the tick it was attributed to.
fn collect(text: &str, seed: u64, ticks: u64) -> Vec<(u64, Event)> {
    let mut sim = sim(text, seed);
    let mut records = Vec::new();
    for _ in 0..ticks {
        let tick = sim.time().tick();
        for event in sim.step().events() {
            records.push((tick, event.clone()));
        }
    }
    records
}

/// Assert a per-key sequence alternates begin/end, starting at a begin.
fn assert_alternates<T: Ord + std::fmt::Debug>(records: &[(T, bool, u64)], what: &str) {
    let mut state: BTreeMap<&T, bool> = BTreeMap::new();
    for (key, flag, tick) in records {
        let was = state.insert(key, *flag).unwrap_or(false);
        assert_ne!(
            was, *flag,
            "{what} {key:?} repeated a state at tick {tick} (flag {flag})"
        );
    }
}

/// The body shape an observed sample describes.
fn body_of(sample: &AgentSample) -> BodyShape {
    let motion = sample.motion.as_ref().expect("full detail");
    match motion.mode {
        AgentMode::Pedestrian => BodyShape::Circle {
            centre: sample.position,
            radius_m: motion.body_length_m * 0.5,
        },
        AgentMode::Vehicle => BodyShape::Box {
            centre: sample.position,
            heading_rad: sample.heading_rad,
            length_m: motion.body_length_m,
            width_m: motion.body_width_m,
        },
    }
}

/// The swept body of one agent over one observed step.
fn swept_of(before: &AgentSample, after: &AgentSample) -> SweptBody {
    SweptBody {
        shape: body_of(before),
        displacement_m: after.position - before.position,
    }
}

/// One observed step: the events it produced, with the frames it moved between.
struct Step {
    tick: u64,
    events: Vec<Event>,
    before: BTreeMap<AgentId, AgentSample>,
    after: BTreeMap<AgentId, AgentSample>,
}

fn observe(text: &str, seed: u64, ticks: u64) -> Vec<Step> {
    let mut sim = sim(text, seed);
    let mut steps = Vec::new();
    for _ in 0..ticks {
        let tick = sim.time().tick();
        let before: BTreeMap<AgentId, AgentSample> = sim
            .snapshot(SnapshotDetail::Full)
            .agents()
            .iter()
            .map(|sample| (sample.id, sample.clone()))
            .collect();
        let events: Vec<Event> = sim.step().events().to_vec();
        let after: BTreeMap<AgentId, AgentSample> = sim
            .snapshot(SnapshotDetail::Full)
            .agents()
            .iter()
            .map(|sample| (sample.id, sample.clone()))
            .collect();
        steps.push(Step {
            tick,
            events,
            before,
            after,
        });
    }
    steps
}

/// The signalized approach of the checked-in compliance contract, with a red
/// hold followed by green so a stop can begin and end.
fn signal_scenario(compliance: f64) -> String {
    format!(
        r#"{{
          schema_version: 1,
          id: 'safety_signal',
          coordinate_system: {{ x: 'east_m', y: 'north_m' }},
          paths: [ {{ id: 'guide', points: [ {{ x: -40.0, y: 0.0 }}, {{ x: 40.0, y: 0.0 }} ] }} ],
          portals: [
            {{ id: 'entry', path: 'guide', end: 'start', width_m: 3.5 }},
            {{ id: 'exit', path: 'guide', end: 'end', width_m: 3.5 }},
          ],
          movements: [
            {{ id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0, stop_line_m: 34.0 }},
          ],
          rules: [ {{ id: 'r_through', movement: 'through', kind: 'signal', signal: 'main' }} ],
          signals: [ {{ id: 'main',
            heads: [ {{ id: 'head', movement: 'through' }} ],
            phases: [
              {{ duration_s: 30.0, states: [ {{ head: 'head', color: 'red' }} ] }},
              {{ duration_s: 30.0, states: [ {{ head: 'head', color: 'green' }} ] }},
            ] }} ],
          demand: [
            {{ id: 'inflow', portal: 'entry', rate_vph: 600.0,
              routes: [ {{ movement: 'through', weight: 1.0 }} ] }},
          ],
          profiles: {{
            speed_mps: {{ min: 10.0, max: 10.0 }},
            length_m: {{ min: 4.0, max: 4.0 }},
            width_m: {{ min: 2.0, max: 2.0 }},
            time_gap_s: {{ min: 1.5, max: 1.5 }},
            max_accel_mps2: {{ min: 2.0, max: 2.0 }},
            comfortable_brake_mps2: {{ min: 3.0, max: 3.0 }},
            compliance: {{ min: {compliance}, max: {compliance} }},
          }},
        }}"#
    )
}

/// One walking path over a signal-controlled crossing whose first 20 s forbid
/// crossing and next 20 s allow it.
fn pedestrian_crossing_scenario(compliance: f64) -> String {
    format!(
        r#"{{
  schema_version: 1,
  id: 'safety_pedestrian_crossing',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [
    {{ id: 'road', points: [ {{ x: -40.0, y: 0.0 }}, {{ x: 40.0, y: 0.0 }} ] }},
    {{ id: 'walk', points: [ {{ x: 0.0, y: -20.0 }}, {{ x: 0.0, y: 20.0 }} ] }},
  ],
  portals: [
    {{ id: 'west', path: 'road', end: 'start', width_m: 7.0 }},
    {{ id: 'east', path: 'road', end: 'end', width_m: 7.0 }},
    {{ id: 'south', path: 'walk', end: 'start', width_m: 3.0 }},
    {{ id: 'north', path: 'walk', end: 'end', width_m: 3.0 }},
  ],
  regions: [
    {{ id: 'crossing_zone', points: [
      {{ x: -3.0, y: -3.0 }}, {{ x: 3.0, y: -3.0 }}, {{ x: 3.0, y: 3.0 }}, {{ x: -3.0, y: 3.0 }}
    ] }},
  ],
  movements: [ {{ id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 }} ],
  crossings: [ {{ id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ],
    pedestrian_signal: {{ phases: [
      {{ duration_s: 20.0, walk: false }},
      {{ duration_s: 20.0, walk: true }},
    ] }} }} ],
  pedestrian_routes: [ {{ id: 'cross_to_north', from: 'south', to: 'north', path: 'walk',
    crossings: [ 'road_crossing' ] }} ],
  pedestrian_demand: [ {{ id: 'footfall', portal: 'south', rate_pph: 720.0,
    routes: [ {{ route: 'cross_to_north', weight: 1.0 }} ] }} ],
  pedestrian_profiles: {{
    radius_m: {{ min: 0.25, max: 0.25 }},
    speed_mps: {{ min: 1.25, max: 1.25 }},
    compliance: {{ min: {compliance}, max: {compliance} }},
  }},
}}"#
    )
}

/// The documented tie-breakers, on hand-built records that share one tick: an
/// ascending agent wins first, then the kind order, then the variant's stable
/// key, then the edge flag.
#[test]
fn the_documented_tie_breakers_order_equal_tick_records() {
    let entries: [Event; 8] = [
        Event::Queue {
            agent: AgentId::from_index(1),
            joined: true,
        },
        Event::Collision {
            agent: AgentId::from_index(0),
            other: AgentId::from_index(2),
            clearance_m: 0.0,
            contacting: true,
        },
        Event::Spawned {
            agent: AgentId::from_index(1),
            mode: AgentMode::Vehicle,
            path: tangle_model::PathId::from_index(3),
            distance_m: 0.0,
        },
        Event::Entry {
            agent: AgentId::from_index(0),
            region: RegionKey::Crossing(CrossingId::from_index(1)),
        },
        Event::Entry {
            agent: AgentId::from_index(0),
            region: RegionKey::ConflictRegion(tangle_model::ConflictRegionId::from_index(0)),
        },
        Event::Exit {
            agent: AgentId::from_index(0),
            region: RegionKey::Crossing(CrossingId::from_index(1)),
        },
        Event::Yielded {
            agent: AgentId::from_index(1),
            crossing: CrossingId::from_index(0),
            yielding: false,
        },
        Event::Yielded {
            agent: AgentId::from_index(1),
            crossing: CrossingId::from_index(0),
            yielding: true,
        },
    ];
    let mut ordered = entries;
    ordered.sort_by_key(|event| event.order_key());
    let kinds: Vec<EventKind> = ordered.iter().map(|event| event.kind()).collect();
    assert_eq!(
        kinds,
        vec![
            // Ascending agent first: agent 0's records before agent 1's.
            EventKind::Collision,
            EventKind::Entry,
            EventKind::Entry,
            EventKind::Exit,
            // Then the kind order, then the key, then the edge flag.
            EventKind::Spawned,
            EventKind::Yielded,
            EventKind::Yielded,
            EventKind::Queue,
        ]
    );
    // Same agent, same kind, same key: the edge flag orders the two records.
    assert_eq!(ordered[5].order_key().4, 0);
    assert_eq!(ordered[6].order_key().4, 1);
    // The two region keys of the same agent and kind are ordered by region tag
    // first, so a crossing key cannot tie with a conflict-region key.
    assert_eq!(ordered[1].order_key().2, 0);
    assert_eq!(ordered[2].order_key().2, 1);
    assert_eq!(EVENT_VERSION, 3);
}

#[test]
fn the_mixed_benchmark_stream_is_ordered_deterministic_and_both_modes() {
    let first = collect(MIXED, 0, 4000);
    let again = collect(MIXED, 0, 4000);
    assert_eq!(first, again, "the same seed must reproduce the same stream");

    let mut ticks: BTreeMap<u64, Vec<Event>> = BTreeMap::new();
    let mut modes: BTreeMap<AgentId, AgentMode> = BTreeMap::new();
    let mut kinds: BTreeMap<EventKind, (u64, u64)> = BTreeMap::new();
    for (tick, event) in &first {
        ticks.entry(*tick).or_default().push(event.clone());
        // A spawned record carries the mode (F5), so the stream alone says which
        // mode each record is about.
        if let Event::Spawned { agent, mode, .. } = event {
            modes.insert(*agent, *mode);
        }
        let mode = modes
            .get(&event.agent())
            .expect("every recorded agent was announced by a spawned record");
        let counts = kinds.entry(event.kind()).or_insert((0, 0));
        match mode {
            AgentMode::Vehicle => counts.0 += 1,
            AgentMode::Pedestrian => counts.1 += 1,
        }
    }
    // Every tick's records follow the documented order.
    let mut multi_kind_ticks = 0;
    for (tick, records) in &ticks {
        assert!(
            records
                .windows(2)
                .all(|pair| pair[0].order_key() <= pair[1].order_key()),
            "tick {tick} is out of order: {records:?}"
        );
        let distinct: BTreeSet<EventKind> = records.iter().map(|event| event.kind()).collect();
        if distinct.len() > 1 {
            multi_kind_ticks += 1;
        }
    }
    assert!(
        multi_kind_ticks > 0,
        "the fixture must exercise ties between kinds within a tick"
    );

    // The union covers every new family the benchmark can produce.
    for kind in [
        EventKind::Collision,
        EventKind::NearMiss,
        EventKind::Entry,
        EventKind::Exit,
        EventKind::Queue,
        EventKind::ControlTransition,
    ] {
        assert!(
            kinds.contains_key(&kind),
            "the mixed benchmark never produced a {kind:?} record; saw {:?}",
            kinds.keys().collect::<Vec<_>>()
        );
    }
    // Both modes emit safety records through the one stream, and both reach the
    // region and queue families the fixture drives for cars and pedestrians.
    let (vehicles, pedestrians) = kinds
        .iter()
        .filter(|(kind, _)| !matches!(kind, EventKind::Spawned | EventKind::Despawned))
        .fold((0u64, 0u64), |(v, p), (_, (vehicle, pedestrian))| {
            (v + vehicle, p + pedestrian)
        });
    assert!(vehicles > 0, "no safety record was about a vehicle");
    assert!(pedestrians > 0, "no safety record was about a pedestrian");
    for kind in [EventKind::Entry, EventKind::Exit, EventKind::Queue] {
        let (vehicle, pedestrian) = kinds[&kind];
        assert!(
            vehicle > 0 && pedestrian > 0,
            "{kind:?} must come from both modes, got {vehicle} vehicle and {pedestrian} pedestrian"
        );
    }
    println!(
        "mixed_interaction_v1 seed 0 records per kind and mode (vehicle, pedestrian): {kinds:?}"
    );

    let other_seed = collect(MIXED, 1, 4000);
    assert_ne!(first, other_seed, "different seeds must diverge");
}

/// The pair edges must agree with an independent swept query built from the
/// observed frames: a contact begin is a contact inside the tick, a contact end
/// is no contact at all, and the near-miss band agrees in the same way. The
/// query is the public slice-B cast, so a reported edge that the geometry does
/// not support is a false positive and an unsupported gap is a missed
/// transition.
#[test]
fn pair_records_match_an_independent_swept_query() {
    let steps = observe(MIXED, 0, 4000);
    let mut contact: BTreeMap<(u32, u32), Vec<(bool, u64)>> = BTreeMap::new();
    let mut near_miss: BTreeMap<(u32, u32), Vec<(bool, u64)>> = BTreeMap::new();
    let mut contact_begins: BTreeSet<(u32, u32, u64)> = BTreeSet::new();
    let mut near_miss_begins: BTreeSet<(u32, u32, u64)> = BTreeSet::new();
    for step in &steps {
        for event in &step.events {
            let (first, second, contacting, entering) = match event {
                Event::Collision {
                    agent,
                    other,
                    contacting,
                    ..
                } => (*agent, *other, Some(*contacting), None),
                Event::NearMiss {
                    agent,
                    other,
                    entering,
                    ..
                } => (*agent, *other, None, Some(*entering)),
                _ => continue,
            };
            let (Some(before_first), Some(before_second), Some(after_first), Some(after_second)) = (
                step.before.get(&first),
                step.before.get(&second),
                step.after.get(&first),
                step.after.get(&second),
            ) else {
                panic!("a pair record must name two live bodies");
            };
            let first_body = swept_of(before_first, after_first);
            let second_body = swept_of(before_second, after_second);
            let end_clearance_m =
                body_clearance_m(&first_body.end_shape(), &second_body.end_shape());
            let contact_this_tick =
                end_clearance_m <= 0.0 || time_of_impact(&first_body, &second_body).is_some();
            let near_this_tick = end_clearance_m <= tangle_sim::NEAR_MISS_THRESHOLD_M
                || band_entry(&first_body, &second_body, tangle_sim::NEAR_MISS_THRESHOLD_M)
                    .is_some();
            let key = (first.get(), second.get());
            if let Some(contacting) = contacting {
                contact
                    .entry(key)
                    .or_default()
                    .push((contacting, step.tick));
                if contacting {
                    contact_begins.insert((key.0, key.1, step.tick));
                }
                assert_eq!(
                    contact_this_tick, contacting,
                    "pair {key:?} reported contact {contacting} at tick {} but the swept \
                     query says {contact_this_tick}",
                    step.tick
                );
            } else if let Some(entering) = entering {
                // The documented near-miss predicate is the band less contact, so
                // the same expression decides both edges. Contact owns its own
                // edge, so a pair never begins both in one tick.
                assert_eq!(
                    near_this_tick && !contact_this_tick,
                    entering,
                    "pair {key:?} reported near-miss {entering} at tick {} but the band \
                     predicate says {}",
                    step.tick,
                    near_this_tick && !contact_this_tick
                );
                if entering {
                    near_miss_begins.insert((key.0, key.1, step.tick));
                }
                near_miss
                    .entry(key)
                    .or_default()
                    .push((entering, step.tick));
            }
        }
    }
    // The two families are nested per-tick predicates, so one tick can never
    // begin both for one pair: a contact is also inside the near-miss band with
    // the band's own edge suppressed, so contact owns the begin. This structural
    // check reads only the emitted stream, independent of the order the records
    // arrived in, so it holds even if the per-tick geometry recomputation above
    // were wrong.
    let both_families: Vec<(u32, u32, u64)> = contact_begins
        .intersection(&near_miss_begins)
        .copied()
        .collect();
    assert!(
        both_families.is_empty(),
        "pair(s) began a contact and a near miss in the same tick: {both_families:?}"
    );
    assert!(
        !contact.is_empty(),
        "the fixture must contain at least one body contact"
    );
    assert!(
        !near_miss.is_empty(),
        "the fixture must contain at least one sub-threshold separation"
    );
    for (pair, events) in contact.iter().chain(near_miss.iter()) {
        assert_alternates(
            &events
                .iter()
                .map(|(flag, tick)| (*pair, *flag, *tick))
                .collect::<Vec<_>>(),
            "pair record",
        );
    }
    println!(
        "mixed_interaction_v1 seed 0 pair records: {} contact pairs, {} near-miss pairs",
        contact.len(),
        near_miss.len()
    );
}

#[test]
fn region_edges_alternate_and_cover_both_region_kinds() {
    let mixed = collect(MIXED, 0, 4000);
    let conflict = collect(CONFLICT, 0, 2000);
    let mut occupied: BTreeMap<(u32, RegionKey), Vec<(bool, u64)>> = BTreeMap::new();
    let mut saw_crossing = false;
    let mut saw_conflict_region = false;
    for (tick, event) in mixed.iter().chain(conflict.iter()) {
        let (agent, region, entering) = match event {
            Event::Entry { agent, region } => (*agent, *region, true),
            Event::Exit { agent, region } => (*agent, *region, false),
            _ => continue,
        };
        match region {
            RegionKey::Crossing(_) => saw_crossing = true,
            RegionKey::ConflictRegion(_) => saw_conflict_region = true,
        }
        occupied
            .entry((agent.get(), region))
            .or_default()
            .push((entering, *tick));
    }
    assert!(
        saw_crossing,
        "the mixed benchmark crosses a crossing region"
    );
    assert!(
        saw_conflict_region,
        "the conflict benchmark crosses an authored conflict region"
    );
    assert!(!occupied.is_empty());
    for (key, events) in &occupied {
        assert_alternates(
            &events
                .iter()
                .map(|(flag, tick)| (*key, *flag, *tick))
                .collect::<Vec<_>>(),
            "region transit",
        );
    }
}

#[test]
fn queue_and_control_edges_alternate_per_agent() {
    let records = collect(MIXED, 0, 4000);
    let mut modes: BTreeMap<u32, AgentMode> = BTreeMap::new();
    let mut queue: BTreeMap<u32, Vec<(bool, u64)>> = BTreeMap::new();
    let mut control: BTreeMap<u32, Vec<(bool, u64)>> = BTreeMap::new();
    let mut control_kinds: BTreeMap<u32, ControlTransitionKind> = BTreeMap::new();
    for (tick, event) in &records {
        if let Event::Spawned { agent, mode, .. } = event {
            modes.insert(agent.get(), *mode);
        }
        match event {
            Event::Queue { agent, joined } => {
                queue.entry(agent.get()).or_default().push((*joined, *tick));
            }
            Event::ControlTransition {
                agent,
                control: kind,
                active,
            } => {
                control
                    .entry(agent.get())
                    .or_default()
                    .push((*active, *tick));
                if let Some(previous) = control_kinds.insert(agent.get(), *kind) {
                    assert_eq!(previous, *kind, "an agent keeps one control state");
                }
            }
            _ => {}
        }
    }
    assert!(!queue.is_empty() && !control.is_empty());
    for (agent, events) in queue.iter().chain(control.iter()) {
        assert_alternates(
            &events
                .iter()
                .map(|(flag, tick)| (*agent, *flag, *tick))
                .collect::<Vec<_>>(),
            "state",
        );
    }
    // Both modes reach the stopped-and-waiting state, and the control-transition
    // family is the pedestrian crossing wait here because no movement in this
    // fixture carries a signal rule. The vehicle-side control transition is the
    // stop at a signal-controlled line, which the signal fixture below covers.
    let queue_modes: Vec<AgentMode> = queue
        .keys()
        .filter_map(|agent| modes.get(agent).copied())
        .collect();
    assert!(
        queue_modes.contains(&AgentMode::Vehicle) && queue_modes.contains(&AgentMode::Pedestrian),
        "both modes must reach the stopped-and-waiting state, got {queue_modes:?}"
    );
    assert!(
        control_kinds
            .values()
            .all(|kind| *kind == ControlTransitionKind::CrossingWait),
        "the crossing wait is the only control state this fixture can produce"
    );
    assert!(!control_kinds.is_empty());
    println!(
        "mixed_interaction_v1 seed 0 state records: {} queueing agents, {} control agents",
        queue.len(),
        control.len()
    );
}

/// A vehicle records one violation per crossing action, from the decision the
/// kernel already stores: a proceeding decision on a forbidding head whose gap
/// was still positive.
#[test]
fn a_vehicle_records_one_red_light_violation_per_crossing() {
    for compliance in [0.0, 1.0] {
        let text = signal_scenario(compliance);
        let mut sim = sim(&text, 2);
        let mut violations: BTreeMap<u32, u64> = BTreeMap::new();
        let mut control: BTreeMap<u32, Vec<(bool, u64)>> = BTreeMap::new();
        let mut saw_noncompliant_record = false;
        for _ in 0..1400 {
            let tick = sim.time().tick();
            // The decision recorded before the step is the one that governed it.
            let before: BTreeMap<AgentId, (ComplianceReason, SignalAction, f64)> = sim
                .snapshot(SnapshotDetail::Full)
                .agents()
                .iter()
                .filter_map(|sample| {
                    sim.agent_decision(sample.id).map(|decision| {
                        (
                            sample.id,
                            (decision.reason, decision.action, decision.stop_line_gap_m),
                        )
                    })
                })
                .collect();
            for (reason, _, _) in before.values() {
                saw_noncompliant_record |= *reason == ComplianceReason::NonCompliantRun;
            }
            for event in sim.step().events() {
                match event {
                    Event::Violation { agent, kind } => {
                        assert_eq!(*kind, ViolationKind::RanRedLight);
                        let (reason, action, gap_m) = before[agent];
                        assert_eq!(
                            action,
                            SignalAction::Proceed,
                            "agent {} recorded a violation from a non-proceeding decision",
                            agent.get()
                        );
                        assert_ne!(reason, ComplianceReason::Green);
                        assert!(gap_m > 0.0, "the front bumper was still upstream");
                        // The recorded reason says whether the breach was the
                        // noncompliant choice or an entry the body could not
                        // brake out of. A fully compliant driver can only reach
                        // the second, because a comfortable stop is always
                        // within its willingness.
                        assert!(
                            matches!(
                                reason,
                                ComplianceReason::NonCompliantRun | ComplianceReason::CannotStop
                            ),
                            "agent {} proceeded on {} with reason {reason:?}",
                            agent.get(),
                            compliance
                        );
                        if compliance == 1.0 {
                            assert_eq!(reason, ComplianceReason::CannotStop);
                        }
                        *violations.entry(agent.get()).or_insert(0) += 1;
                    }
                    Event::ControlTransition {
                        agent,
                        control: kind,
                        active,
                    } => {
                        assert_eq!(*kind, ControlTransitionKind::SignalStop);
                        control
                            .entry(agent.get())
                            .or_default()
                            .push((*active, tick));
                    }
                    _ => {}
                }
            }
        }
        for (agent, count) in &violations {
            assert_eq!(*count, 1, "agent {agent} ran the head more than once");
        }
        if compliance == 0.0 {
            assert!(
                !violations.is_empty(),
                "a noncompliant driver must be reported for running the head"
            );
            // The decision record carries the willful choice: while it is still
            // upstream the zero-compliance driver records a noncompliant run,
            // and the crossing tick's own reason is the entry it could no longer
            // brake out of.
            assert!(
                saw_noncompliant_record,
                "a zero-compliance driver must record a noncompliant run"
            );
        } else {
            // A fully compliant driver never records a noncompliant choice.
            assert!(
                !saw_noncompliant_record,
                "a fully compliant driver cannot choose to run the head"
            );
            // It stops at the line instead: the stop is a control transition
            // that begins and ends, reported once each.
            for (agent, events) in &control {
                assert_alternates(
                    &events
                        .iter()
                        .map(|(flag, tick)| (*agent, *flag, *tick))
                        .collect::<Vec<_>>(),
                    "signal stop",
                );
            }
            assert!(
                control.values().flatten().any(|(active, _)| *active),
                "a compliant driver must report a stop transition"
            );
        }
        println!(
            "signal compliance {compliance}: {} violating agents, {} stop-transition agents",
            violations.len(),
            control.len()
        );
    }
}

/// A pedestrian records one violation per entry it took against a forbidding
/// signal, and a compliant pedestrian records none.
#[test]
fn a_pedestrian_records_one_crossing_against_the_signal_violation() {
    for compliance in [1.0, 0.0] {
        let text = pedestrian_crossing_scenario(compliance);
        let mut sim = sim(&text, 3);
        let mut violations: BTreeMap<u32, u64> = BTreeMap::new();
        for _ in 0..3000 {
            let before: BTreeMap<AgentId, (PedestrianComplianceReason, f64)> = sim
                .snapshot(SnapshotDetail::Full)
                .agents()
                .iter()
                .filter_map(|sample| {
                    sim.agent_pedestrian_decision(sample.id)
                        .map(|decision| (sample.id, (decision.reason, decision.crossing_gap_m)))
                })
                .collect();
            for event in sim.step().events() {
                let Event::Violation { agent, kind } = event else {
                    continue;
                };
                assert_eq!(*kind, ViolationKind::CrossedAgainstSignal);
                let (reason, gap_m) = before[agent];
                assert_eq!(reason, PedestrianComplianceReason::NonCompliantCross);
                assert!(gap_m > 0.0, "the pedestrian was still upstream");
                *violations.entry(agent.get()).or_insert(0) += 1;
            }
        }
        for (agent, count) in &violations {
            assert_eq!(*count, 1, "pedestrian {agent} violated more than once");
        }
        if compliance == 1.0 {
            assert!(
                violations.is_empty(),
                "a compliant pedestrian never crosses against the signal: {violations:?}"
            );
        } else {
            assert!(
                !violations.is_empty(),
                "a noncompliant pedestrian must be reported"
            );
        }
        println!(
            "pedestrian compliance {compliance}: {} violating pedestrians",
            violations.len()
        );
    }
}
