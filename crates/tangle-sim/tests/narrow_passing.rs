//! TAS-093: same-facility narrow passing eligibility, target, and claim.
//!
//! A narrow wheeled agent with the `pass` capability overtakes a slower narrow
//! leader inside one shared continuous-width facility. The tactic under test is
//! the eligibility and target seam: a component-driven decision that records an
//! ordinary [`LateralManeuverRequest`](tangle_sim::LateralManeuverRequest) when
//! every precondition holds, and the kernel's ordinary predictor, claim,
//! state-machine, steering, collision, and longitudinal paths carry the pass
//! from there. Nothing here scripts a speed or a position.
//!
//! The fixtures author the two narrow modes by name (this is test code), but the
//! kernel reads only their compiled components: the capability set, the lateral
//! policy, the traversal permission, the usable interval, and the measured
//! leader geometry. The tests observe the pass through the public API only.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec2;
use tangle_model::{CompiledScenario, parse_scenario_source_v2};
use tangle_sim::{
    AgentId, ManeuverEdge, ManeuverReason, ManeuverState, RunConfig, Simulation, SnapshotDetail,
    StepOutput,
};

/// The default 0.05 s step, so a per-step position delta has one bound.
const DT: f64 = 0.05;

/// `900` ticks is 45 s: long enough for several demand cycles, a catch-up, and
/// a completed pass in either travel direction (the first pass returns to
/// `following` by about tick 790 with the fixtures below).
const TICKS: u64 = 900;

/// What one run of the fixture recorded.
struct Trace {
    /// The maneuver state edges each agent recorded, in step order.
    edges: BTreeMap<AgentId, Vec<(ManeuverState, ManeuverState, ManeuverEdge)>>,
    /// Every narrow pass reason observed for an agent over the run.
    reasons: BTreeMap<AgentId, BTreeSet<ManeuverReason>>,
    /// The largest per-step position delta seen across all agents.
    max_step_m: f64,
}

impl Trace {
    /// Whether an agent recorded the full pass edges:
    /// `following -> preparing -> committed -> returning -> following`, in
    /// order, with no abort.
    fn completed_a_pass(&self, agent: AgentId) -> bool {
        let Some(edges) = self.edges.get(&agent) else {
            return false;
        };
        let mut state = ManeuverState::Following;
        for (from, to, edge) in edges {
            if *from != state || *edge == ManeuverEdge::Aborted {
                return false;
            }
            state = *to;
        }
        state == ManeuverState::Following && edges.len() >= 4
    }

    /// The agent that completed a pass, if any.
    fn passer(&self) -> Option<AgentId> {
        self.edges
            .keys()
            .copied()
            .find(|&agent| self.completed_a_pass(agent))
    }

    /// Every reason observed anywhere in the run.
    fn all_reasons(&self) -> BTreeSet<ManeuverReason> {
        self.reasons.values().flatten().copied().collect()
    }

    /// Whether any agent recorded a maneuver transition at all.
    fn any_maneuver(&self) -> bool {
        self.edges.values().any(|edges| !edges.is_empty())
    }
}

/// Drive the simulation for `ticks` steps, recording maneuver edges, pass
/// reasons, and per-step world positions.
fn drive(sim: &mut Simulation, ticks: u64) -> Trace {
    let mut trace = Trace {
        edges: BTreeMap::new(),
        reasons: BTreeMap::new(),
        max_step_m: 0.0,
    };
    let mut previous: BTreeMap<AgentId, DVec2> = BTreeMap::new();
    for _ in 0..ticks {
        let transitions: Vec<(AgentId, ManeuverState, ManeuverState, ManeuverEdge)> = {
            let output: StepOutput<'_> = sim.step();
            output
                .transitions()
                .iter()
                .map(|t| (t.agent, t.from, t.to, t.edge))
                .collect()
        };
        for (agent, from, to, edge) in transitions {
            trace.edges.entry(agent).or_default().push((from, to, edge));
        }
        let frame = sim.snapshot(SnapshotDetail::Position);
        for sample in frame.agents() {
            if let Some(reason) = sim.narrow_pass_reason(sample.id) {
                trace.reasons.entry(sample.id).or_default().insert(reason);
            }
            if let Some(before) = previous.get(&sample.id) {
                trace.max_step_m = trace.max_step_m.max((sample.position - *before).length());
            }
            previous.insert(sample.id, sample.position);
        }
    }
    trace
}

