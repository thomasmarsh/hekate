//! TAS-080: one narrow agent's longitudinal tactics along a compiled facility.
//!
//! The checked-in narrow fixture (`fixtures/narrow_longitudinal_v2.json5`)
//! authors one narrow mode on a compiled facility whose reference path is the
//! route its demand selects, a leader in front of it, an authored stop line,
//! and a fixed-time signal. This test drives that scenario and proves the
//! `TAS-080` `Done when`: command envelopes and body bounds hold for a narrow
//! agent as it accelerates, follows a leader, holds the stop line, yields to
//! the red head, and completes its route along the compiled facility — all
//! through the shared stages, with no mode name in the kernel.

use std::collections::BTreeMap;

use hekate_model::{
    AgentFamily, BodyKind, CompiledScenario, DemandId, MovementId, PathId, SignalColor,
    parse_scenario_source_v2,
};
use hekate_sim::{
    AgentId, DEFAULT_STEP, DespawnReason, Event, RunConfig, SignalAction, Simulation,
    SnapshotDetail,
};

/// The checked-in TAS-080 narrow longitudinal fixture.
const FIXTURE: &str = include_str!("fixtures/narrow_longitudinal_v2.json5");

/// Fixed seed; the fixture's constant ranges make the sampled profile
/// seed-independent, and this seed fixes the demand arrival schedule.
const SEED: u64 = 20_260_913;

/// 60 s at the default 0.05 s step: the 12 s red hold, the 120 s green, and a
/// queue that clears the 80 m facility route well inside this window.
const TICKS: u64 = 1200;

/// The largest front-to-rear gap at which a narrow agent counts as following
/// its leader rather than riding free.
const FOLLOWING_GAP_M: f64 = 8.0;

/// One live narrow agent as read from a full snapshot, with everything the
/// behavior and envelope assertions need.
struct NarrowObservation {
    path: PathId,
    distance_m: f64,
    speed_mps: f64,
    body_length_m: f64,
    profile: hekate_sim::NarrowProfile,
}

