//! TAS-129: the checked-in Increment 2 passing fixtures.
//!
//! Every fixture under `scenarios/phase2/inc2/` runs here through the kernel,
//! loaded from the same path the CLI and the benchmark matrix name:
//!
//! - `narrow_passing_v2` — a bicycle and a scooter passing on one shared
//!   bikeway;
//! - `motor_passing_narrow_v2` — a passenger car overtaking a slower bicycle on
//!   one shared road;
//! - `motor_lane_change_v2` — a configured adjacent-band change of lane around
//!   a slower motor leader, recorded by a tactical leaf's request.
//!
//! Each fixture is asserted for the same Increment 2 set: the attempt, commit,
//! and completion edges of the maneuver lifecycle; continuous, finite lateral
//! samples with no teleport; the motion and world-boundary limits of the
//! command envelope; the exact close-pass evidence of the declared clearance
//! bands; the route completion of both participants; and no silent contact.
//!
//! ## What the fixtures pin, and what the kernel pins
//!
//! The narrow and motor fixtures author sparse demand rates so that exactly one
//! leader and one follower run the facility over the horizon below; the runs
//! therefore carry exactly one overtaking interval, which these tests assert
//! globally, not only for the pair they watched.
//!
//! The route-completion evidence is the `Spawned` event followed by the
//! `Despawned { reason: ExitedPath }` event. `Event::Entry` and `Event::Exit`
//! are safety-region edges the kernel emits for crossing and conflict regions,
//! and a passing fixture authors neither, so the despawn at the guide path's end
//! is the observable that says both participants completed their route.
//!
//! ## The one command-envelope exemption, and why it is exact
//!
//! A completed within-facility pass also engages the kernel's anti-overlap
//! position cap for one step on the *passed* body: the leader scan selects
//! leaders by path and progress, so when a passer's rear draws level with the
//! passed body's front that body's gap clamps to zero and `command_motion` caps
//! its speed for that step. That is the landed kernel's documented last-resort
//! backstop and not a maneuver reliance: the fixtures record no contact, and the
//! maneuver clears no corridor through it. The landed model fixture in
//! `narrow_passing.rs` reports the same one-cap-per-executed-pass relationship.
//! So the envelope assertion skips exactly the step on which the run's cap
//! counter advanced, and the counter itself is asserted below: zero for the
//! change of lane, which crosses the shared boundary before its passer draws
//! ahead of the leader, and one per executed pass otherwise.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec2;
use hekate_model::{CompiledScenario, FacilityId, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, BodyShape, DespawnReason, Event, LateralManeuverRequest, ManeuverEdge, ManeuverReason,
    ManeuverState, RunConfig, Simulation, SnapshotDetail, StepOutput, body_clearance_m,
};

/// The checked-in narrow passing fixture.
const NARROW: &str = include_str!("../../../scenarios/phase2/inc2/narrow_passing_v2.json5");
/// The checked-in motor-over-narrow overtaking fixture.
const MOTOR: &str = include_str!("../../../scenarios/phase2/inc2/motor_passing_narrow_v2.json5");
/// The checked-in configured lane-change fixture.
const LANE: &str = include_str!("../../../scenarios/phase2/inc2/motor_lane_change_v2.json5");

/// The default 0.05 s step, so a per-step displacement has one bound.
const DT: f64 = 0.05;

/// The root seed every fixture is pinned to.
const SEED: u64 = 0;

/// Ticks that let both participants of a passing fixture enter, pass, settle,
/// and reach the end of their guide path. The narrow and motor fixtures admit
/// their follower 17.5 s after their leader; the lane-change fixture needs the
/// same window plus its crossing out and back.
const NARROW_TICKS: u64 = 1900;
const MOTOR_TICKS: u64 = 1900;
const LANE_TICKS: u64 = 2500;

/// How the suite drives one fixture.
enum Plan {
    /// No request: the fixture's own demand and tactics produce the maneuver.
    Autonomous,
    /// The change of lane the tactical leaf records for the follower once the
    /// leader it displaces around is within `window_m` metres.
    LaneChange {
        /// The adjacent facility the change targets.
        target: FacilityId,
        /// The follower-to-leader gap, in metres, that opens the request.
        window_m: (f64, f64),
    },
}

