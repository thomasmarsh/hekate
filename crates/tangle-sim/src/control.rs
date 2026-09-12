//! Documented Intelligent Driver Model (IDM) longitudinal controller.
//!
//! # Model card
//!
//! This is the Phase 1 vehicle longitudinal model. It is a documented,
//! replaceable microscopic car-following model, not a calibrated scientific
//! claim. Source: Martin Treiber, Ansgar Hennecke, and Dirk Helbing, "Congested
//! Traffic States in Empirical Observations and Microscopic Simulations" (2000),
//! which introduces the IDM; see `VISION.md` and `PHASE_1_PLAN.md` Increment 2.
//!
//! ## Equations
//!
//! The commanded acceleration of a vehicle is
//!
//! ```text
//! a = a_max * [ 1 - (v / v0)^delta - sum_i (s*(v, dv_i) / gap_i)^2 ]
//! ```
//!
//! with the desired dynamic gap of each constraint `i`
//!
//! ```text
//! s*(v, dv) = s0_i + max(0, v * T + v * dv / (2 * sqrt(a_max * b)))
//! ```
//!
//! where:
//!
//! - `v` is the current speed in m/s;
//! - `v0` is the desired free-flow speed in m/s;
//! - `delta = 4` is the free-flow acceleration exponent;
//! - `a_max` is the maximum acceleration in m/s²;
//! - `b` is the comfortable deceleration in m/s²;
//! - `T` is the desired following time gap in seconds;
//! - for each constraint, `gap_i` is the clearance in metres and `dv = v - v_i`
//!   is the closing speed, with `v_i` the constraint's speed (`0` for a stop
//!   line);
//! - `s0_i` is the constraint's standstill gap in metres.
//!
//! Parameters `v0`, `T`, `a_max`, and `b` come from the vehicle's sampled
//! [`VehicleProfile`]; the controller never substitutes its own values for
//! them. The exponent `delta` and the leader standstill gap [`IDM_STANDSTILL_GAP_M`]
//! are model constants: they are properties of the model rather than of a
//! sampled vehicle.
//!
//! ## Bounds
//!
//! The raw acceleration is clamped to `[-b, +a_max]`, so the commanded
//! acceleration never exceeds the profile's maximum acceleration and the
//! commanded braking never exceeds the profile's comfortable deceleration.
//! The kernel additionally integrates speed within `[0, v0]`.
//!
//! ## Emergency backstops
//!
//! The profile bound above describes the IDM command only. The kernel adds two
//! position caps outside that clamp: the next speed may not pass the nearest
//! leader's rear in one step, and it may not pass a required stop line. When a
//! cap binds, the one-step deceleration it implies can exceed `b`, because the
//! cap answers a physical constraint (not crossing a bumper or a line) rather
//! than a comfort target. Each such step is counted in
//! `Simulation::emergency_cap_steps`, so a caller can assert that the backstop
//! stayed idle: the controlled car-following benchmark requires `0`, and a
//! signalized queue forming from free flow can legitimately engage it.
//!
//! ## Stop lines
//!
//! A stop line is modeled as a stationary constraint with `v_i = 0` and a
//! zero standstill gap, so the model wants the front bumper exactly at the
//! line. Because a bounded braking acceleration alone cannot guarantee an
//! exact rest position, the kernel also applies a position cap that keeps the
//! front bumper from crossing the line within one step. Reaching the rest
//! position is therefore the combination of IDM braking and the cap, not IDM
//! alone.
//!
//! ## Leader selection
//!
//! The kernel passes at most one leader constraint: the nearest live vehicle
//! ahead on the same guide path travelling the same direction, measured
//! bumper to bumper. Ties resolve to the lowest agent id (stable spawn order):
//! the kernel scans agents in ascending id order and replaces the current
//! leader only for a strictly smaller gap. Opposite-direction and crossing-path
//! interactions are later increments.

use crate::profile::VehicleProfile;

/// Free-flow acceleration exponent `delta` in the IDM equation.
pub(crate) const IDM_FREE_FLOW_EXPONENT: f64 = 4.0;

/// Standstill gap in metres the controller aims to keep behind a stopped
/// leader. A property of the model; the stop-line constraint uses `0.0`
/// instead and relies on the kernel's stop-line position cap.
pub(crate) const IDM_STANDSTILL_GAP_M: f64 = 2.0;

/// Lower bound on the gap divisor, so a touching or overlapping constraint
/// produces a very large (but finite) interaction rather than a division by
/// zero.
const GAP_FLOOR_M: f64 = 0.01;

/// One longitudinal constraint ahead of a vehicle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Constraint {
    /// Bumper-to-bumper clearance in metres.
    pub(crate) gap_m: f64,
    /// Speed of the constraint in metres per second; `0` for a stop line.
    pub(crate) speed_mps: f64,
    /// Standstill gap in metres this constraint keeps at rest.
    pub(crate) standstill_m: f64,
}

