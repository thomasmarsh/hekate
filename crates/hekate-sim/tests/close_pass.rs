//! TAS-121: detect executed overtakes and accumulate clearance-band durations.
//!
//! A faster motor mode must overtake a slower narrow mode on one shared
//! continuous-width facility. The tracker owns no tactic and emits no event: it
//! reads the ordinary world-body state and the compiled clearance bands and
//! reports, per completed overtaking interval, how long the pair's exact
//! clearance sat inside each band. These tests drive the public
//! [`Simulation::close_pass_tracker`] surface over a scenario that authors
//! bands, so detection and band accumulation are exercised end to end.

use std::collections::BTreeMap;

use hekate_model::{BodyKind, ClearanceBandId, CompiledScenario, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, Event, EventKind, OvertakeObservation, RunConfig, Simulation, SnapshotDetail,
};

/// The default 0.05 s step, so one tick is a twentieth of a second.
const DT: f64 = 0.05;

/// `900` ticks is 45 s: long enough for demand cycles, a catch-up, and a
/// completed overtake on the fixture below.
const TICKS: u64 = 900;

/// The three declared bands, in declaration order, so a report keeps the stable
/// ids and the order.
const BANDS: [ClearanceBandId; 3] = [
    ClearanceBandId::from_index(0),
    ClearanceBandId::from_index(1),
    ClearanceBandId::from_index(2),
];