/// Every observation one run produced, and the checks made while producing it.
#[derive(Default)]
struct Trace {
    /// The maneuver state edges each agent recorded, in step order.
    edges: BTreeMap<AgentId, Vec<(ManeuverState, ManeuverState, ManeuverEdge)>>,
    /// Every lateral maneuver reason observed for an agent over the run.
    reasons: BTreeMap<AgentId, BTreeSet<ManeuverReason>>,
    /// Agents the run admitted.
    spawned: BTreeSet<AgentId>,
    /// Agents the run released at the end of their guide path.
    completed_route: BTreeSet<AgentId>,
    /// Every contact the run recorded, as a description.
    contacts: Vec<String>,
    /// Every cross-body clearance at or below zero that carried no contact
    /// event: the "no silent contact" evidence.
    silent_contacts: Vec<String>,
    /// Every sample that left the world boundary, as a description.
    outside_boundary: Vec<String>,
    /// Every command-envelope violation, as a description.
    envelope: Vec<String>,
    /// The largest per-step world displacement seen across all agents.
    max_step_m: f64,
    /// The facility handoffs the run performed.
    facility_transitions: usize,
    /// Finite route samples observed for a steering body.
    lateral_samples: usize,
    /// The pair a [`Plan::LaneChange`] request named, once it was recorded.
    requested: Option<(AgentId, AgentId)>,
}

impl Trace {
    /// Whether an agent recorded the full maneuver edges
    /// `following -> preparing -> committed -> returning -> following`, in
    /// order, with no abort. Returns the edge count seen for the agent.
    fn completed_maneuver(&self, agent: AgentId) -> Option<usize> {
        let edges = self.edges.get(&agent)?;
        let mut state = ManeuverState::Following;
        for (from, to, edge) in edges {
            if *from != state || *edge == ManeuverEdge::Aborted {
                return None;
            }
            state = *to;
        }
        (state == ManeuverState::Following && edges.len() >= 4).then_some(edges.len())
    }

    /// The agent that completed a maneuver, if any.
    fn passer(&self) -> Option<AgentId> {
        self.edges
            .keys()
            .copied()
            .find(|&agent| self.completed_maneuver(agent).is_some())
    }
}

