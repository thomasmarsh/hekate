//! Increment 3: deterministic hitch/trailer-pose integration and admit wiring
//! for `AgentFamily::ArticulatedWheeled` (tas-12mx01cfsxskm0pjzq13hvcm2g).
//!
//! `Simulation::try_admit` now spawns a `tractor_semitrailer` mode template's
//! lead (tractor) segment exactly as it spawns a `WheeledBox`, and a small
//! dedicated per-tick pass (`crate::articulated`) drives every trailing
//! segment's pose from the lead's own just-integrated pose through its
//! authored hitch offset — see that module's doc comment for the derivation.
//! This suite proves the wiring and the kinematics against two fixtures, each
//! carrying the tractor-semitrailer as the only vehicle on its path (no
//! collision partner is needed to prove pose correctness):
//!
//! - `tractor_semitrailer_straight_v2.json5`: on a straight path the trailer's
//!   heading and offset behind the tractor are an *exact* closed form,
//!   independent of the tractor's speed profile (the derivation never assumes
//!   a particular speed), so every admitted agent at every tick is checked.
//! - `tractor_semitrailer_turning_v2.json5`: on an 80 m-radius constant-radius
//!   turn, the first-admitted (and therefore never-following, always
//!   free-flowing) agent's settled hitch angle is held to the classical
//!   single-axle towed-trailer steady-state off-tracking relation, `sin(phi) =
//!   L / R`. Every admitted agent, whatever its speed, is also held to the
//!   physical hitch-joint invariant (the trailer's kingpin coincides exactly
//!   with the tractor's rear reference point) and to the compiled 0.9 rad
//!   articulation limit.

use glam::DVec2;
use hekate_model::{BodyKind, CompiledScenario, parse_scenario_source_v2, validate_v2};
use hekate_sim::{AgentMode, Event, RunConfig, Simulation, SnapshotDetail};

/// The straight-path runtime fixture.
const STRAIGHT_FIXTURE: &str =
    include_str!("../../../scenarios/phase2/inc3/tractor_semitrailer_straight_v2.json5");

/// The constant-radius runtime fixture.
const TURNING_FIXTURE: &str =
    include_str!("../../../scenarios/phase2/inc3/tractor_semitrailer_turning_v2.json5");

/// Both fixtures' declared root seed.
const SEED: u64 = 0;

/// The tractor's authored length in metres, shared by both fixtures.
const LEAD_LENGTH_M: f64 = 6.0;
/// The semitrailer's authored length in metres, shared by both fixtures.
const TRAILER_LENGTH_M: f64 = 13.6;
/// The semitrailer's authored kingpin setback in metres, shared by both
/// fixtures.
const HITCH_OFFSET_M: f64 = 1.2;

/// Parse, validate, and compile a version-2 fixture.
fn compiled(source: &str) -> CompiledScenario {
    let document = parse_scenario_source_v2(source).expect("the fixture is a version-2 document");
    let diagnostics = validate_v2(&document);
    assert!(
        diagnostics.is_empty(),
        "the fixture must be a valid version-2 document, got {diagnostics:?}"
    );
    CompiledScenario::compile_v2(document).expect("the fixture compiles")
}

/// Wrap an angle in radians to `(-pi, pi]`, mirroring the kernel's own
/// convention (`crate::articulated::wrap_pi`, private to `hekate-sim`).
fn wrap_pi(angle_rad: f64) -> f64 {
    let mut wrapped = (angle_rad + std::f64::consts::PI) % std::f64::consts::TAU;
    if wrapped <= 0.0 {
        wrapped += std::f64::consts::TAU;
    }
    wrapped - std::f64::consts::PI
}

