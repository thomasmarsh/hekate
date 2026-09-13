//! Documented Intelligent Driver Model (IDM) longitudinal controller.
//!
//! # Model card
//!
//! This is the Phase 1 vehicle longitudinal model. It is a documented,
//! replaceable microscopic car-following model, not a calibrated scientific
//! claim. Source: Martin Treiber, Ansgar Hennecke, and Dirk Helbing, "Congested
//! Traffic States in Empirical Observations and Microscopic Simulations" (2000),
//! which introduces the IDM; see `VISION.md` and `PHASE_1_PLAN.md` Increment 2.
//! The kernel reaches it only through
//! [`crate::controller::VehicleController`], so the model is replaceable
//! without editing any interaction logic.
//!
//! ## State
//!
//! The controller's state is one vehicle's longitudinal state as the kernel
//! holds it: the current speed `v` in m/s along the guide path, the path
//! progress that fixes its position and its front-bumper progress, and the body
//! length that turns a centre-to-centre distance into a bumper-to-bumper gap.
//! Heading and lateral position follow the path: this model steers nothing,
//! and Phase 1 has no lane changing or lateral negotiation. Its full output is
//! one commanded acceleration.
//!
//! ## Parameters
//!
//! Parameters are sampled per vehicle from the scenario's `profiles` envelope
//! and travel with the agent ([`VehicleProfile`]):
//!
//! - `desired_speed_mps` is `v0`, the free-flow speed;
//! - `time_gap_s` is `T`, the desired following time gap;
//! - `max_accel_mps2` is `a_max`, the maximum acceleration;
//! - `comfortable_brake_mps2` is `b`, the comfortable deceleration;
//! - `length_m` and `width_m` size the body box;
//! - `compliance` belongs to the signal-compliance decision
//!   ([`crate::compliance`]), not to this longitudinal model.
//!
//! The controller never substitutes its own values for a parameter, so a
//! sampled profile fully determines the command.
//!
//! ## Constants
//!
//! Model properties, not sampled: the free-flow acceleration exponent `delta`
//! ([`IDM_FREE_FLOW_EXPONENT`]), the leader standstill gap
//! [`IDM_STANDSTILL_GAP_M`] used when the kernel builds a leader constraint,
//! and [`GAP_FLOOR_M`], the floor on the gap divisor that keeps a touching or
//! overlapping constraint finite instead of dividing by zero.
//!
//! ## Decision inputs
//!
//! The kernel selects the constraints and passes at most three, each as a gap
//! in metres, a constraint speed in m/s, and a standstill gap in metres:
//!
//! - the nearest leader: a live vehicle ahead on the same guide path travelling
//!   the same direction, measured bumper to bumper, with the leader's own
//!   standstill gap [`IDM_STANDSTILL_GAP_M`];
//! - a required stop line: stationary (`v_i = 0`) with a zero standstill gap,
//!   present only while the recorded signal-compliance decision is to stop;
//! - an occupied crossing the vehicle is obliged to yield to: stationary with a
//!   zero standstill gap, present only while a pedestrian body overlaps the
//!   crossing region and the vehicle's front bumper is still upstream.
//!
//! An empty list is free-flow driving. The model sees only these constraints,
//! the sampled profile, and the current speed; the kernel owns which of them
//! exist, and the position caps in [Emergency backstop](#emergency-backstop)
//! are applied outside the model.
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
//! ## Bounds
//!
//! The raw acceleration is clamped to `[-b, +a_max]`, so the commanded
//! acceleration never exceeds the profile's maximum acceleration and the
//! commanded braking never exceeds the profile's comfortable deceleration.
//! The kernel additionally integrates speed within `[0, v0]`.
//!
//! ## Tie-breaks
//!
//! The kernel passes at most one leader constraint, so the model itself never
//! chooses between candidates. The kernel chooses deterministically: it scans
//! live agents in ascending [`crate::AgentId`] order and replaces the current
//! leader only for a strictly smaller gap, so two candidates at exactly equal
//! gaps resolve to the lowest agent id, and equal positions therefore resolve
//! by stable spawn order. Same-direction status is required, so an
//! opposite-direction body on the same path is not a constraint for this
//! model; opposite-direction and crossing-path interaction is later work.
//!
//! ## Emergency backstop
//!
//! The profile bound above describes the IDM command only. The kernel adds
//! three position caps outside that clamp: the next speed may not pass the
//! nearest leader's rear in one step, it may not pass a required stop line, and
//! it may not pass a vehicle's yield stop point short of an occupied crossing.
//! When a cap binds, the one-step deceleration it implies can exceed `b`,
//! because the cap answers a physical constraint (not crossing a bumper or a
//! line) rather than a comfort target. Each such step is counted in
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
//! ## Crossing yields
//!
//! A vehicle obliged by an authored `yield` rule yields to the crossing its
//! movement crosses while a pedestrian body occupies the crossing region. The
//! occupied crossing is modeled exactly like a stop line: a stationary
//! constraint with `v_i = 0` and a zero standstill gap, plus the kernel's
//! position cap that holds the front bumper at the crossing entry. Yielding
//! therefore reuses IDM's bounded braking rather than an emergency stop: in the
//! normal regime the occupancy appears while the vehicle is still far enough
//! away for the comfortable deceleration to bring it to rest at the entry, and
//! the position cap binds only when a body enters the region inside the
//! vehicle's braking distance. The crossing entry, its region, and the movement
//! it crosses all come from the shared scenario representation, so the rule is
//! scenario data rather than a controller branch.
//!
//! ## Assumptions
//!
//! The model assumes one longitudinal degree of freedom: the vehicle is a point
//! mass on a fixed guide path, so it steers nothing and has no lateral state.
//! It assumes the sampled profile fully describes the driver and is fixed for
//! the run, with no learning, memory, or per-vehicle variation. It assumes the
//! kernel supplies every interaction constraint and every position cap listed
//! above; IDM itself never guarantees an exact rest position at a stop line, and
//! the only longitudinal interactions in Phase 1 are the leader, the required
//! stop line, and the occupied crossing of [Decision inputs](#decision-inputs).
//! Nothing here is calibrated against observed trajectories.
//!
//! ## Parameter sources
//!
//! Every parameter is authored scenario data, sampled once per vehicle from the
//! scenario's `profiles` envelope and never fitted to observations. The Phase 1
//! parser defaults in `ProfileSource` are provisional engineering values
//! (desired speed 9–15 m/s, length 4.0–5.2 m, width 1.7–2.0 m, time gap 1.0–2.0
//! s, maximum acceleration 1.2–2.5 m/s², comfortable deceleration 2.0–3.5 m/s²),
//! and every checked-in scenario restates its envelope explicitly rather than
//! relying on that default. The IDM constants — the exponent `delta`, the
//! standstill gap, and the gap floor — are model properties, not sampled values.
//!
//! ## Validated ranges
//!
//! Evidence covers the Phase 1 vehicle fixtures: the mixed-profile
//! `car_following_v1` corridor and the signalized, pedestrian-crossing, and
//! mixed-interaction benchmarks, at the sampled envelope above (desired speed
//! 9–15 m/s) and the Fast, Standard, and Fine steps. The bounded model is kept
//! collision-free by the kernel's position caps, not by a collision resolver.
//! Envelopes, densities, and speeds outside those fixtures are unvalidated: the
//! model has no evidence there, and its output should be labelled rather than
//! treated as credible.
//!
//! ## Known failure modes
//!
//! - The raw acceleration is clamped to `[-b, +a_max]`, so a constraint closer
//!   than the comfortable braking distance saturates the command and IDM alone
//!   cannot guarantee a stop; the kernel's position caps, not this model, keep
//!   bodies from overlapping.
//! - With `a_max * b == 0` the interaction term drops the closing-speed
//!   contribution, and the desired dynamic gap loses its `dv` term.
//! - A non-positive or non-finite desired speed is floored to
//!   `f64::MIN_POSITIVE`, and a non-finite gap contributes no interaction, so a
//!   malformed parameter or constraint is silently absorbed rather than
//!   rejected.
//! - The model has no lateral or steering state, so it cannot represent lane
//!   changing, passing, or wrong-way movement, and it treats only same-direction
//!   leaders as constraints.
//!
//! ## Incompatible fidelity settings
//!
//! The IDM takes no fidelity parameter: `a_max`, `b`, `T`, and `delta` are
//! properties of the model, not of the step, so Fast, Standard, and Fine produce
//! the same command from the same state and the model is compatible with all
//! three Phase 1 presets. It is incompatible with any preset that disables the
//! kernel's position caps or that requires lateral or steering state, which it
//! does not express. The kernel resolves the step; see [`crate::RunConfig`].

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
/// `constraints` are the active leader, stop-line, and occupied-crossing
/// constraints; an empty slice is free-flow driving. The result is clamped to
/// `[-comfortable_brake, +max_accel]`. The kernel applies the position caps
/// outside this command; see the module model card.
pub(crate) fn desired_acceleration(
    profile: &VehicleProfile,
    speed_mps: f64,
    constraints: &[Constraint],
) -> f64 {
    idm_acceleration(
        profile.max_accel_mps2,
        profile.comfortable_brake_mps2,
        profile.desired_speed_mps,
        profile.time_gap_s,
        speed_mps,
        constraints,
    )
}