/// Drive the simulation for `ticks` steps under `plan`, checking every live
/// sample against the world boundary, the sampled profile's speed and
/// acceleration envelope, and every pair's signed body clearance.
fn drive(sim: &mut Simulation, ticks: u64, plan: Plan) -> Trace {
    let ring: Vec<DVec2> = sim.scenario().boundaries()[0].polygon().ring().to_vec();
    let mut trace = Trace::default();
    let mut previous: BTreeMap<AgentId, (DVec2, f64)> = BTreeMap::new();
    for _ in 0..ticks {
        if trace.requested.is_none()
            && let Plan::LaneChange { target, window_m } = &plan
        {
            let frame = sim.snapshot(SnapshotDetail::Full);
            let samples: Vec<(AgentId, f64, f64)> = frame
                .agents()
                .iter()
                .map(|sample| {
                    let motion = sample.motion.as_ref().expect("full detail");
                    (
                        sample.id,
                        motion.path_distance_m,
                        sim.agent_profile(sample.id)
                            .expect("a demand vehicle carries a profile")
                            .desired_speed_mps,
                    )
                })
                .collect();
            let leader = samples.iter().min_by_key(|sample| sample.0.get()).copied();
            let follower = samples
                .iter()
                .filter(|sample| Some(sample.0) != leader.map(|leader| leader.0))
                .min_by_key(|sample| sample.0.get())
                .copied();
            if let (Some(leader), Some(follower)) = (leader, follower)
                && follower.2 > leader.2
                && (window_m.0..=window_m.1).contains(&(leader.1 - follower.1))
            {
                assert!(
                    sim.request_lateral_maneuver(
                        follower.0,
                        LateralManeuverRequest {
                            target_offset_m: 0.0,
                            passed_body: leader.0,
                            target_facility: Some(*target),
                        },
                    ),
                    "the tactical leaf's change of lane is admissible"
                );
                trace.requested = Some((follower.0, leader.0));
            }
        }
        let caps_before = sim.emergency_cap_steps();
        let (events, transitions, handoffs) = {
            let output: StepOutput<'_> = sim.step();
            (
                output.events().to_vec(),
                output
                    .transitions()
                    .iter()
                    .map(|t| (t.agent, t.from, t.to, t.edge))
                    .collect::<Vec<_>>(),
                output.facility_transitions().len(),
            )
        };
        trace.facility_transitions += handoffs;
        let backstop_step = sim.emergency_cap_steps() > caps_before;
        for (agent, from, to, edge) in transitions {
            trace.edges.entry(agent).or_default().push((from, to, edge));
        }
        for event in &events {
            match event {
                Event::Spawned { agent, .. } => {
                    trace.spawned.insert(*agent);
                }
                Event::Despawned { agent, reason, .. } => {
                    if matches!(reason, DespawnReason::ExitedPath) {
                        trace.completed_route.insert(*agent);
                    }
                }
                Event::Collision {
                    agent,
                    other,
                    clearance_m,
                    contacting,
                } => {
                    if *contacting {
                        trace
                            .contacts
                            .push(format!("{agent:?}/{other:?} at {clearance_m:.4} m"));
                    }
                }
                _ => {}
            }
        }
        let frame = sim.snapshot(SnapshotDetail::Full);
        let samples = frame.agents();
        for (index, sample) in samples.iter().enumerate() {
            let motion = sample.motion.as_ref().expect("full detail");
            if let Some(reason) = sim.lateral_maneuver_reason(sample.id) {
                trace.reasons.entry(sample.id).or_default().insert(reason);
            }
            if let Some(route) = &motion.route_state {
                assert!(
                    route.d_m.is_finite() && route.s_m.is_finite(),
                    "a route sample must be finite: {route:?}"
                );
                trace.lateral_samples += 1;
            }
            if !point_in_ring(sample.position, &ring) {
                trace.outside_boundary.push(format!(
                    "{:?} at ({:.3}, {:.3})",
                    sample.id, sample.position.x, sample.position.y
                ));
            }
            let profile = sim
                .agent_profile(sample.id)
                .expect("a demand vehicle carries a sampled profile");
            assert!(
                motion.speed_mps.is_finite()
                    && motion.speed_mps >= -1e-9
                    && motion.speed_mps <= profile.desired_speed_mps + 1e-9,
                "speed {:.4} left the profile bound [0, {:.4}] for {:?}",
                motion.speed_mps,
                profile.desired_speed_mps,
                sample.id
            );
            let body = body_shape(sample);
            for other in &samples[index + 1..] {
                let clearance = body_clearance_m(&body, &body_shape(other));
                if clearance < -1e-9 {
                    trace.silent_contacts.push(format!(
                        "{:?}/{:?} overlapped by {:.4} m",
                        sample.id, other.id, -clearance
                    ));
                }
            }
            if let Some((before, before_speed)) = previous.get(&sample.id) {
                trace.max_step_m = trace.max_step_m.max((sample.position - *before).length());
                // The backstop step is the one the cap counter advanced on; the
                // envelope of every ordinary command is asserted here.
                if !backstop_step {
                    let accel = (motion.speed_mps - before_speed) / DT;
                    if accel > profile.max_accel_mps2 + 1e-6
                        || accel < -profile.comfortable_brake_mps2 - 1e-6
                    {
                        trace.envelope.push(format!(
                            "{:?} accelerated {accel:.3} m/s^2 outside [-{:.3}, {:.3}]",
                            sample.id, profile.comfortable_brake_mps2, profile.max_accel_mps2
                        ));
                    }
                }
            }
            previous.insert(sample.id, (sample.position, motion.speed_mps));
        }
    }
    assert!(
        trace.contacts.is_empty(),
        "a passing fixture records no contact: {:?}",
        trace.contacts
    );
    assert!(
        trace.silent_contacts.is_empty(),
        "no pair may overlap without a contact event: {:?}",
        trace.silent_contacts
    );
    assert!(
        trace.outside_boundary.is_empty(),
        "every sample stays inside the world boundary: {:?}",
        trace.outside_boundary
    );
    assert!(
        trace.envelope.is_empty(),
        "every command stays inside the sampled profile envelope: {:?}",
        trace.envelope
    );
    trace
}