/// On a straight path, every trailing segment's heading equals the tractor's
/// exactly and its offset behind the tractor is an exact constant — a
/// property of the model's derivation alone, holding for any tractor speed
/// profile, so this checks every admitted agent at every tick rather than
/// isolating one.
#[test]
fn a_straight_tractor_semitrailer_keeps_its_trailer_exactly_aligned_and_offset() {
    let scenario = compiled(STRAIGHT_FIXTURE);
    let mut sim = Simulation::new(scenario, RunConfig::new(SEED)).expect("the simulation builds");

    const TICKS: u64 = 300;
    const HEADING_TOL_RAD: f64 = 1e-9;
    const OFFSET_TOL_M: f64 = 1e-6;
    // hitch_point(0) is length/2 behind the tractor's centre; the trailer's
    // centre sits `length_1/2 - hitch_offset_1` further behind that point.
    let expected_offset_m = LEAD_LENGTH_M / 2.0 + (TRAILER_LENGTH_M / 2.0 - HITCH_OFFSET_M);

    let mut observed = 0usize;
    for tick in 0..TICKS {
        sim.step();
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        for sample in snapshot.agents() {
            let motion = sample.motion.as_ref().expect("a full snapshot");
            if motion.mode != AgentMode::Vehicle || motion.body_kind != BodyKind::ArticulatedChain {
                continue;
            }
            assert_eq!(
                motion.segments.len(),
                2,
                "tick {tick}: a tractor-semitrailer chain reports two segment poses"
            );
            let lead = motion.segments[0];
            let trailer = motion.segments[1];
            assert_eq!(lead.position, sample.position);
            assert_eq!(lead.heading_rad, sample.heading_rad);

            assert!(
                lead.heading_rad.abs() < HEADING_TOL_RAD,
                "tick {tick}: the tractor left the straight path's zero heading: {}",
                lead.heading_rad
            );
            assert!(
                trailer.heading_rad.abs() < HEADING_TOL_RAD,
                "tick {tick}: the trailer's heading drifted off straight: {}",
                trailer.heading_rad
            );
            assert!(
                (trailer.position.x - (lead.position.x - expected_offset_m)).abs() < OFFSET_TOL_M,
                "tick {tick}: trailer.x={} lead.x={} expected offset {expected_offset_m} m",
                trailer.position.x,
                lead.position.x
            );
            assert!(
                trailer.position.y.abs() < OFFSET_TOL_M,
                "tick {tick}: the trailer drifted laterally off the straight path: {}",
                trailer.position.y
            );
            observed += 1;
        }
    }
    assert!(
        observed > 0,
        "the fixture must admit at least one tractor-semitrailer over {TICKS} ticks"
    );
}