/// Commanded acceleration in m/s² for one IDM parameter set.
///
/// This is the IDM law itself, parameterized by `a_max`, `b`, `v0`, and `T`
/// rather than by a [`VehicleProfile`], so the narrow wheeled family
/// ([`crate::narrow`]) reaches the same law under its own sampled profile. The
/// result is clamped to `[-b, +a_max]`; the kernel applies the position caps
/// outside this command.
pub(crate) fn idm_acceleration(
    a_max: f64,
    b: f64,
    v0: f64,
    time_gap_s: f64,
    speed_mps: f64,
    constraints: &[Constraint],
) -> f64 {
    let v = speed_mps.max(0.0);
    let v0 = v0.max(f64::MIN_POSITIVE);

    let free = 1.0 - (v / v0).powi(IDM_FREE_FLOW_EXPONENT as i32);
    let mut interaction = 0.0;
    for constraint in constraints {
        interaction += interaction_term(time_gap_s, v, constraint, a_max, b);
    }

    (a_max * (free - interaction)).clamp(-b, a_max)
}

/// The `(s* / gap)^2` interaction term of one constraint.
fn interaction_term(
    time_gap_s: f64,
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
        speed_mps * time_gap_s + speed_mps * closing / (2.0 * sqrt_ab)
    } else {
        speed_mps * time_gap_s
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