/// The body shape the kernel's own collision scan uses: a vehicle is an
/// oriented box of its sampled length and width.
fn body_shape(sample: &hekate_sim::AgentSample) -> BodyShape {
    let motion = sample.motion.as_ref().expect("full detail");
    BodyShape::Box {
        centre: sample.position,
        heading_rad: sample.heading_rad,
        length_m: motion.body_length_m,
        width_m: motion.body_width_m,
    }
}

/// A ray-cast point-in-ring test for the world boundary.
fn point_in_ring(point: DVec2, ring: &[DVec2]) -> bool {
    let mut inside = false;
    let mut previous = ring.len() - 1;
    for current in 0..ring.len() {
        let (a, b) = (ring[current], ring[previous]);
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < x {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

/// The fixture's simulation, built from its checked-in text.
fn build(text: &str) -> Simulation {
    let source = parse_scenario_source_v2(text).expect("the fixture is a version 2 document");
    let compiled = CompiledScenario::compile_v2(source).expect("the fixture compiles");
    Simulation::new(compiled, RunConfig::new(SEED)).expect("the simulation builds")
}

/// Assert the evidence every passing fixture carries: the passer's whole
/// lifecycle and slower-leader selection, no teleport, finite lateral samples,
/// exactly one overtaking interval naming the pair with the authored bands in
/// declaration order and no violation, and both participants completing their
/// route.
fn assert_shared_evidence(
    trace: &Trace,
    sim: &Simulation,
    horizon: u64,
    study_band_duration_positive: bool,
    expect_slower_leader: bool,
) {
    let observations = sim.close_pass_tracker().overtakes();
    assert_eq!(
        observations.len(),
        1,
        "the fixture carries exactly one overtaking interval: {observations:?}"
    );
    let (passer, passed) = (observations[0].agent, observations[0].partner);
    let observation = &observations[0];
    let edges = trace
        .completed_maneuver(passer)
        .unwrap_or_else(|| panic!("{passer:?} completed the maneuver lifecycle"));
    assert!(
        edges >= 4,
        "{passer:?} recorded the whole lifecycle, got {edges} edges"
    );
    if expect_slower_leader {
        assert!(
            trace
                .reasons
                .get(&passer)
                .is_some_and(|reasons| reasons.contains(&ManeuverReason::SlowerLeader)),
            "{passer:?} selected a slower leader: {:?}",
            trace.reasons.get(&passer)
        );
    } else {
        // A requested change of lane is the tactical leaf's, so the mode's own
        // tactic records its rejection reason instead of a selection.
        assert!(
            trace
                .reasons
                .get(&passer)
                .is_some_and(|reasons| !reasons.is_empty()),
            "{passer:?} recorded its tactic's reason"
        );
    }
    assert!(
        trace.max_step_m <= 12.0 * DT + 1e-6,
        "no agent teleported: the largest step was {} m",
        trace.max_step_m
    );
    assert!(
        trace.lateral_samples > 0,
        "the fixture produced no lateral sample"
    );
    assert!(
        trace.spawned.contains(&passer) && trace.spawned.contains(&passed),
        "both participants were admitted"
    );
    assert!(
        trace.completed_route.contains(&passer) && trace.completed_route.contains(&passed),
        "both participants reached the end of their guide path: passer {}, passed {}",
        trace.completed_route.contains(&passer),
        trace.completed_route.contains(&passed)
    );

    assert_eq!(
        (observation.agent, observation.partner),
        (passer, passed),
        "the observation names the passing pair"
    );
    assert!(
        observation.start_tick <= observation.end_tick && observation.end_tick < horizon,
        "the interval lies inside the run: {observation:?}"
    );
    assert!(
        observation.min_clearance_m.is_finite() && observation.min_clearance_m > 0.5,
        "the close pass stayed outside the authored close band: {observation:?}"
    );
    let ids: Vec<u32> = observation
        .bands
        .iter()
        .map(|band| band.band.get())
        .collect();
    assert_eq!(
        ids,
        (0..ids.len() as u32).collect::<Vec<u32>>(),
        "every declared band participates in declaration order: {observation:?}"
    );
    assert!(
        observation.violating_bands.is_empty(),
        "no band named a violation on this pass: {observation:?}"
    );
    if study_band_duration_positive {
        assert!(
            observation.bands.iter().any(|band| band.duration_s > 0.0),
            "a declared study band accumulated a duration: {observation:?}"
        );
    }
}

/// The narrow fixture: a faster bicycle passes a slower scooter on one shared
/// bikeway, through the ordinary lifecycle and with no teleport.
#[test]
fn narrow_passing_fixture_completes_the_documented_pass() {
    let mut sim = build(NARROW);
    let trace = drive(&mut sim, NARROW_TICKS, Plan::Autonomous);
    assert!(trace.passer().is_some(), "a bicycle completes a pass");
    assert_shared_evidence(&trace, &sim, NARROW_TICKS, true, true);
    assert_eq!(
        sim.emergency_cap_steps(),
        1,
        "one executed pass engages the anti-overlap backstop exactly once"
    );
}

/// The motor fixture: a passenger car overtakes a slower bicycle on one shared
/// road, through the ordinary lifecycle and with no teleport.
#[test]
fn motor_passing_narrow_fixture_completes_the_documented_overtake() {
    let mut sim = build(MOTOR);
    let trace = drive(&mut sim, MOTOR_TICKS, Plan::Autonomous);
    let passer = trace.passer().expect("the car completes an overtake");
    assert_eq!(
        sim.agent_profile(passer).map(|profile| profile.length_m),
        Some(4.5),
        "the passing agent is the motor vehicle"
    );
    let passed = sim.close_pass_tracker().overtakes()[0].partner;
    assert!(
        sim.agent_profile(passed).map(|profile| profile.length_m) < Some(2.0),
        "the passed body is the narrow bicycle"
    );
    assert_shared_evidence(&trace, &sim, MOTOR_TICKS, true, true);
    assert_eq!(
        sim.emergency_cap_steps(),
        1,
        "one executed overtake engages the anti-overlap backstop exactly once"
    );
}

/// The lane-change fixture: a car changes from the carriageway into the
/// adjacent kerb lane around a slower motor leader, crossing the compiled
/// shared boundary out and back, and the crossing records the pass it makes on
/// the way out.
#[test]
fn motor_lane_change_fixture_changes_lane_around_a_slower_leader() {
    let mut sim = build(LANE);
    assert_eq!(
        sim.scenario()
            .facility(FacilityId::from_index(1))
            .map(|facility| facility.name()),
        Some("kerb_lane"),
        "the second declared facility is the change-of-lane target"
    );
    // The request window is the approach's following equilibrium: the follower
    // holds about 10.7 m behind the leader, and one decision inside the window
    // is early enough that its bounded lateral displacement draws it level with
    // the leader before the crossing would carry it out of the band.
    let trace = drive(
        &mut sim,
        LANE_TICKS,
        Plan::LaneChange {
            target: FacilityId::from_index(1),
            window_m: (10.0, 11.5),
        },
    );
    assert!(
        trace.requested.is_some(),
        "the approach reached the request window"
    );
    assert_eq!(
        trace.facility_transitions, 2,
        "the change of lane crosses the shared boundary out and back exactly once"
    );
    assert_eq!(
        sim.emergency_cap_steps(),
        0,
        "the crossing hands the passer off before it draws ahead of the leader, \
         so no position cap is engaged"
    );
    assert_shared_evidence(&trace, &sim, LANE_TICKS, true, false);
    let (passer, _) = trace.requested.expect("a request was recorded");
    assert!(
        trace.completed_maneuver(passer).is_some(),
        "the change of lane completed"
    );
}