/// One version-2 continuous-width road with a motor mode that may overtake and
/// a slower narrow mode, one demand source each so a faster motor enters behind
/// a slower narrow user and passes it.
///
/// The fixture authors three clearance bands: two that name the narrow mode
/// (`bicycle`) and one that applies to every pair, so the mode gate is exercised
/// cross-sectionally. `motor_speed_mps` and `bicycle_speed_mps` are parameters
/// so a no-overtake run can hold them equal.
fn passing_scenario(motor_speed_mps: f64, bicycle_speed_mps: f64) -> CompiledScenario {
    let source = parse_scenario_source_v2(&format!(
        r#"{{
  schema_version: 2,
  id: 'close_pass_integration',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [ {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 500.0, y: 0.0 }} ] }} ],
  portals: [
    {{ id: 'entry', path: 'guide', end: 'start', width_m: 10.0 }},
    {{ id: 'exit', path: 'guide', end: 'end', width_m: 10.0 }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -20.0, y: -20.0 }}, {{ x: 520.0, y: -20.0 }},
      {{ x: 520.0, y: 20.0 }}, {{ x: -20.0, y: 20.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band', points: [
      {{ x: 0.0, y: -5.0 }}, {{ x: 500.0, y: -5.0 }},
      {{ x: 500.0, y: 5.0 }}, {{ x: 0.0, y: 5.0 }},
    ] }},
  ],
  facilities: [
    {{ id: 'road', region: 'band', reference_path: 'guide',
      width_m: 10.0, nominal_direction: 'forward',
      access: {{ modes: [ 'passenger_car', 'bicycle' ] }}, lateral_use: 'shared',
      lateral_policy: {{ passing_side: 'left' }},
      speed_policy: {{ limit_mps: null }} }},
  ],
  movements: [
    {{ id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
      direction: 'forward' }},
  ],
  mode_templates: [
    {{
      id: 'passenger_car',
      body: {{ kind: 'box', length_m: {{ min: 4.5, max: 4.5 }},
        width_m: {{ min: 1.8, max: 1.8 }} }},
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'overtake' ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: {motor_speed_mps}, max: {motor_speed_mps} }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_accel_max_mps2: {{ min: 2.0, max: 2.0 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
      lateral: {{ target_clearance_m: 0.75, horizon_s: 2.0 }},
    }},
    {{
      id: 'bicycle',
      body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }},
        radius_m: {{ min: 0.35, max: 0.35 }} }},
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield' ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: {bicycle_speed_mps}, max: {bicycle_speed_mps} }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
    }},
  ],
  clearance_bands: [
    {{ id: 'bicycle_close', threshold_m: 0.75, violation: true,
      applies_to_modes: [ 'bicycle' ] }},
    {{ id: 'bicycle_study', threshold_m: 1.5, violation: false,
      applies_to_modes: [ 'bicycle' ] }},
    {{ id: 'all_study', threshold_m: 3.0, violation: false }},
  ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'bicycle_inflow', mode: 'bicycle',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'car_inflow', mode: 'passenger_car',
      spawn: {{ rate: {{
        portal: 'entry',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}"#
    ))
    .expect("the document is version 2");
    CompiledScenario::compile_v2(source).expect("the scenario compiles")
}

/// Every piece of evidence one run of the passing fixture records: the
/// completed overtaking intervals, the body kind the Full frame observed for
/// every agent, and every `ClosePass` event with the tick that emitted it.
struct Run {
    /// The completed overtaking intervals the run closed.
    overtakes: Vec<OvertakeObservation>,
    /// The body kind of every agent the run observed, so a narrow (capsule)
    /// body is distinguishable from a motor (box) body.
    kinds: BTreeMap<AgentId, BodyKind>,
    /// Every `ClosePass` event the run emitted, with its emitting tick.
    events: Vec<(u64, Event)>,
}

/// Run the simulation once, returning every piece of evidence the assertions
/// read: the completed overtaking intervals, the observed body kinds, and the
/// `ClosePass` events with the tick that emitted each.
fn run(scenario: CompiledScenario, seed: u64) -> Run {
    let mut sim = Simulation::new(scenario, RunConfig::new(seed)).expect("the simulation builds");
    let mut kinds: BTreeMap<AgentId, BodyKind> = BTreeMap::new();
    let mut events: Vec<(u64, Event)> = Vec::new();
    for _ in 0..TICKS {
        let output = sim.step();
        let tick = output.time().tick();
        for event in output.events() {
            if event.kind() == EventKind::ClosePass {
                events.push((tick, event.clone()));
            }
        }
        let frame = sim.snapshot(SnapshotDetail::Full);
        for sample in frame.agents() {
            if let Some(motion) = &sample.motion {
                kinds.insert(sample.id, motion.body_kind);
            }
        }
    }
    Run {
        overtakes: sim.close_pass_tracker().overtakes().to_vec(),
        kinds,
        events,
    }
}

/// Every observation's bands are a declaration-ordered subsequence of the
/// declared bands, with non-decreasing durations because the thresholds nest.
fn assert_declaration_order_and_monotone(observation: &OvertakeObservation) {
    let ids: Vec<u32> = observation
        .bands
        .iter()
        .map(|band| band.band.get())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "bands are reported in declaration order");
    assert!(
        observation
            .bands
            .windows(2)
            .all(|pair| pair[0].duration_s <= pair[1].duration_s),
        "nested thresholds accumulate monotonically: {:?}",
        observation.bands
    );
    for band in &observation.bands {
        assert!(
            band.duration_s >= 0.0
                && band.duration_s
                    <= (observation.end_tick - observation.start_tick + 1) as f64 * DT + 1e-9,
            "a band duration lies within the interval: {band:?}"
        );
    }
}

/// One 900-tick run of the passing fixture serves every assertion the four
/// separate cases made: the detected overtaking intervals with their bands and
/// observed body kinds, the `ClosePass` events with their payload and the
/// one-event-per-observation count, and — from a second, prefix run that stops
/// at an existing interval's `end_tick` — the run-end closure of the interval
/// still alongside. Nextest runs one process per test, so the shared run has to
/// live in one test rather than in a shared fixture.
#[test]
fn a_passing_run_reports_its_bands_events_and_run_end_closure() {
    let evidence = run(passing_scenario(9.0, 4.0), 7);

    assert_a_car_overtaking_a_bicycle_accumulates_every_band(&evidence.overtakes, &evidence.kinds);
    assert_a_completed_pass_emits_one_close_pass_event_with_its_evidence(&evidence.events);
    // Every closed observation becomes exactly one event: the kernel emits one
    // `ClosePass` per observation it closes, so a pair is never counted twice
    // and no completed pass is lost.
    assert_eq!(
        evidence.events.len(),
        evidence.overtakes.len(),
        "one event per closed observation"
    );
    assert_the_interval_still_open_at_the_final_tick_closes_with_the_completed_evidence(
        &evidence.overtakes,
    );
}

