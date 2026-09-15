//! Bounded single-body steering for route-relative wheeled agents.
//!
//! `PHASE_2_PLAN.md` "Continuous lateral motion" requires a wheeled agent's
//! lateral motion to be integrated under explicit limits and then projected
//! back, never snapped between lane centres in one decision tick. This module
//! is that integration step, and it is the source of the compiled
//! bounded-steering contract `docs/schema-v2-contract.md` *Increment 2
//! additions* fixes:
//!
//! - `|heading_rate| <= steering_rate_max_rad_s`;
//! - `|v * heading_rate| <= lateral_accel_max_mps2`;
//! - the Increment 1 turning-limit rule `|kappa| <= steering_rate_max_rad_s / v`
//!   (`kappa = heading_rate / v`, so it is the same bound as the first);
//! - the facility corridor boundary (`UsableLateralInterval`); and
//! - the lateral offset rate derived from the integrated heading,
//!   `d_dot = v * sin(theta_error)`.
//!
//! World pose is collision and output truth: a step integrates the heading and
//! the world position from the current pose, then projects the integrated
//! position back onto the compiled reference for tactical coordinates and
//! drift. No target offset is ever written to `d` or to a world position
//! directly, and a request whose step would leave the usable corridor is
//! rejected — the caller brakes or holds rather than clipping.
//!
//! # Model card
//!
//! This is a kinematic single-track-free steering step, not a calibrated
//! vehicle dynamics claim.
//!
//! ## State
//!
//! One step reads the agent's world position and heading, its longitudinal
//! speed, its route-relative target signed offset (in its own travel frame),
//! its travel direction sign, the facility's reference geometry, the compiled
//! [`SteeringLimits`], and the usable corridor; it writes an integrated
//! position, heading, heading rate, speed, and projected route coordinates.
//!
//! ## Bounds
//!
//! The command heading rate is the desired rate clamped to
//! `min(steering_rate_max_rad_s, lateral_accel_max_mps2 / max(v, v_floor))`, so
//! both compiled limits hold every step and a slow agent is bounded by the
//! heading rate rather than an unbounded lateral acceleration. The longitudinal
//! speed is taken as given: the caller has already applied the kernel's leader,
//! stop-line, and crossing-yield caps, and because the diagonal step advances
//! no further along the reference than `v * dt`, those caps still bound the same
//! proposed world step.
//!
//! ## Assumptions
//!
//! The reference is locally straight over one step, so the heading error is
//! measured against the reference tangent at the projected arc length. The
//! corridor is constant along the reference, as `CompiledFacility` fixes it.
//! The step is deterministic and pure: no wall-clock, RNG, or I/O.

use glam::DVec2;
use hekate_model::CompiledReferencePath;

/// Speed floor in metres per second used in the lateral-acceleration bound, so
/// a standstill does not make the bound unbounded.
pub const MIN_SPEED_FOR_LATERAL_BOUND_MPS: f64 = 0.5;

/// Lateral approach time constant in seconds.
///
/// The desired lateral rate toward the target offset is the offset error divided
/// by this constant, so the offset closes continuously over several steps and
/// never snaps to the target in one tick. The compiled motion limits clamp the
/// rate further.
pub const LATERAL_APPROACH_S: f64 = 2.0;

/// Tolerance in metres on the corridor test, so a step that lands on the
/// boundary by floating-point rounding is still accepted.
pub const CORRIDOR_TOLERANCE_M: f64 = 1e-9;

/// The compiled bounded-steering limits of one wheeled agent.
///
/// These are the mode's profile parameters `steering_rate_max_rad_s` and
/// `lateral_accel_max_mps2`; the module reads them, never a mode or template id.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteeringLimits {
    /// Maximum heading rate in radians per second.
    pub heading_rate_max_rad_s: f64,
    /// Maximum lateral acceleration in metres per second squared.
    pub lateral_accel_max_mps2: f64,
}

/// The signed lateral offsets a body may occupy on a facility, in the
/// facility's own reference frame (positive to the left of the reference
/// tangent).
///
/// This is the facility corridor `CompiledFacility::usable_lateral_interval`
/// resolves; the step mirrors it into the agent's own travel frame exactly as it
/// mirrors `d`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LateralCorridor {
    /// Lowest usable signed offset in metres.
    pub d_min: f64,
    /// Highest usable signed offset in metres.
    pub d_max: f64,
}

/// The compiled bounded-steering envelope of one wheeled steering agent: the
/// limits its heading rate obeys and the usable corridor its offset stays
/// inside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundedSteering {
    /// The mode's heading-rate and lateral-acceleration limits.
    pub limits: SteeringLimits,
    /// The signed offsets the body plus its lateral clearance may occupy.
    pub corridor: LateralCorridor,
}

