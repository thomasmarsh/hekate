//! TAS-094: motor-vehicle overtaking of narrow users.
//!
//! A single-body motor vehicle (a wheeled box) overtakes a slower narrow user
//! (a wheeled capsule) inside one shared continuous-width facility. The tactic
//! under test is component-driven: a motor mode that declares the compiled
//! `overtake` capability and an authored `lateral` policy flows through the
//! same eligibility, target, predictor, claim, state-machine, steering,
//! collision, and longitudinal paths as every other wheeled agent. Nothing here
//! scripts a speed or a position, and no code path names `passenger_car`,
//! `bicycle`, or `scooter` — the fixtures do, because fixtures author modes by
//! name.
//!
//! The tests observe the overtake through the public API: the maneuver state
//! edges and eligibility reasons from [`StepOutput::transitions`] and
//! [`Simulation::lateral_maneuver_reason`], the route-relative offset and
//! committed target from a snapshot, and the ordinary predictor through
//! [`hekate_sim::predict_maneuver_corridor`].

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec2;
use hekate_model::{CompiledReferencePath, CompiledScenario, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, BoundedSteering, LateralCorridor, ManeuverEdge, ManeuverInputs, ManeuverReason,
    ManeuverState, PredictedBody, PredictionVerdict, RunConfig, Simulation, SnapshotDetail,
    SteeringLimits, StepOutput, predict_maneuver_corridor,
};

/// The default 0.05 s step, so a per-step position delta has one bound.
const DT: f64 = 0.05;

/// `900` ticks is 45 s: long enough for several demand cycles, a catch-up, and a
/// completed overtake on the fixtures below.
const TICKS: u64 = 900;

/// What one run of the fixture recorded.
#[derive(Default)]
struct Trace {
    /// The maneuver state edges each agent recorded, in step order.
    edges: BTreeMap<AgentId, Vec<(ManeuverState, ManeuverState, ManeuverEdge)>>,
    /// Every lateral maneuver reason observed for an agent over the run.
    reasons: BTreeMap<AgentId, BTreeSet<ManeuverReason>>,
    /// The largest per-step position delta seen across all agents.
    max_step_m: f64,
    /// The largest positive and most negative signed route offset each agent
    /// held, in the agent's own travel frame.
    max_offset_m: BTreeMap<AgentId, f64>,
    min_offset_m: BTreeMap<AgentId, f64>,
    /// The target offset each agent committed to, when it first committed.
    committed_target_m: BTreeMap<AgentId, f64>,
}