/// A motor mode overtaking a slower narrow mode is detected, and each declared
/// band accumulates its own duration with its stable id retained.
fn assert_a_car_overtaking_a_bicycle_accumulates_every_band(
    observed: &[OvertakeObservation],
    kinds: &BTreeMap<AgentId, BodyKind>,
) {
    assert!(
        !observed.is_empty(),
        "the faster motor must overtake the slower narrow user at least once"
    );

    // A motor (box) overtaking a narrow (capsule) user is the overtake of
    // interest; two bodies of one mode at one speed never pass each other.
    let mut saw_narrow_pass = false;
    for observation in observed {
        let agent_kind = kinds
            .get(&observation.agent)
            .copied()
            .expect("the passing agent was observed");
        let partner_kind = kinds
            .get(&observation.partner)
            .copied()
            .expect("the passed body was observed");
        assert_eq!(
            agent_kind,
            BodyKind::Box,
            "the passing agent is a motor box"
        );
        assert!(
            observation.start_tick <= observation.end_tick,
            "the interval is well formed"
        );
        assert_declaration_order_and_monotone(observation);
        let partner_is_narrow = partner_kind == BodyKind::Capsule;
        if partner_is_narrow {
            saw_narrow_pass = true;
        }
        // A band naming `bicycle` applies exactly when the pair includes one.
        let names_bicycle = observation
            .bands
            .iter()
            .any(|band| band.band == BANDS[0] || band.band == BANDS[1]);
        assert_eq!(
            names_bicycle, partner_is_narrow,
            "the mode gate matches a narrow partner: {observation:?}"
        );
        // The widest band applies to every pair and is crossed by an actual
        // pass on this shared road.
        let all_study = observation
            .bands
            .iter()
            .find(|band| band.band == BANDS[2])
            .expect("the every-pair band always applies");
        assert!(
            all_study.duration_s > 0.0,
            "the widest band is crossed by a real pass: {observation:?}"
        );
    }
    assert!(
        saw_narrow_pass,
        "at least one pass was over the narrow mode"
    );
}

/// Two runs of the same scenario and seed report exactly the same overtaking
/// intervals and band durations: detection is a pure function of the states the
/// ticks produced.
#[test]
fn detection_is_deterministic_across_runs() {
    let first = run(passing_scenario(9.0, 4.0), 7).overtakes;
    let second = run(passing_scenario(9.0, 4.0), 7).overtakes;
    assert_eq!(first, second);
}

/// Two modes held at the same speed never pass each other, so no overtaking
/// interval is ever detected even though the bodies come alongside.
#[test]
fn equal_speeds_yield_no_overtake() {
    let observed = run(passing_scenario(4.0, 4.0), 7).overtakes;
    assert!(
        observed.is_empty(),
        "an abreast convoy is not an overtake: {observed:?}"
    );
}

/// A scenario that authors no overtaking capability and no band still runs and
/// records nothing: the pass is inert where no overtake exists.
#[test]
fn a_single_mode_run_records_nothing() {
    let source = parse_scenario_source_v2(
        r#"{
  schema_version: 2,
  id: 'close_pass_no_demand',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 300.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.0 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.0 },
  ],
  mode_templates: [
    { id: 'walker', body: { kind: 'capsule', length_m: { min: 1.6, max: 1.6 },
        radius_m: { min: 0.3, max: 0.3 } },
      motion: 'single_body_wheeled', tactics: [ 'follow', 'stop', 'yield' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 4.0, max: 4.0 },
        max_accel_mps2: { min: 1.0, max: 1.0 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_clearance_m: { min: 0.1, max: 0.1 },
        compliance: { min: 1.0, max: 1.0 },
      },
    },
  ],
}"#,
    )
    .expect("the document is version 2");
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    let mut sim = Simulation::new(scenario, RunConfig::new(1)).expect("the simulation builds");
    for _ in 0..TICKS {
        sim.step();
    }
    assert!(sim.close_pass_tracker().overtakes().is_empty());
}