/// Commanded acceleration in m/s² for one vehicle under IDM.
///
/// `constraints` are the active leader and stop-line constraints; an empty
/// slice is free-flow driving. The result is clamped to
/// `[-comfortable_brake, +max_accel]`.
pub(crate) fn desired_acceleration(
    profile: &VehicleProfile,
    speed_mps: f64,
    constraints: &[Constraint],
) -> f64 {
    let a_max = profile.max_accel_mps2;
    let b = profile.comfortable_brake_mps2;
    let v = speed_mps.max(0.0);
    let v0 = profile.desired_speed_mps.max(f64::MIN_POSITIVE);

    let free = 1.0 - (v / v0).powi(IDM_FREE_FLOW_EXPONENT as i32);
    let mut interaction = 0.0;
    for constraint in constraints {
        interaction += interaction_term(profile, v, constraint, a_max, b);
    }

    (a_max * (free - interaction)).clamp(-b, a_max)
}

/// The `(s* / gap)^2` interaction term of one constraint.
fn interaction_term(
    profile: &VehicleProfile,
    speed_mps: f64,
    constraint: &Constraint,
    a_max: f64,
    b: f64,
) -> f64 {
    if !constraint.gap_m.is_finite() {
        return 0.0;
    }
    let gap = constraint.gap_m.max(GAP_FLOOR_M);
    let closing = speed_mps - constraint.speed_mps;
    let sqrt_ab = (a_max * b).sqrt();
    let dynamic = if sqrt_ab > 0.0 {
        speed_mps * profile.time_gap_s + speed_mps * closing / (2.0 * sqrt_ab)
    } else {
        speed_mps * profile.time_gap_s
    };
    let desired = constraint.standstill_m + dynamic.max(0.0);
    (desired / gap).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> VehicleProfile {
        VehicleProfile {
            desired_speed_mps: 10.0,
            length_m: 4.5,
            width_m: 1.8,
            time_gap_s: 1.5,
            max_accel_mps2: 2.0,
            comfortable_brake_mps2: 3.0,
            compliance: 1.0,
        }
    }

    fn leader(gap_m: f64, speed_mps: f64) -> Constraint {
        Constraint {
            gap_m,
            speed_mps,
            standstill_m: IDM_STANDSTILL_GAP_M,
        }
    }

    fn stop_line(gap_m: f64) -> Constraint {
        Constraint {
            gap_m,
            speed_mps: 0.0,
            standstill_m: 0.0,
        }
    }

    #[test]
    fn free_flow_converges_to_the_desired_speed() {
        let profile = profile();
        // Below the desired speed the model accelerates.
        assert!(desired_acceleration(&profile, 5.0, &[]) > 0.0);
        // At the desired speed the free-flow term is zero.
        assert!(desired_acceleration(&profile, 10.0, &[]).abs() < 1e-12);
    }

    #[test]
    fn the_commanded_acceleration_stays_within_profile_bounds() {
        let profile = profile();
        // A closing vehicle almost touching a stopped leader demands the most
        // braking the model can express, clamped to the comfortable value.
        let tight = [leader(0.01, 0.0)];
        let a = desired_acceleration(&profile, 15.0, &tight);
        assert!((a + profile.comfortable_brake_mps2).abs() < 1e-9);
        // Full free flow cannot exceed the maximum acceleration.
        let a = desired_acceleration(&profile, 0.0, &[]);
        assert!((a - profile.max_accel_mps2).abs() < 1e-9);
    }

    #[test]
    fn a_larger_gap_demands_less_braking() {
        let profile = profile();
        // At 10 m/s the desired dynamic gap is tens of metres, so a 30 m gap is
        // still saturating the braking clamp while a 120 m gap is nearly free.
        let near = desired_acceleration(&profile, 10.0, &[leader(30.0, 0.0)]);
        let far = desired_acceleration(&profile, 10.0, &[leader(120.0, 0.0)]);
        assert!(near < far, "near {near} should brake harder than far {far}");
    }

    #[test]
    fn a_stop_line_brakes_a_moving_vehicle() {
        let profile = profile();
        // Approaching the line at speed, the zero-standstill stop-line
        // interaction dominates and commands braking.
        assert!(desired_acceleration(&profile, 3.0, &[stop_line(1.0)]) < 0.0);
        // At rest exactly at the line the interaction is zero; the kernel's
        // stop-line position cap, not this acceleration, holds the bumper.
        assert!(desired_acceleration(&profile, 0.0, &[stop_line(0.0)]) > 0.0);
        // Far from the line at rest, the model accelerates freely.
        assert!(desired_acceleration(&profile, 0.0, &[stop_line(500.0)]) > 0.0);
    }

    #[test]
    fn constraints_do_not_change_profile_parameters() {
        let profile = profile();
        // The command is a pure function of the sampled profile and the
        // constraints; no module-level parameter shadows the profile.
        let first = desired_acceleration(&profile, 7.0, &[leader(12.0, 4.0)]);
        let second = desired_acceleration(&profile, 7.0, &[leader(12.0, 4.0)]);
        assert_eq!(first, second);
    }
}
