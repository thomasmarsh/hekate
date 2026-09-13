//! Increment 1: narrow modes spawn from their compiled mode template.
//!
//! `tangle-sim` spawns a demand source through the family its mode template
//! compiles to: a capsule template (the checked-in `bicycle` and `scooter`
//! templates) spawns a narrow wheeled agent with a narrow profile and a capsule
//! body, and the agent then runs the same four shared stages as a car, with the
//! narrow wheeled model reached through the model seam. This test drives the
//! checked-in narrow fixture and observes the capsule bodies, the narrow
//! profiles, the body-bound agreement, the bounded speed, and a completed
//! route — all without a mode name entering the kernel.

use std::collections::BTreeSet;

use tangle_model::{BodyKind, CompiledScenario, parse_scenario_source_v2};
use tangle_sim::{AgentId, AgentMode, Event, RunConfig, Simulation, SnapshotDetail};

/// The checked-in narrow mode fixture (TAS-076): a `passenger_car` plus the
/// `bicycle` and `scooter` capsule templates, each with one demand source.
const FIXTURE: &str =
    include_str!("../../tangle-model/tests/fixtures/narrow_mode_templates_v2.json5");

/// 300 s at the default 0.05 s step. The 200 m guide path takes a narrow agent
/// about 40 s, and the authored rates (120/h bicycle, 60/h scooter) admit
/// several of each, so the run exercises spawn, motion, and route completion.
const TICKS: u64 = 6000;

#[test]
fn narrow_modes_spawn_from_their_compiled_template_with_their_body_and_profile() {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    let scenario = CompiledScenario::compile_v2(source).expect("the narrow scenario compiles");
    let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds");

    let mut narrow_ids: BTreeSet<AgentId> = BTreeSet::new();
    let mut car_bodies = 0usize;
    let mut narrow_despawned = false;
    let mut peak_narrow_speed = 0.0f64;

    for _ in 0..TICKS {
        let events: Vec<Event> = sim.step().events().to_vec();
        for event in events {
            if let Event::Despawned { agent, .. } = event
                && sim.agent_narrow_profile(agent).is_some()
            {
                narrow_despawned = true;
            }
        }
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let motion = sample.motion.as_ref().expect("full detail");
            match sim.agent_narrow_profile(sample.id) {
                Some(narrow) => {
                    narrow_ids.insert(sample.id);
                    // The body is a capsule sized exactly by the sampled narrow
                    // profile, and the body_kind column reports it.
                    assert_eq!(
                        motion.body_kind,
                        BodyKind::Capsule,
                        "a narrow mode reports a capsule body"
                    );
                    assert!((motion.body_length_m - narrow.length_m).abs() < 1e-12);
                    assert!((motion.body_width_m - narrow.radius_m * 2.0).abs() < 1e-12);
                    // The shared stages bound the speed to the narrow profile.
                    assert!(
                        motion.speed_mps <= narrow.desired_speed_mps + 1e-9,
                        "a narrow agent exceeded its desired speed: {} > {}",
                        motion.speed_mps,
                        narrow.desired_speed_mps
                    );
                    peak_narrow_speed = peak_narrow_speed.max(motion.speed_mps);
                }
                None if motion.mode == AgentMode::Vehicle => {
                    assert_eq!(
                        motion.body_kind,
                        BodyKind::Box,
                        "a demand car keeps its box body"
                    );
                    car_bodies += 1;
                }
                None => {}
            }
        }
    }

    assert!(
        car_bodies > 0,
        "the passenger-car demand must still spawn cars"
    );

    // Both narrow templates are exercised. Their capsule lengths are disjoint
    // (bicycle 1.6–1.9 m, scooter 1.0–1.2 m), so a body in each interval proves
    // each authored mode template spawned.
    let lengths: Vec<f64> = narrow_ids
        .iter()
        .map(|&agent| {
            sim.agent_narrow_profile(agent)
                .expect("a narrow id carries a narrow profile")
                .length_m
        })
        .collect();
    assert!(
        lengths.iter().any(|&length| length >= 1.6 - 1e-9),
        "no bicycle-length capsule spawned: {lengths:?}"
    );
    assert!(
        lengths.iter().any(|&length| length <= 1.2 + 1e-9),
        "no scooter-length capsule spawned: {lengths:?}"
    );

    // The narrow modes moved through the shared stages rather than standing
    // still, and at least one completed its route and despawned.
    assert!(
        peak_narrow_speed > 1.0,
        "no narrow agent accelerated: peak {peak_narrow_speed}"
    );
    assert!(
        narrow_despawned,
        "no narrow agent completed its route and despawned"
    );
}