impl Trace {
    /// Whether an agent recorded the full overtake edges:
    /// `following -> preparing -> committed -> returning -> following`, in
    /// order, with no abort.
    fn completed_an_overtake(&self, agent: AgentId) -> bool {
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

    /// The agent that completed an overtake, if any.
    fn overtaker(&self) -> Option<AgentId> {
        self.edges
            .keys()
            .copied()
            .find(|&agent| self.completed_an_overtake(agent))
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

/// Drive the simulation for `ticks` steps, recording maneuver edges, reasons,
/// route offsets, and per-step world positions.
fn drive(sim: &mut Simulation, ticks: u64) -> Trace {
    let mut trace = Trace::default();
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
        let frame = sim.snapshot(SnapshotDetail::Full);
        for sample in frame.agents() {
            if let Some(reason) = sim.lateral_maneuver_reason(sample.id) {
                trace.reasons.entry(sample.id).or_default().insert(reason);
            }
            if let Some(before) = previous.get(&sample.id) {
                trace.max_step_m = trace.max_step_m.max((sample.position - *before).length());
            }
            previous.insert(sample.id, sample.position);
            if let Some(motion) = &sample.motion
                && let Some(route) = &motion.route_state
            {
                let entry = trace.max_offset_m.entry(sample.id).or_insert(f64::MIN);
                *entry = entry.max(route.d_m);
                let entry = trace.min_offset_m.entry(sample.id).or_insert(f64::MAX);
                *entry = entry.min(route.d_m);
                if route.maneuver_state == ManeuverState::Committed {
                    trace
                        .committed_target_m
                        .entry(sample.id)
                        .or_insert(route.target_offset_m.unwrap_or(0.0));
                }
            }
        }
    }
    trace
}

/// One version-2 continuous-width road with a motor mode and a narrow mode and
/// a demand source for each, so a faster motor can enter behind a slower narrow
/// user and overtake it.
///
/// The motor template is named `passenger_car` because the version-2 → version-1
/// view derives the shared wheeled longitudinal distribution from that
/// template; the kernel reads only its compiled components. `overtake_effect`
/// authors an `overtake` statement for the motor when present.
fn overtaking_scenario(
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
    let permission = match overtake_effect {
        Some(effect) => format!(
            "{{ id: 'car_overtake', kind: 'overtake', holder: 'passenger_car', \
             target: 'road', effect: '{effect}' }}"
        ),
        None => String::new(),
    };
    format!(
        r#"{{
  schema_version: 2,
  id: 'motor_passing_narrow_v2',
  coordinate_system: {{ x: 'east_m', y: 'north_m' }},
  paths: [ {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 500.0, y: 0.0 }} ] }} ],
  portals: [
    {{ id: 'entry', path: 'guide', end: 'start', width_m: {width_m} }},
    {{ id: 'exit', path: 'guide', end: 'end', width_m: {width_m} }},
  ],
  boundaries: [
    {{ id: 'world', points: [
      {{ x: -20.0, y: -20.0 }}, {{ x: 520.0, y: -20.0 }},
      {{ x: 520.0, y: 20.0 }}, {{ x: -20.0, y: 20.0 }},
    ] }},
  ],
  regions: [
    {{ id: 'band', points: [
      {{ x: 0.0, y: -{half} }}, {{ x: 500.0, y: -{half} }},
      {{ x: 500.0, y: {half} }}, {{ x: 0.0, y: {half} }},
    ] }},
  ],
  facilities: [
    {{ id: 'road', region: 'band', reference_path: 'guide',
      width_m: {width_m}, nominal_direction: 'forward',
      access: {{ modes: [ 'passenger_car', 'bicycle' ] }}, lateral_use: 'shared',
      lateral_policy: {{ passing_side: '{passing_side}' }},
      speed_policy: {{ limit_mps: null }} }},
  ],
  movements: [
    {{ id: 'through', from: '{from}', to: '{to}', path: 'guide', priority: 0,
      direction: '{direction}' }},
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
        speed_mps: {{ min: 9.0, max: 9.0 }},
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
        speed_mps: {{ min: 4.0, max: 4.0 }},
        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
        time_gap_s: {{ min: 1.0, max: 1.0 }},
        steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
        lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
        compliance: {{ min: 1.0, max: 1.0 }},
      }},
    }},
  ],
  permissions: [ {permission} ],
  maneuver_policy: {{
    commit: {{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }},
  }},
  demand: [
    {{ id: 'bicycle_inflow', mode: 'bicycle',
      spawn: {{ rate: {{
        portal: '{from}',
        rate_per_hour: 450.0,
        interval_s: {{ start_s: 0.0, end_s: null }},
        choice: {{ movements: [ {{ movement: 'through', weight: 1.0 }} ] }},
      }} }} }},
    {{ id: 'car_inflow', mode: 'passenger_car',
      spawn: {{ rate: {{
        portal: '{from}',
        rate_per_hour: 450.0,
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

/// The eligible overtake: a motor vehicle catches a slower bicycle and
/// completes the full lifecycle, without a teleport or route change.
#[test]
fn a_motor_vehicle_overtakes_a_slower_bicycle() {
    let scenario = overtaking_scenario(10.0, "left", None, false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    let overtaker = trace
        .overtaker()
        .expect("a motor vehicle completes an overtake");
    assert!(
        trace
            .reasons
            .get(&overtaker)
            .is_some_and(|reasons| reasons.contains(&ManeuverReason::SlowerLeader)),
        "the overtaker recorded a slower-leader selection"
    );
    // The overtake displaces the motor to the positive-`d` (left) side in its
    // own travel frame and back.
    assert!(
        trace.max_offset_m.get(&overtaker).copied().unwrap_or(0.0) > 1.0,
        "the motor displaced onto the passing side: {:?}",
        trace.max_offset_m.get(&overtaker)
    );
    assert!(
        trace.max_step_m <= 9.0 * DT + 1e-6,
        "no agent teleported: max step {} m",
        trace.max_step_m
    );
}

/// A road too narrow for the motor and the narrow user to share laterally
/// rejects the eligible overtake with the inspectable width reason, and attempts
/// no maneuver at all.
#[test]
fn a_narrow_road_rejects_the_overtake_as_insufficient_width() {
    let scenario = overtaking_scenario(6.0, "left", None, false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    assert!(!trace.any_maneuver(), "no overtake is attempted");
    assert!(
        trace
            .all_reasons()
            .contains(&ManeuverReason::InsufficientWidth),
        "a narrow road rejects the eligible overtake with insufficient_width: {:?}",
        trace.all_reasons()
    );
}

/// An applicable `overtake` prohibition makes the motor ineligible with the
/// permission reason, even though the geometry would allow the overtake.
#[test]
fn a_prohibited_road_rejects_the_overtake_as_no_permission() {
    let scenario = overtaking_scenario(10.0, "left", Some("prohibit"), false);
    let mut sim = build(&scenario);
    let trace = drive(&mut sim, TICKS);

    assert!(!trace.any_maneuver(), "no overtake is attempted");
    assert!(
        trace
            .reasons
            .values()
            .flatten()
            .any(|reason| *reason == ManeuverReason::NoPermission),
        "a prohibited road rejects with no_permission: {:?}",
        trace.all_reasons()
    );
}

/// The tactic is a pure function of the compiled components and the tick-start
/// geometry: a left policy commits to the positive-`d` target and a right policy
/// to the negative-`d` one, whichever side the motor's own travel frame is.
/// (The same seed's reproduction of the decisions, and the invariance of the
/// batch to request order, are asserted at unit level: `sim.rs`
/// `same_seed_produces_identical_observations`,
/// `a_batch_is_decided_independently_of_request_order`, and
/// `the_event_buffer_is_key_ordered_and_invariant_to_request_order`.)
#[test]
fn the_overtake_side_follows_the_policy() {
    let run = |side: &str| {
        let scenario = overtaking_scenario(10.0, side, None, false);
        let mut sim = build(&scenario);
        drive(&mut sim, TICKS)
    };
    let left = run("left");
    let right = run("right");

    let left_overtaker = left.overtaker().expect("a left-side overtake completes");
    let right_overtaker = right.overtaker().expect("a right-side overtake completes");
    assert!(
        left.committed_target_m[&left_overtaker] > 0.0,
        "a left policy commits to the positive-`d` target: {:?}",
        left.committed_target_m[&left_overtaker]
    );
    assert!(
        right.committed_target_m[&right_overtaker] < 0.0,
        "a right policy commits to the negative-`d` target: {:?}",
        right.committed_target_m[&right_overtaker]
    );
}

/// The ordinary predictor accounts for the motor's own box envelope, the narrow
/// capsule ahead, its relative speed, and the mode's target clearance: a clear
/// corridor is feasible, and a closing rear body or an occupied opposing
/// corridor is not.
#[test]
fn the_predictor_settles_the_motor_box_against_front_rear_and_opposing_bodies() {
    let geometry =
        CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(400.0, 0.0)]);
    let motor = |bodies: &[PredictedBody], target_offset_m: f64| {
        predict_maneuver_corridor(
            ManeuverInputs {
                geometry: &geometry,
                facility_width_m: 10.0,
                direction: 1.0,
                body: hekate_sim::BodyShape::Box {
                    centre: DVec2::new(50.0, 0.0),
                    heading_rad: 0.0,
                    length_m: 4.5,
                    width_m: 1.8,
                },
                speed_mps: 8.0,
                target_offset_m,
                steering: BoundedSteering {
                    limits: SteeringLimits {
                        heading_rate_max_rad_s: 0.9,
                        lateral_accel_max_mps2: 2.0,
                    },
                    corridor: LateralCorridor {
                        d_min: -3.8,
                        d_max: 3.8,
                    },
                },
                target_clearance_m: 0.75,
                horizon_s: 2.0,
                cadence_s: 0.05,
                subdivisions: 8,
            },
            bodies,
        )
    };
    let narrow_leader = PredictedBody {
        id: AgentId::from_index(0),
        shape: hekate_sim::BodyShape::Box {
            centre: DVec2::new(64.0, 0.0),
            heading_rad: 0.0,
            length_m: 1.8,
            width_m: 0.7,
        },
        velocity_mps: DVec2::new(4.0, 0.0),
    };

    // A clear corridor past the slower narrow leader is feasible, and the
    // narrow leader is the front body the box settles against.
    let clear = motor(&[narrow_leader], 2.675);
    assert!(clear.is_feasible(), "verdict {:?}", clear.verdict);
    let front = clear.clears.front.expect("a body is ahead");
    assert!(front.clearance_m >= 0.75, "front {}", front.clearance_m);
    assert_eq!(
        front.object,
        hekate_sim::LimitingObject::Agent(narrow_leader.id)
    );

    // A wide body closing from behind within the horizon is an insufficient
    // rear gap: the maneuver is rejected, limited by that body.
    let rear = PredictedBody {
        id: AgentId::from_index(1),
        shape: hekate_sim::BodyShape::Box {
            centre: DVec2::new(42.0, 0.0),
            heading_rad: 0.0,
            length_m: 6.0,
            width_m: 2.5,
        },
        velocity_mps: DVec2::new(14.0, 0.0),
    };
    let blocked = motor(&[narrow_leader, rear], 2.675);
    assert!(
        !blocked.is_feasible(),
        "a closing rear body rejects the overtake: {:?}",
        blocked.verdict
    );
    let rear_fact = blocked.clears.rear.expect("a body is behind");
    assert!(
        rear_fact.clearance_m < 0.75,
        "rear {}",
        rear_fact.clearance_m
    );
    assert_eq!(
        blocked.verdict,
        PredictionVerdict::Infeasible {
            limiting: hekate_sim::LimitingObject::Agent(rear.id)
        }
    );

    // A body travelling the other way in the corridor the motor would occupy is
    // an occupied opposing corridor: the maneuver is rejected, limited by it.
    let opposing = PredictedBody {
        id: AgentId::from_index(2),
        shape: hekate_sim::BodyShape::Box {
            centre: DVec2::new(66.0, 2.0),
            heading_rad: std::f64::consts::PI,
            length_m: 4.5,
            width_m: 1.8,
        },
        velocity_mps: DVec2::new(-8.0, 0.0),
    };
    let opposed = motor(&[narrow_leader, opposing], 2.675);
    assert!(
        !opposed.is_feasible(),
        "an occupied opposing corridor rejects the overtake: {:?}",
        opposed.verdict
    );
    assert_eq!(
        opposed.verdict,
        PredictionVerdict::Infeasible {
            limiting: hekate_sim::LimitingObject::Agent(opposing.id)
        }
    );
}

/// A target outside the usable corridor is rejected on the band edge rather than
/// crossing the boundary silently, and the mode's own clearance is what the
/// corridor bound holds.
#[test]
fn a_target_outside_the_corridor_is_rejected_on_the_band_edge() {
    let geometry =
        CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(400.0, 0.0)]);
    let prediction = predict_maneuver_corridor(
        ManeuverInputs {
            geometry: &geometry,
            facility_width_m: 10.0,
            direction: 1.0,
            body: hekate_sim::BodyShape::Box {
                centre: DVec2::new(50.0, 0.0),
                heading_rad: 0.0,
                length_m: 4.5,
                width_m: 1.8,
            },
            speed_mps: 8.0,
            target_offset_m: 8.0,
            steering: BoundedSteering {
                limits: SteeringLimits {
                    heading_rate_max_rad_s: 0.9,
                    lateral_accel_max_mps2: 2.0,
                },
                corridor: LateralCorridor {
                    d_min: -3.8,
                    d_max: 3.8,
                },
            },
            target_clearance_m: 0.75,
            // A horizon long enough for the first-order lateral approach to
            // reach the out-of-corridor target, so the rejection is the
            // boundary and not merely the horizon.
            horizon_s: 10.0,
            cadence_s: 0.05,
            subdivisions: 8,
        },
        &[],
    );
    assert_eq!(
        prediction.verdict,
        PredictionVerdict::Infeasible {
            limiting: hekate_sim::LimitingObject::BandEdge
        }
    );
}