/// The inputs one bounded steering step integrates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteeringRequest {
    /// Current world position in metres.
    pub position: DVec2,
    /// Current world heading in radians.
    pub heading_rad: f64,
    /// Current longitudinal speed in metres per second, after the kernel's caps.
    pub speed_mps: f64,
    /// Target signed lateral offset in metres, in the agent's own travel frame.
    pub target_offset_m: f64,
    /// Longitudinal travel sign: `1.0` forward, `-1.0` reverse.
    pub direction: f64,
}

/// One integrated bounded steering step: the reconstructed world pose and its
/// projection back onto the reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteeringStep {
    /// Integrated world position in metres.
    pub position: DVec2,
    /// Integrated world heading in radians, wrapped to `(-pi, pi]`.
    pub heading_rad: f64,
    /// The heading rate applied this step, within the compiled limits.
    pub heading_rate_rad_s: f64,
    /// The longitudinal speed used this step.
    pub speed_mps: f64,
    /// Reference arc length of the integrated pose.
    pub s_m: f64,
    /// Signed lateral offset of the integrated pose, in the agent travel frame.
    pub d_m: f64,
}

/// Integrate one bounded steering step toward `request.target_offset_m`.
///
/// The step is computed by steering the world heading toward the offset target,
/// clamping the heading rate to the compiled limits, integrating the heading and
/// the world position, and projecting the integrated position back onto
/// `geometry` for the route coordinates. It never writes the target offset to a
/// world position or to `d`.
///
/// Returns `None` when the integrated position would leave the usable corridor:
/// the request is infeasible and the caller brakes or holds. A speed at or below
/// zero makes no progress and holds the heading.
pub fn bounded_steering_step(
    geometry: &CompiledReferencePath,
    request: SteeringRequest,
    steering: BoundedSteering,
    dt: f64,
) -> Option<SteeringStep> {
    let direction = if request.direction < 0.0 { -1.0 } else { 1.0 };
    let speed_mps = request.speed_mps.max(0.0);

    // The reference tangent at the current projected arc length, in the agent's
    // own travel frame: a reverse traveller's forward reference points the
    // opposite way, and its offset is mirrored by the projection.
    let coordinate = geometry.project(request.position);
    let reference_heading = geometry.heading_at(coordinate.s());
    let agent_reference_heading = if direction > 0.0 {
        reference_heading
    } else {
        wrap_pi(reference_heading + std::f64::consts::PI)
    };

    let heading_rate_rad_s = if speed_mps > 0.0 {
        // First-order lateral approach: the desired offset rate closes the
        // remaining error over `LATERAL_APPROACH_S`. `d_dot = v * sin(theta)`,
        // so the desired heading error is `asin(d_dot / v)`.
        let d_m = coordinate.d() * direction;
        let desired_lateral_rate = (request.target_offset_m - d_m) / LATERAL_APPROACH_S;
        let desired_error = (desired_lateral_rate / speed_mps).clamp(-1.0, 1.0).asin();
        let desired_heading = wrap_pi(agent_reference_heading + desired_error);
        let error_rad = wrap_pi(desired_heading - request.heading_rad);

        // Both compiled limits: the heading-rate bound, and the lateral
        // acceleration bound which dominates at speed.
        let rate_cap = steering.limits.heading_rate_max_rad_s.min(
            steering.limits.lateral_accel_max_mps2 / speed_mps.max(MIN_SPEED_FOR_LATERAL_BOUND_MPS),
        );
        (error_rad / dt).clamp(-rate_cap, rate_cap)
    } else {
        0.0
    };

    let heading_rad = wrap_pi(request.heading_rad + heading_rate_rad_s * dt);
    let position = request.position + DVec2::from_angle(heading_rad) * (speed_mps * dt);

    let coordinate = geometry.project(position);
    let s_m = coordinate.s();
    let d_m = coordinate.d() * direction;

    // The corridor is expressed against the reference's own tangent, so it is
    // mirrored into the agent's travel frame exactly as `d` is.
    let (d_min, d_max) = if direction > 0.0 {
        (steering.corridor.d_min, steering.corridor.d_max)
    } else {
        (-steering.corridor.d_max, -steering.corridor.d_min)
    };
    if d_m < d_min - CORRIDOR_TOLERANCE_M || d_m > d_max + CORRIDOR_TOLERANCE_M {
        return None;
    }

    Some(SteeringStep {
        position,
        heading_rad,
        heading_rate_rad_s,
        speed_mps,
        s_m,
        d_m,
    })
}

/// Wrap an angle in radians to `(-pi, pi]`, so a heading stays canonical however
/// long a run is.
fn wrap_pi(angle_rad: f64) -> f64 {
    let mut wrapped = (angle_rad + std::f64::consts::PI) % std::f64::consts::TAU;
    if wrapped <= 0.0 {
        wrapped += std::f64::consts::TAU;
    }
    wrapped - std::f64::consts::PI
}
