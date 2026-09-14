//! TAS-089: bounded single-body steering for route-relative wheeled agents.
//!
//! A lateral target offset is integrated as a bounded heading and world step and
//! then projected back onto the reference — never snapped between lane centres
//! in one decision tick. These fixtures assert the compiled per-step limits
//! (`|heading_rate| <= steering_rate_max_rad_s` and
//! `|v * heading_rate| <= lateral_accel_max_mps2`), continuous displacement, the
//! projection back (drift), the mirrored reverse travel frame, and that an
//! out-of-corridor request holds instead of clipping.
//!
//! The pipeline seam that carries this step as a `MotionCommand` and integrates
//! it in `Simulation::advance_physics` is exercised by the crate-internal test
//! `a_bounded_steering_request_integrates_in_world_and_reprojects_without_snapping`
//! in `crate::sim`, which can reach the crate-internal route state.

use glam::DVec2;
use tangle_model::CompiledReferencePath;
use tangle_sim::{
    BoundedSteering, LateralCorridor, SteeringLimits, SteeringRequest, bounded_steering_step,
};

/// The fixed step of the fixtures, seconds.
const DT: f64 = 0.05;

/// The sampled speed of the forward and reverse fixtures, metres per second.
const SPEED_MPS: f64 = 6.0;

/// The fixture's compiled bounded-steering limits: the `rider` template's
/// authored values.
fn limits() -> SteeringLimits {
    SteeringLimits {
        heading_rate_max_rad_s: 0.9,
        lateral_accel_max_mps2: 2.0,
    }
}

/// A straight reference along the world x axis, so the left normal is `+y`.
fn straight() -> CompiledReferencePath {
    CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(200.0, 0.0)])
}

/// A bounded-steering envelope with a symmetric corridor of the given half
/// width and the fixture limits.
fn steering(corridor_half_m: f64) -> BoundedSteering {
    BoundedSteering {
        limits: limits(),
        corridor: LateralCorridor {
            d_min: -corridor_half_m,
            d_max: corridor_half_m,
        },
    }
}

/// Integrate one step and advance the request to the integrated pose, so a
/// fixture can walk a multi-tick lane change.
fn next(
    geometry: &CompiledReferencePath,
    request: &mut SteeringRequest,
    steering: BoundedSteering,
) -> Option<tangle_sim::SteeringStep> {
    let step = bounded_steering_step(geometry, *request, steering, DT)?;
    request.position = step.position;
    request.heading_rad = step.heading_rad;
    Some(step)
}

/// Every returned step obeys both compiled limits and the step's own heading
/// integration.
fn assert_within_limits(step: &tangle_sim::SteeringStep, limits: SteeringLimits) {
    assert!(
        step.heading_rate_rad_s.abs() <= limits.heading_rate_max_rad_s + 1e-12,
        "heading rate {} exceeds the limit {}",
        step.heading_rate_rad_s,
        limits.heading_rate_max_rad_s
    );
    assert!(
        step.speed_mps * step.heading_rate_rad_s.abs() <= limits.lateral_accel_max_mps2 + 1e-9,
        "lateral acceleration {} exceeds the limit {}",
        step.speed_mps * step.heading_rate_rad_s.abs(),
        limits.lateral_accel_max_mps2
    );
}

#[test]
fn a_straight_step_obeys_the_compiled_limits_and_moves_toward_the_target() {
    let geometry = straight();
    let request = SteeringRequest {
        position: DVec2::ZERO,
        heading_rad: 0.0,
        speed_mps: SPEED_MPS,
        target_offset_m: 1.0,
        direction: 1.0,
    };
    let step =
        bounded_steering_step(&geometry, request, steering(1.0), DT).expect("inside the corridor");

    assert_within_limits(&step, limits());
    // The heading integrates by exactly the bounded rate over the step.
    assert!((step.heading_rad - step.heading_rate_rad_s * DT).abs() < 1e-12);
    // The world displacement has the commanded speed's magnitude, so the step
    // is continuous and does not teleport.
    assert!((step.position.length() - request.speed_mps * DT).abs() < 1e-9);
    // It moved toward the target but did not reach it in one tick.
    assert!(step.d_m > 0.0);
    assert!(step.d_m < request.target_offset_m);
    assert!(step.s_m > 0.0);
}

