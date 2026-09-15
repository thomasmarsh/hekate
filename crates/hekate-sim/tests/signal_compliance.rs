//! Phase 1 Increment 2 slice C: the contextual signal-compliance decision.
//!
//! These tests author their own scenarios so a benchmark change cannot move the
//! kernel contract. They cover the green/yellow/red boundaries at the
//! simulation level, the recorded decision reason the inspector shows, stream
//! isolation of the `compliance` draws from demand/profile, and same-seed
//! reproducibility of the decision records.

use hekate_model::{CompiledScenario, MovementId, SignalColor, parse_scenario_source};
use hekate_sim::{ComplianceReason, Event, RunConfig, SignalAction, Simulation, SnapshotDetail};

/// A signalized 80 m approach whose movement stops at an authored 34 m stop
/// line. The single phase keeps one color for the whole run so each test pins a
/// fixed boundary, and the compliance range is parameterized.
fn scenario(
    color: &str,
    compliance_min: f64,
    compliance_max: f64,
    rate_vph: f64,
) -> CompiledScenario {
    let text = format!(
        r#"{{
          schema_version: 1,
          id: 'compliance_test',
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
            phases: [ {{ duration_s: 100000.0, states: [ {{ head: 'head', color: '{color}' }} ] }} ] }} ],
          demand: [
            {{ id: 'inflow', portal: 'entry', rate_vph: {rate_vph},
              routes: [ {{ movement: 'through', weight: 1.0 }} ] }},
          ],
          profiles: {{
            speed_mps: {{ min: 10.0, max: 10.0 }},
            length_m: {{ min: 4.0, max: 4.0 }},
            width_m: {{ min: 2.0, max: 2.0 }},
            time_gap_s: {{ min: 1.5, max: 1.5 }},
            max_accel_mps2: {{ min: 2.0, max: 2.0 }},
            comfortable_brake_mps2: {{ min: 3.0, max: 3.0 }},
            compliance: {{ min: {compliance_min}, max: {compliance_max} }},
          }},
        }}"#
    );
    let source = parse_scenario_source(&text).expect("scenario parses");
    CompiledScenario::compile(source).expect("scenario compiles")
}

fn sim(color: &str, compliance_min: f64, compliance_max: f64, seed: u64) -> Simulation {
    Simulation::new(
        scenario(color, compliance_min, compliance_max, 600.0),
        RunConfig::new(seed),
    )
    .expect("simulation builds")
}

/// The stop line's front-bumper distance from the movement entry.
const STOP_LINE_M: f64 = 34.0;

#[test]
fn a_green_head_proceeds_for_every_compliance() {
    for compliance in [0.0, 1.0] {
        let mut sim = sim("green", compliance, compliance, 1);
        let movement = MovementId::from_index(0);
        let mut crossed = 0;
        for _ in 0..3000 {
            assert_eq!(sim.movement_signal(movement), Some(SignalColor::Green));
            sim.step();
            for sample in sim.snapshot(SnapshotDetail::Full).agents() {
                let decision = sim
                    .agent_decision(sample.id)
                    .expect("signal-controlled vehicle records a decision");
                assert_eq!(decision.action, SignalAction::Proceed);
                assert_eq!(decision.reason, ComplianceReason::Green);
                if sample.motion.as_ref().expect("full detail").path_distance_m > STOP_LINE_M {
                    crossed += 1;
                }
            }
        }
        assert!(
            crossed > 0,
            "green must release traffic for compliance {compliance}"
        );
    }
}

#[test]
fn a_red_head_stops_a_compliant_driver_at_the_line() {
    let mut sim = sim("red", 1.0, 1.0, 2);
    let mut stopped = 0;
    for _ in 0..2000 {
        sim.step();
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let motion = sample.motion.as_ref().expect("full detail");
            let decision = sim
                .agent_decision(sample.id)
                .expect("signal-controlled vehicle records a decision");
            if decision.action == SignalAction::Stop {
                assert_eq!(decision.color, SignalColor::Red);
                assert_eq!(decision.reason, ComplianceReason::CompliantStop);
                let front_m = motion.path_distance_m + motion.body_length_m * 0.5;
                assert!(
                    front_m <= STOP_LINE_M + 1e-6,
                    "a stopped vehicle crossed its line: {front_m}"
                );
                stopped += 1;
            }
        }
    }
    assert!(stopped > 0, "a compliant driver never chose to stop");
}