#[test]
fn a_narrow_agent_accelerates_follows_holds_yields_and_completes_a_compiled_facility_route() {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    let scenario = CompiledScenario::compile_v2(source).expect("the narrow scenario compiles");

    // Compiled facility route selection: the demand names a narrow wheeled
    // template, and the route that template's demand selects is a compiled
    // facility's reference path.
    let mode_id = scenario
        .demand_mode(DemandId::from_index(0))
        .expect("the demand names a mode template");
    let template = scenario
        .mode_template(mode_id)
        .expect("the mode template compiles");
    assert_eq!(
        template.family(),
        Some(AgentFamily::WheeledCapsule),
        "the fixture drives the narrow wheeled family"
    );
    let facility = scenario
        .facilities()
        .iter()
        .find(|facility| facility.permits_mode(mode_id))
        .expect("a compiled facility permits the narrow mode");
    let reference_path = facility
        .reference_path()
        .expect("the compiled facility has a reference path");
    let route = scenario
        .movement(MovementId::from_index(0))
        .expect("the demand route compiles");
    assert_eq!(
        route.path(),
        reference_path,
        "the route the demand selects is the compiled facility's reference path"
    );
    let route_length_m = facility.length().expect("the reference path has a length");
    let reference = facility
        .reference()
        .expect("the facility carries a reference path")
        .geometry()
        .clone();
    let stop_line_m = route.stop_line_m();

    let mut sim = Simulation::new(scenario, RunConfig::new(SEED)).expect("the simulation builds");

    let mut previous_speed: BTreeMap<AgentId, f64> = BTreeMap::new();
    let mut narrow_seen = 0usize;
    let mut accelerated = false;
    let mut followed = false;
    let mut held_stop_line = false;
    let mut yielded = false;
    let mut completed = false;

    for _ in 0..TICKS {
        let events = sim.step().events().to_vec();
        for event in &events {
            if let Event::Despawned {
                agent,
                path,
                reason,
            } = event
                && sim.agent_narrow_profile(*agent).is_some()
            {
                assert_eq!(*reason, DespawnReason::ExitedPath);
                assert_eq!(
                    *path, reference_path,
                    "the narrow agent completed the compiled facility route"
                );
                completed = true;
            }
        }

        let mut observations: Vec<NarrowObservation> = Vec::new();
        for sample in sim.snapshot(SnapshotDetail::Full).agents() {
            let Some(profile) = sim.agent_narrow_profile(sample.id) else {
                continue;
            };
            let motion = sample.motion.as_ref().expect("full detail");

            // Body bounds: the capsule body matches the sampled narrow profile.
            assert_eq!(motion.body_kind, BodyKind::Capsule);
            assert!((motion.body_length_m - profile.length_m).abs() < 1e-12);
            assert!((motion.body_width_m - profile.radius_m * 2.0).abs() < 1e-12);

            // Command envelope: the integrated speed stays within [0, v0] and
            // the implied acceleration stays within the profile's model bounds.
            assert!(
                motion.speed_mps >= -1e-9 && motion.speed_mps <= profile.desired_speed_mps + 1e-9,
                "narrow speed {} outside [0, {}]",
                motion.speed_mps,
                profile.desired_speed_mps
            );
            if let Some(&previous) = previous_speed.get(&sample.id) {
                let acceleration = (motion.speed_mps - previous) / DEFAULT_STEP.as_secs();
                if acceleration >= 0.0 {
                    assert!(
                        acceleration <= profile.max_accel_mps2 + 1e-9,
                        "narrow acceleration {acceleration} exceeds {} m/s^2",
                        profile.max_accel_mps2
                    );
                    if acceleration > 1e-9 {
                        accelerated = true;
                    }
                } else {
                    assert!(
                        acceleration >= -profile.comfortable_brake_mps2 - 1e-9,
                        "narrow deceleration {acceleration} exceeds {} m/s^2",
                        profile.comfortable_brake_mps2
                    );
                }
            }

            // The agent follows the compiled facility's reference geometry: its
            // world position projects onto the reference path with zero lateral
            // offset and the arc length it reports.
            let projected = reference.project(sample.position);
            assert!(
                projected.d().abs() < 1e-6,
                "narrow agent left the facility reference path by {} m",
                projected.d()
            );
            assert!(
                (projected.s() - motion.path_distance_m).abs() < 1e-6,
                "narrow agent's reference arc {} disagrees with its path distance {}",
                projected.s(),
                motion.path_distance_m
            );

            let front = motion.path_distance_m + profile.length_m * 0.5;
            if motion.speed_mps < 0.25 && (front - stop_line_m).abs() <= 0.5 {
                held_stop_line = true;
            }
            if let Some(decision) = motion.decision
                && decision.action == SignalAction::Stop
                && decision.color != SignalColor::Green
            {
                yielded = true;
            }

            observations.push(NarrowObservation {
                path: motion.path,
                distance_m: motion.path_distance_m,
                speed_mps: motion.speed_mps,
                body_length_m: profile.length_m,
                profile,
            });
            previous_speed.insert(sample.id, motion.speed_mps);
        }
        narrow_seen = narrow_seen.max(observations.len());

        // Following a leader: two narrow agents on the same path where the
        // follower rides a small gap behind the leader and is constrained below
        // its own desired speed. Bodies never overlap.
        observations.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));
        for pair in observations.windows(2) {
            let (follower, leader) = (&pair[0], &pair[1]);
            if follower.path != leader.path {
                continue;
            }
            let gap = leader.distance_m
                - leader.body_length_m * 0.5
                - (follower.distance_m + follower.body_length_m * 0.5);
            assert!(gap >= -1e-6, "narrow bodies overlap: gap {gap} m");
            if gap <= FOLLOWING_GAP_M
                && follower.speed_mps <= leader.speed_mps + 1e-9
                && follower.speed_mps < follower.profile.desired_speed_mps
            {
                followed = true;
            }
        }
    }

    assert!(
        narrow_seen >= 2,
        "the fixture must queue a narrow follower behind a narrow leader, saw {narrow_seen}"
    );
    assert!(accelerated, "no narrow agent accelerated");
    assert!(followed, "no narrow agent followed a leader");
    assert!(
        held_stop_line,
        "no narrow agent came to rest at the stop line {stop_line_m} m"
    );
    assert!(yielded, "no narrow agent yielded to the signal");
    assert!(
        completed,
        "no narrow agent completed the {route_length_m} m facility route"
    );

    // The comfortable-braking envelope held without the kernel's anti-overlap
    // backstop ever braking harder than the model bound.
    assert_eq!(
        sim.emergency_cap_steps(),
        0,
        "the command envelope must hold without the emergency cap binding"
    );
}