/// One version-2 continuous-width facility with two narrow modes and a demand
/// source for each, so a faster follower can enter behind a slower leader.
///
/// `leader` and `follower` name one of the two narrow mode templates (`bicycle`
/// or `scooter`); their speeds are authored into the templates. The leader's
/// demand source is first, so the kernel admits a leader before a follower in
/// each demand cycle and a follower trails a slower body.
#[allow(clippy::too_many_arguments)]
fn passing_scenario(
    leader: &str,
    leader_speed: f64,
    follower: &str,
    follower_speed: f64,
    width_m: f64,
    passing_side: &str,
    overtake_effect: Option<&str>,
    reverse: bool,
) -> String {
    let (from, to, direction) = if reverse {
        ("exit", "entry", "reverse")
    } else {
        ("entry", "exit", "forward")
    };
    let half = width_m * 0.5;
    // An `overtake` statement is only valid for a mode that declares the
    // `overtake` tactic, so the prohibition fixture authors it on the follower.
    let permission = match overtake_effect {
        Some(effect) => format!(
            "{{ id: 'follower_overtake', kind: 'overtake', holder: '{follower}', \
             target: 'bikeway', effect: '{effect}' }}"
        ),
        None => String::new(),
    };
    let mode = |id: &str, speed: f64, length: f64, radius: f64| {
        format!(
            r#"{{
      id: '{id}',
      body: {{ kind: 'capsule', length_m: {{ min: {length}, max: {length} }},
        radius_m: {{ min: {radius}, max: {radius} }} }},
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'overtake', 'pass' ],
      access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: {{ limit_mps: null }} }},
      occupancy: 'operator_only',
      profiles: {{
        speed_mps: {{ min: {speed}, max: {speed} }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_accel_max_mps2: {{ min: 2.0, max: 2.0 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
      lateral: {{ target_clearance_m: 0.75, horizon_s: 2.0 }},
    }}"#
        )
    };
    let bicycle_speed = if leader == "bicycle" {
        leader_speed
    } else {
        follower_speed
    };
    let scooter_speed = if leader == "scooter" {
        leader_speed
    } else {
        follower_speed
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'narrow_passing_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [ {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 500.0, y: 0.0 }} ] }} ],
  portals: [
    {{ id: 'entry', path: 'guide', end: 'start', width_m: {width_m} }},
    {{ id: 'exit', path: 'guide', end: 'end', width_m: {width_m} }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -20.0, y: -10.0 }}, {{ x: 520.0, y: -10.0 }},
      {{ x: 520.0, y: 10.0 }}, {{ x: -20.0, y: 10.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band', points: [
      {{ x: 0.0, y: -{half} }}, {{ x: 500.0, y: -{half} }},
      {{ x: 500.0, y: {half} }}, {{ x: 0.0, y: {half} }},
    ] }},
  ],
  facilities: [
    {{ id: 'bikeway', region: 'band', reference_path: 'guide',
      width_m: {width_m}, nominal_direction: 'forward',
      access: {{ modes: [ 'bicycle', 'scooter' ] }}, lateral_use: 'shared',
      lateral_policy: {{ passing_side: '{passing_side}' }},
      speed_policy: {{ limit_mps: null }} }},
  ],
  movements: [
    {{ id: 'through', from: '{from}', to: '{to}', path: 'guide', priority: 0,
      direction: '{direction}' }},
  ],
  mode_templates: [
    {},
    {},
  ],
  permissions: [ {permission} ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'leader_inflow', mode: '{leader}',
      spawn: {{ rate: {{
        portal: '{from}',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'follower_inflow', mode: '{follower}',
      spawn: {{ rate: {{
        portal: '{from}',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
  ],
}}
"#,
        mode("bicycle", bicycle_speed, 1.8, 0.35),
        mode("scooter", scooter_speed, 1.2, 0.28),
    )
}

fn build(scenario: &str) -> Simulation {
    let source = parse_scenario_source_v2(scenario).expect("the document is version 2");
    let compiled = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    Simulation::new(compiled, RunConfig::new(0)).expect("the simulation builds")
}

/// The eligible pass: a bicycle catches a slower scooter and completes a pass
/// through the ordinary lifecycle, without a teleport or route change.
#[test]
fn a_bicycle_passes_a_slower_scooter() {
    let scenario = passing_scenario("scooter", 4.0, "bicycle", 7.0, 6.0, "left", None, false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    let passer = trace.passer().expect("a bicycle completes a pass");
    assert!(
        trace
            .reasons
            .get(&passer)
            .is_some_and(|reasons| reasons.contains(&ManeuverReason::SlowerLeader)),
        "the passer recorded a slower-leader selection"
    );
    assert!(
        trace.max_step_m <= 7.5 * DT + 1e-6,
        "no agent teleported: max step {} m",
        trace.max_step_m
    );
}

/// The complementary direction: a faster scooter passes a slower bicycle.
#[test]
fn a_scooter_passes_a_slower_bicycle() {
    let scenario = passing_scenario("bicycle", 4.0, "scooter", 7.0, 6.0, "left", None, false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    let passer = trace.passer().expect("a scooter completes a pass");
    assert!(
        trace
            .reasons
            .get(&passer)
            .is_some_and(|reasons| reasons.contains(&ManeuverReason::SlowerLeader)),
        "the passer recorded a slower-leader selection"
    );
    assert!(
        trace.max_step_m <= 7.5 * DT + 1e-6,
        "no agent teleported: max step {} m",
        trace.max_step_m
    );
}

/// The pass selects the positive-`d` side in the agent's own travel frame
/// whichever way the facility is travelled: the resolved side never depends on
/// the reference path's own vertex order.
#[test]
fn the_pass_side_is_the_travel_frame_side_in_both_directions() {
    for reverse in [false, true] {
        let scenario = passing_scenario("scooter", 4.0, "bicycle", 7.0, 6.0, "left", None, reverse);
        let mut sim = build(&scenario);
        let trace = drive(&mut sim, TICKS);
        let passer = trace
            .passer()
            .unwrap_or_else(|| panic!("a pass completes (reverse={reverse})"));
        assert!(
            trace
                .reasons
                .get(&passer)
                .is_some_and(|reasons| reasons.contains(&ManeuverReason::SlowerLeader)),
            "the passer selected a slower leader (reverse={reverse})"
        );
        assert!(
            trace.max_step_m <= 7.5 * DT + 1e-6,
            "no agent teleported (reverse={reverse})"
        );
    }
}

/// A facility too narrow for the pass rejects the eligible candidates with the
/// inspectable width reason, and attempts no maneuver at all.
#[test]
fn a_narrow_facility_rejects_the_pass_as_insufficient_width() {
    let scenario = passing_scenario("scooter", 4.0, "bicycle", 7.0, 2.6, "left", None, false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    assert!(!trace.any_maneuver(), "no pass is attempted");
    assert!(
        trace
            .all_reasons()
            .contains(&ManeuverReason::InsufficientWidth),
        "a narrow facility rejects the eligible pass with insufficient_width: {:?}",
        trace.all_reasons()
    );
}

/// An applicable `overtake` prohibition makes the eligible pass ineligible with
/// the permission reason, even though the geometry would allow it.
#[test]
fn a_prohibited_facility_rejects_the_pass_as_no_permission() {
    let scenario = passing_scenario(
        "scooter",
        4.0,
        "bicycle",
        7.0,
        6.0,
        "left",
        Some("prohibit"),
        false,
    );
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    assert!(!trace.any_maneuver(), "no pass is attempted");
    assert!(
        trace.all_reasons().contains(&ManeuverReason::NoPermission),
        "a prohibited facility rejects with no_permission: {:?}",
        trace.all_reasons()
    );
}

/// A leader that is not slower offers no route benefit, so no pass is selected
/// and the follower simply keeps following. Both modes author the same speed, so
/// whichever mode leads, the leader offers no benefit.
#[test]
fn a_leader_with_no_route_benefit_rejects_the_pass_as_no_benefit() {
    let scenario = passing_scenario("bicycle", 5.0, "scooter", 5.0, 6.0, "left", None, false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    assert!(!trace.any_maneuver(), "no pass is attempted");
    assert!(
        trace.all_reasons().contains(&ManeuverReason::NoBenefit),
        "a faster leader rejects with no_benefit: {:?}",
        trace.all_reasons()
    );
}

/// The tactic is a pure function of the compiled components and the tick-start
/// geometry: the same scenario and seed produce the same transitions and the
/// same selection reasons, with no dependence on iteration order.
#[test]
fn the_pass_decision_is_stable_for_a_seed() {
    let run = || {
        let scenario = passing_scenario("scooter", 4.0, "bicycle", 7.0, 6.0, "left", None, false);
        let mut sim = build(&scenario);
        let trace = drive(&mut sim, TICKS);
        let reasons = trace.all_reasons();
        (trace.edges, reasons)
    };
    assert_eq!(run(), run(), "the same seed reproduces the decisions");
}