#[test]
fn a_low_compliance_driver_runs_a_red_head() {
    let mut sim = sim("red", 0.0, 0.0, 2);
    let mut ran = 0;
    let mut crossed = false;
    for _ in 0..2000 {
        sim.step();
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let motion = sample.motion.as_ref().expect("full detail");
            let decision = sim
                .agent_decision(sample.id)
                .expect("signal-controlled vehicle records a decision");
            assert_eq!(decision.action, SignalAction::Proceed);
            assert_ne!(
                decision.reason,
                ComplianceReason::Green,
                "a red head cannot be a green proceed"
            );
            if decision.reason == ComplianceReason::NonCompliantRun {
                ran += 1;
            }
            if motion.path_distance_m + motion.body_length_m * 0.5 > STOP_LINE_M {
                crossed = true;
            }
        }
    }
    assert!(ran > 0, "no noncompliant run was recorded");
    assert!(crossed, "a noncompliant driver never reached the line");
}

#[test]
fn a_yellow_head_is_stop_required_for_a_compliant_driver() {
    let mut sim = sim("yellow", 1.0, 1.0, 3);
    let mut stopped = 0;
    for _ in 0..2000 {
        sim.step();
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let decision = sim
                .agent_decision(sample.id)
                .expect("signal-controlled vehicle records a decision");
            if decision.action == SignalAction::Stop {
                assert_eq!(decision.color, SignalColor::Yellow);
                stopped += 1;
            }
        }
    }
    assert!(stopped > 0, "yellow must be treated as stop-required");
}

/// Every spawned agent's route and non-compliance profile fields, in spawn
/// order, as the demand and profile streams own them.
fn owned_outcomes(sim: &mut Simulation) -> Vec<(u32, u32, [f64; 5])> {
    let mut outcomes = Vec::new();
    for _ in 0..4000 {
        let events: Vec<Event> = sim.step().events().to_vec();
        for event in events {
            if let Event::Spawned { agent, .. } = event {
                let route = sim.agent_route(agent).expect("route assigned");
                let profile = sim.agent_profile(agent).expect("profile sampled");
                outcomes.push((
                    agent.get(),
                    route.get(),
                    [
                        profile.desired_speed_mps,
                        profile.length_m,
                        profile.width_m,
                        profile.time_gap_s,
                        profile.max_accel_mps2,
                    ],
                ));
            }
        }
    }
    outcomes
}

#[test]
fn the_compliance_stream_does_not_change_demand_or_profile_outcomes() {
    // A sparse rate keeps the entry clear, so admission itself does not depend
    // on how compliance changed the traffic ahead. Demand arrivals, routes, and
    // the physical/longitudinal profile draws must then be byte-identical even
    // though the `compliance` range, and so every propensity, differs.
    let mut fully_compliant = Simulation::new(scenario("red", 1.0, 1.0, 60.0), RunConfig::new(9))
        .expect("simulation builds");
    let mut mixed = Simulation::new(scenario("red", 0.0, 1.0, 60.0), RunConfig::new(9))
        .expect("simulation builds");
    assert_eq!(
        owned_outcomes(&mut fully_compliant),
        owned_outcomes(&mut mixed)
    );
}

#[test]
fn decision_records_reproduce_for_a_seed() {
    fn trace(seed: u64) -> Vec<String> {
        let mut sim = sim("red", 0.0, 1.0, seed);
        let mut log = Vec::new();
        for _ in 0..2500 {
            sim.step();
            for sample in sim.snapshot(SnapshotDetail::Full).agents() {
                let decision = sim
                    .agent_decision(sample.id)
                    .expect("signal-controlled vehicle records a decision");
                log.push(format!(
                    "{}:{}:{:?}:{:?}:{:.6}:{:.6}",
                    sim.time().tick(),
                    sample.id.get(),
                    decision.action,
                    decision.reason,
                    decision.stop_line_gap_m,
                    decision.required_decel_mps2,
                ));
            }
        }
        log
    }

    let first = trace(4);
    assert!(!first.is_empty());
    assert_eq!(first, trace(4), "same seed must reproduce decision records");
    assert_ne!(
        first,
        trace(5),
        "a different seed should produce different decisions"
    );
}

#[test]
fn every_admitted_signal_vehicle_carries_a_reasoned_decision() {
    let mut sim = sim("red", 0.0, 1.0, 6);
    for _ in 0..1500 {
        sim.step();
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let motion = sample.motion.as_ref().expect("full detail");
            let decision = motion.decision.expect("decision recorded in the snapshot");
            assert_eq!(Some(decision), sim.agent_decision(sample.id));
            // The record always explains its action with a known reason.
            assert!(!decision.reason.label().is_empty());
        }
    }
}