#[test]
fn a_straight_request_never_snaps_to_the_target_in_one_tick() {
    let geometry = straight();
    let envelope = steering(1.0);
    let target = 1.0;
    let mut request = SteeringRequest {
        position: DVec2::ZERO,
        heading_rad: 0.0,
        speed_mps: SPEED_MPS,
        target_offset_m: target,
        direction: 1.0,
    };

    let first = next(&geometry, &mut request, envelope).expect("inside the corridor");
    assert!(first.d_m < 0.1, "the first tick moved {} m", first.d_m);

    let mut previous_d = first.d_m;
    let mut ticks = 1;
    let mut converged = false;
    while ticks < 1000 {
        let step = next(&geometry, &mut request, envelope).expect("inside the corridor");
        // The lateral offset changes continuously and monotonically: it advances
        // by at most the speed's full step, and never regresses.
        assert!(step.d_m >= previous_d - 1e-12, "offset regressed");
        assert!(
            step.d_m - previous_d <= SPEED_MPS * DT + 1e-9,
            "offset jumped {} m in one tick",
            step.d_m - previous_d
        );
        previous_d = step.d_m;
        ticks += 1;
        if step.d_m >= target - 0.05 {
            converged = true;
            break;
        }
    }
    assert!(
        converged,
        "the bounded request converged, at {previous_d} m"
    );
    assert!(
        ticks > 10,
        "a lane-centre change takes many bounded ticks, not {ticks}"
    );
}

#[test]
fn a_curved_reference_advances_progress_continuously_and_projects_back() {
    // A quarter circle of radius 50 m; the entry pose is taken from the
    // reference so the fixture is independent of the arc's parameterization.
    let geometry = CompiledReferencePath::arc(DVec2::ZERO, 50.0, 0.0, std::f64::consts::FRAC_PI_2);
    let envelope = steering(1.0);
    let mut request = SteeringRequest {
        position: geometry.position_at(0.0),
        heading_rad: geometry.heading_at(0.0),
        speed_mps: SPEED_MPS,
        target_offset_m: 0.5,
        direction: 1.0,
    };
    let entry = geometry.project(request.position);
    let mut previous_s = entry.s();
    let mut previous_d = entry.d();

    // 200 steps is 60 m along the 78.5 m arc, so the fixture stays on the
    // reference; the reference end is a longitudinal connector concern, not a
    // steering-step one.
    for _ in 0..200 {
        let step =
            bounded_steering_step(&geometry, request, envelope, DT).expect("inside the corridor");
        assert_within_limits(&step, limits());

        // The reported route coordinates are exactly the projection of the
        // integrated world pose: the drift check reads the world truth.
        let coordinate = geometry.project(step.position);
        assert!((coordinate.s() - step.s_m).abs() < 1e-9);
        assert!((coordinate.d() - step.d_m).abs() < 1e-9);

        // Progress is continuous and never regresses. On a curved reference the
        // arc-length advance of the projected chord differs from the chord
        // length by the curve's small geometric term (`d * kappa`), so the bound
        // carries that margin.
        assert!(step.s_m > previous_s);
        assert!(step.s_m - previous_s <= SPEED_MPS * DT + 1e-2);
        // The offset approaches the target monotonically.
        assert!(step.d_m >= previous_d - 1e-12);

        // Projection drift: the arc-length advance is the world displacement
        // projected onto the local reference tangent, within the step's turn.
        let theta_error = step.heading_rad - geometry.heading_at(previous_s);
        let tangential = SPEED_MPS * DT * theta_error.cos();
        assert!(
            (step.s_m - previous_s - tangential).abs() < 2e-2,
            "projection drift {} exceeded the tolerance",
            step.s_m - previous_s - tangential
        );

        request.position = step.position;
        request.heading_rad = step.heading_rad;
        previous_s = step.s_m;
        previous_d = step.d_m;
    }
    assert!(
        previous_d > 0.0,
        "the curved fixture steered toward its target"
    );
}