/// An overtaking interval still alongside at the run's final tick is closed by
/// the run-end close boundary as one observation carrying exactly the evidence
/// the completed interval carried, so a pass in progress at termination is
/// neither dropped nor counted twice. It re-runs the fixture for the prefix of
/// the shared run up to an existing interval's `end_tick`.
fn assert_the_interval_still_open_at_the_final_tick_closes_with_the_completed_evidence(
    completed: &[OvertakeObservation],
) {
    // An interval that stayed alongside for more than one tick was still open on
    // its last observed tick, so a run that stops there leaves it open.
    let full = completed
        .iter()
        .find(|observation| observation.end_tick > observation.start_tick)
        .expect("the fixture records an overtaking interval spanning several ticks");
    let final_tick = full.end_tick;

    let mut sim = Simulation::new(passing_scenario(9.0, 4.0), RunConfig::new(7))
        .expect("the simulation builds");
    for _ in 0..final_tick {
        sim.step();
    }
    let before = sim.close_pass_tracker().overtakes().len();

    sim.close_open_close_passes();

    let closed = sim.close_pass_tracker().overtakes();
    assert!(
        closed.len() > before,
        "the run end closes the interval still alongside at the final tick"
    );
    let expected: Vec<OvertakeObservation> = completed
        .iter()
        .filter(|observation| observation.end_tick <= final_tick)
        .cloned()
        .collect();
    assert_eq!(
        closed, expected,
        "the run-end closure records the still-open interval as its one \
         observation, alongside the intervals that closed on their own ticks"
    );
    assert_eq!(
        closed
            .iter()
            .filter(|observation| observation.start_tick == full.start_tick)
            .count(),
        1,
        "the closed-at-run-end interval yields one record for its pair"
    );
    let reopened = closed
        .iter()
        .find(|observation| observation.start_tick == full.start_tick)
        .expect("the interval still open at the final tick closed at run end");
    assert_eq!(
        reopened, full,
        "a run-end closure carries the same minimum, time, relative speed, and \
         bands as the completed interval"
    );
}

/// A completed overtake emits one `ClosePass` event carrying the passing agent,
/// the passed body, the facility and side, the exact minimum with its time and
/// relative speed, and the declaration-ordered bands — the contract's payload.
fn assert_a_completed_pass_emits_one_close_pass_event_with_its_evidence(events: &[(u64, Event)]) {
    assert!(
        !events.is_empty(),
        "the faster motor passing the slower user emits a ClosePass event"
    );
    for (tick, event) in events {
        assert_eq!(event.kind(), EventKind::ClosePass);
        let Event::ClosePass {
            agent,
            partner,
            facility,
            side,
            min_clearance_m,
            min_clearance_time_s,
            relative_speed_mps,
            bands,
            violating_bands,
            ..
        } = event
        else {
            unreachable!("the kind accessor agreed it is a close pass")
        };
        assert_ne!(
            agent, partner,
            "the actor and passed-user roles are distinct"
        );
        assert_eq!(
            event.agent(),
            *agent,
            "the event attributes itself to the actor"
        );
        assert!(
            min_clearance_m.is_finite(),
            "the minimum is a real clearance"
        );
        assert!(
            min_clearance_time_s.is_finite() && *min_clearance_time_s <= *tick as f64 * DT + 1e-9,
            "the minimum's time lies at or before its emitting tick: {min_clearance_time_s}"
        );
        assert!(relative_speed_mps.is_finite());
        assert!(
            !bands.is_empty(),
            "the every-pair band always applies to this pass"
        );
        let ids: Vec<u32> = bands.iter().map(|band| band.band.get()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "bands are reported in declaration order");
        for band in bands {
            assert!(band.duration_s >= 0.0);
        }
        for violating in violating_bands {
            assert!(
                ids.contains(&violating.get()),
                "a violating band is one of the participating bands"
            );
        }
        // The facility and side resolve, so the payload names where and which
        // way the pass happened.
        let _ = facility.get();
        let _ = side.label();
    }
}