/// On an 80 m-radius constant-radius turn, the first-admitted (always
/// free-flowing) tractor-semitrailer's settled hitch angle matches the
/// classical single-axle towed-trailer steady-state off-tracking relation, and
/// every admitted chain's physical hitch joint stays exact and inside the
/// compiled articulation limit.
#[test]
fn a_tractor_semitrailer_on_a_constant_radius_turn_settles_to_the_classical_off_tracking_angle() {
    let scenario = compiled(TURNING_FIXTURE);
    let mut sim = Simulation::new(scenario, RunConfig::new(SEED)).expect("the simulation builds");

    // 15 s of travel: the quarter-circle arc (~125.66 m at 8 m/s takes
    // ~15.7 s) is not completed, so the agent never exits, and ~120 m of
    // travel is ~9.7 arm-length time constants (`L / v` = 12.4 / 8 = 1.55 s
    // per 12.4 m), which settles the transient far below this test's
    // tolerance.
    const TICKS: u64 = 300;
    const RADIUS_M: f64 = 80.0;
    const TRAILER_ARM_M: f64 = TRAILER_LENGTH_M - HITCH_OFFSET_M;
    const ARTICULATION_LIMIT_RAD: f64 = 0.9;
    // The tolerance covers the chorded polyline's residual heading ripple
    // (attenuated, not eliminated, by the trailer's own relaxation), the
    // still-decaying transient at the tick this is checked, and floating-point
    // accumulation over 300 ticks. The measured residual is about 0.003 rad;
    // 0.01 rad leaves headroom without hiding a real regression in the model.
    const HITCH_ANGLE_TOLERANCE_RAD: f64 = 0.01;
    const JOINT_TOLERANCE_M: f64 = 1e-6;

    // The tractor's own exposed rear-reference point (`length/2` behind its
    // centre, along its heading) traces a circle of a *larger* radius than the
    // centreline: offsetting a fixed distance `e` along the tangent of a
    // circle of radius `R` puts the offset point on a concentric circle of
    // radius `sqrt(R^2 + e^2)` (Pythagoras on the tangent/radius right
    // triangle), rotating at the *same* angular rate as the centreline. See
    // `crates/hekate-sim/src/articulated.rs`'s module doc for why this is the
    // "driving" radius the classical `sin(phi) = L / R` relation reads.
    //
    // That relation gives the phase lag `phi` between the *direction of
    // travel* of this offset rear-reference point and the trailer's heading —
    // but this test (and the kernel's own jackknife check) measures the
    // articulation angle between the two **body headings** (tractor heading
    // minus trailer heading), which is what a real fifth-wheel angle means.
    // The rear-reference point's own direction of travel itself leads the
    // tractor's body heading by a constant `delta = atan2(length_0 / 2, R)`
    // (the same offset-circle geometry: a point offset along the tangent by
    // `e` on a circle of radius `R` has its own velocity direction rotated by
    // `atan2(e, R)` from the original tangent), so the settled body-heading
    // articulation angle is `delta + asin(L / R_hitch)`, not `asin(L /
    // R_hitch)` alone.
    let hitch_radius_m = RADIUS_M.hypot(LEAD_LENGTH_M / 2.0);
    let delta_rad = (LEAD_LENGTH_M / 2.0).atan2(RADIUS_M);
    let expected_hitch_angle_rad = delta_rad + (TRAILER_ARM_M / hitch_radius_m).asin();

    let mut settled_hitch_angle_rad = None;
    let mut jackknife_events = 0usize;
    let mut observed_any = false;
    for tick in 0..TICKS {
        let output = sim.step();
        jackknife_events += output
            .events()
            .iter()
            .filter(|event| matches!(event, Event::ArticulationLimitExceeded { .. }))
            .count();

        let snapshot = sim.snapshot(SnapshotDetail::Full);
        for sample in snapshot.agents() {
            let motion = sample.motion.as_ref().expect("a full snapshot");
            if motion.mode != AgentMode::Vehicle || motion.body_kind != BodyKind::ArticulatedChain {
                continue;
            }
            observed_any = true;
            assert_eq!(motion.segments.len(), 2);
            let lead = motion.segments[0];
            let trailer = motion.segments[1];

            // The physical joint: the trailer's own kingpin, `hitch_offset_m`
            // behind its front edge, must coincide exactly with the tractor's
            // rear reference point, for every admitted chain regardless of its
            // speed.
            let lead_rear =
                lead.position - DVec2::from_angle(lead.heading_rad) * (LEAD_LENGTH_M / 2.0);
            let kingpin = trailer.position
                + DVec2::from_angle(trailer.heading_rad)
                    * (TRAILER_LENGTH_M / 2.0 - HITCH_OFFSET_M);
            assert!(
                (kingpin - lead_rear).length() < JOINT_TOLERANCE_M,
                "tick {tick} agent {}: the hitch joint drifted apart by {} m",
                sample.id.get(),
                (kingpin - lead_rear).length()
            );

            let hitch_angle_rad = wrap_pi(lead.heading_rad - trailer.heading_rad).abs();
            assert!(
                hitch_angle_rad <= ARTICULATION_LIMIT_RAD,
                "tick {tick} agent {}: hitch angle {hitch_angle_rad} rad exceeded the compiled \
                 {ARTICULATION_LIMIT_RAD} rad limit",
                sample.id.get()
            );

            // Only the first-admitted agent (id 0) has no leader and is
            // therefore always free-flowing at the authored 8 m/s desired
            // speed, which the classical steady-state relation assumes.
            if sample.id.get() == 0 {
                settled_hitch_angle_rad = Some(hitch_angle_rad);
            }
        }
    }

    assert!(
        observed_any,
        "the fixture must admit at least one tractor-semitrailer over {TICKS} ticks"
    );
    assert_eq!(
        jackknife_events, 0,
        "an 80 m-radius turn must never trip the 0.9 rad articulation limit"
    );
    let settled = settled_hitch_angle_rad
        .expect("agent 0 must be admitted and observed at least once over {TICKS} ticks");
    assert!(
        (settled - expected_hitch_angle_rad).abs() < HITCH_ANGLE_TOLERANCE_RAD,
        "settled hitch angle {settled} rad is not within {HITCH_ANGLE_TOLERANCE_RAD} rad of the \
         classical steady-state off-tracking prediction {expected_hitch_angle_rad} rad (hitch \
         radius {hitch_radius_m} m, trailer arm {TRAILER_ARM_M} m)"
    );
}