#[test]
fn a_reverse_step_mirrors_the_offset_frame_and_advances_backward() {
    let geometry = straight();
    let envelope = steering(1.0);
    let mut request = SteeringRequest {
        position: DVec2::new(150.0, 0.0),
        heading_rad: std::f64::consts::PI,
        speed_mps: SPEED_MPS,
        target_offset_m: 0.5,
        direction: -1.0,
    };
    let mut previous_d = 0.0;
    for _ in 0..400 {
        let step =
            bounded_steering_step(&geometry, request, envelope, DT).expect("inside the corridor");
        assert_within_limits(&step, limits());
        // Reverse travel advances toward the path start.
        assert!(step.position.x < request.position.x);
        // A positive agent-frame offset is to the reference's right when
        // travelling in reverse, so the body moves to negative world y.
        assert!(step.position.y < 0.0);
        // The offset still advances continuously toward the target.
        assert!(step.d_m >= previous_d - 1e-12);
        assert!(step.d_m - previous_d <= SPEED_MPS * DT + 1e-9);
        previous_d = step.d_m;
        request.position = step.position;
        request.heading_rad = step.heading_rad;
    }
    assert!(
        (previous_d - 0.5).abs() < 0.05,
        "reverse offset {previous_d} m"
    );
}

#[test]
fn a_request_that_would_leave_the_corridor_holds_instead_of_clipping() {
    let geometry = straight();
    let envelope = steering(0.2);
    let mut request = SteeringRequest {
        position: DVec2::ZERO,
        heading_rad: 0.0,
        speed_mps: SPEED_MPS,
        target_offset_m: 1.0,
        direction: 1.0,
    };

    let mut last_d = 0.0;
    let mut held = false;
    for _ in 0..200 {
        match bounded_steering_step(&geometry, request, envelope, DT) {
            Some(step) => {
                // A returned step never clips the corridor boundary.
                assert!(step.d_m <= 0.2 + 1e-9, "a step clipped to {} m", step.d_m);
                last_d = step.d_m;
                request.position = step.position;
                request.heading_rad = step.heading_rad;
            }
            None => {
                held = true;
                break;
            }
        }
    }
    assert!(held, "an out-of-corridor request is rejected, not clipped");
    assert!(
        last_d > 0.0,
        "the feasible steps made progress before the hold"
    );
    assert!(last_d <= 0.2 + 1e-9);
}

#[test]
fn the_more_restrictive_of_the_rate_and_lateral_acceleration_limits_binds() {
    let geometry = straight();
    let envelope = steering(2.0);
    let request = |speed_mps: f64| SteeringRequest {
        position: DVec2::ZERO,
        heading_rad: 0.0,
        speed_mps,
        target_offset_m: 1.0,
        direction: 1.0,
    };

    // At 20 m/s the lateral-acceleration bound dominates: 2 / 20 = 0.1 rad/s.
    let fast =
        bounded_steering_step(&geometry, request(20.0), envelope, DT).expect("inside the corridor");
    assert!((fast.heading_rate_rad_s - 0.1).abs() < 1e-9);

    // At 0.6 m/s the heading-rate bound dominates instead.
    let slow =
        bounded_steering_step(&geometry, request(0.6), envelope, DT).expect("inside the corridor");
    assert!((slow.heading_rate_rad_s - 0.9).abs() < 1e-9);
}

#[test]
fn a_zero_speed_request_holds_the_pose() {
    let geometry = straight();
    let request = SteeringRequest {
        position: DVec2::new(10.0, 0.3),
        heading_rad: 0.0,
        speed_mps: 0.0,
        target_offset_m: 1.0,
        direction: 1.0,
    };
    let step =
        bounded_steering_step(&geometry, request, steering(1.0), DT).expect("inside the corridor");
    assert_eq!(step.heading_rate_rad_s, 0.0);
    assert_eq!(step.position, request.position);
    assert_eq!(step.heading_rad, request.heading_rad);
}
