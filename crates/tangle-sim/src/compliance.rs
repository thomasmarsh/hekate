//! Documented contextual signal-compliance decision.
//!
//! # Model card
//!
//! When a profile vehicle approaches a signal-controlled stop line, this module
//! decides whether it will hold at the line or continue past it. The decision
//! is a *choice over a physically possible movement*, never a special
//! trajectory: when the action is [`SignalAction::Proceed`] the stop-line
//! constraint is simply not imposed, and the vehicle still obeys the documented
//! IDM controller and its profile bounds. A non-compliant vehicle therefore
//! runs the head under ordinary physics; it is never teleported or given an
//! impossible speed.
//!
//! ## Inputs
//!
//! The decision is a pure function of the context a driver can perceive:
//!
//! - the governing head's [`SignalColor`];
//! - `stop_line_gap_m`, the front-bumper distance to the authored stop line;
//! - `speed_mps`, the current speed;
//! - the urgency [`required_deceleration`], the uniform deceleration that would
//!   bring the vehicle to rest exactly at the line;
//! - the driver's stable `compliance` propensity in `[0, 1]` and their
//!   comfortable braking, both from the sampled [`crate::VehicleProfile`].
//!
//! ## Rule
//!
//! A green head, or a vehicle whose front bumper has already reached the line,
//! proceeds. Otherwise the driver obeys the head when the required deceleration
//! is within the fraction of their comfortable braking they are willing to use:
//!
//! ```text
//! obey  when  required_deceleration <= comfortable_brake * compliance
//! ```
//!
//! A fully compliant driver (`compliance = 1.0`) obeys whenever a comfortable
//! stop is possible and otherwise runs, because braking harder than comfortable
//! is not a legal movement. A driver with lower compliance obeys only when the
//! stop is easy and runs when it is not. The comparison is inclusive: at the
//! exact boundary the decision is [`SignalAction::Stop`], which is the
//! documented tie-breaker.
//!
//! ## Reproducibility
//!
//! The propensity is drawn once per agent from the `compliance` random stream
//! (see [`crate::rng`]), derived from the root seed and the stable agent id.
//! The decision adds no per-tick randomness: it is a deterministic function of
//! its context, so a rare behavior is contextual rather than an independent
//! coin flip each tick, and the same seed reproduces every decision.

use tangle_model::SignalColor;

/// Action selected by a signal-compliance decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalAction {
    /// Hold at the stop line: obey a stop-required head.
    Stop,
    /// Continue past the head; ordinary IDM control still applies.
    Proceed,
}

/// Why a signal-compliance decision selected its action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceReason {
    /// The governing head shows green, so no stop is required.
    Green,
    /// The front bumper has already reached the stop line.
    PastStopLine,
    /// Stopping would need more than the driver's comfortable braking.
    CannotStop,
    /// The driver could stop comfortably and chose to obey.
    CompliantStop,
    /// The driver could stop comfortably but chose to run the head.
    NonCompliantRun,
}

impl ComplianceReason {
    /// Short stable label for inspectors and traces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::PastStopLine => "past stop line",
            Self::CannotStop => "cannot stop comfortably",
            Self::CompliantStop => "compliant stop",
            Self::NonCompliantRun => "noncompliant run",
        }
    }
}

/// Record of one signal-compliance decision.
///
/// The record is small and `Copy` so an observer snapshot can carry the latest
/// one per agent cheaply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplianceDecision {
    /// Action the decision selected.
    pub action: SignalAction,
    /// Why the action was selected.
    pub reason: ComplianceReason,
    /// The governing head's color at the decision.
    pub color: SignalColor,
    /// Front-bumper distance to the stop line in metres.
    pub stop_line_gap_m: f64,
    /// Required deceleration to rest at the line in metres per second squared.
    pub required_decel_mps2: f64,
}

/// Uniform deceleration that brings `speed_mps` to rest over `gap_m`.
///
/// Returns `f64::INFINITY` for a non-positive or non-finite gap, which the
/// decision treats as "cannot stop".
pub fn required_deceleration(gap_m: f64, speed_mps: f64) -> f64 {
    if !gap_m.is_finite() || gap_m <= 0.0 {
        return f64::INFINITY;
    }
    let speed = speed_mps.max(0.0);
    speed * speed / (2.0 * gap_m)
}

/// Decide whether a vehicle obeys a stop-required head.
///
/// See the module model card for the rule, the inputs, and the tie-breaker. The
/// function is pure, so the same context always yields the same decision.
pub(crate) fn decide(
    color: SignalColor,
    stop_line_gap_m: f64,
    speed_mps: f64,
    compliance: f64,
    comfortable_brake_mps2: f64,
) -> ComplianceDecision {
    if color == SignalColor::Green {
        return ComplianceDecision {
            action: SignalAction::Proceed,
            reason: ComplianceReason::Green,
            color,
            stop_line_gap_m: stop_line_gap_m.max(0.0),
            required_decel_mps2: 0.0,
        };
    }

    if stop_line_gap_m <= 0.0 {
        return ComplianceDecision {
            action: SignalAction::Proceed,
            reason: ComplianceReason::PastStopLine,
            color,
            stop_line_gap_m: 0.0,
            required_decel_mps2: 0.0,
        };
    }

    let required = required_deceleration(stop_line_gap_m, speed_mps);
    let willingness = comfortable_brake_mps2 * compliance.clamp(0.0, 1.0);
    let (action, reason) = if required <= willingness {
        (SignalAction::Stop, ComplianceReason::CompliantStop)
    } else if required <= comfortable_brake_mps2 {
        (SignalAction::Proceed, ComplianceReason::NonCompliantRun)
    } else {
        (SignalAction::Proceed, ComplianceReason::CannotStop)
    };

    ComplianceDecision {
        action,
        reason,
        color,
        stop_line_gap_m,
        required_decel_mps2: required,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A comfortable braking value used across the boundary cases.
    const BRAKE: f64 = 3.0;

    #[test]
    fn green_always_proceeds() {
        let decision = decide(SignalColor::Green, 5.0, 20.0, 1.0, BRAKE);
        assert_eq!(decision.action, SignalAction::Proceed);
        assert_eq!(decision.reason, ComplianceReason::Green);
    }

    #[test]
    fn a_vehicle_at_or_past_the_line_proceeds() {
        for gap in [0.0, -0.5] {
            for color in [SignalColor::Red, SignalColor::Yellow] {
                let decision = decide(color, gap, 5.0, 1.0, BRAKE);
                assert_eq!(decision.action, SignalAction::Proceed);
                assert_eq!(decision.reason, ComplianceReason::PastStopLine);
                assert_eq!(decision.stop_line_gap_m, 0.0);
            }
        }
    }

    #[test]
    fn a_compliant_driver_stops_within_comfortable_braking() {
        // required = 10^2 / (2 * 20) = 2.5 m/s² < 3.0.
        let decision = decide(SignalColor::Red, 20.0, 10.0, 1.0, BRAKE);
        assert_eq!(decision.action, SignalAction::Stop);
        assert_eq!(decision.reason, ComplianceReason::CompliantStop);
        assert!((decision.required_decel_mps2 - 2.5).abs() < 1e-12);
    }

    #[test]
    fn a_compliant_driver_runs_when_it_cannot_stop_comfortably() {
        // required = 15^2 / (2 * 10) = 11.25 m/s² > 3.0.
        let decision = decide(SignalColor::Red, 10.0, 15.0, 1.0, BRAKE);
        assert_eq!(decision.action, SignalAction::Proceed);
        assert_eq!(decision.reason, ComplianceReason::CannotStop);
    }

    #[test]
    fn a_noncompliant_driver_runs_a_comfortable_stop() {
        // required 2.5 m/s² is within comfortable braking but above 3.0 * 0.5.
        let decision = decide(SignalColor::Red, 20.0, 10.0, 0.5, BRAKE);
        assert_eq!(decision.action, SignalAction::Proceed);
        assert_eq!(decision.reason, ComplianceReason::NonCompliantRun);
    }

    #[test]
    fn the_compliance_boundary_is_inclusive() {
        // required = 3.0 m/s² exactly equals the comfortable limit, and the
        // 0.5 propensity puts the boundary at 1.5 m/s².
        let at_full = decide(SignalColor::Red, 6.0, 6.0, 1.0, BRAKE);
        assert!((at_full.required_decel_mps2 - 3.0).abs() < 1e-12);
        assert_eq!(at_full.action, SignalAction::Stop);
        assert_eq!(at_full.reason, ComplianceReason::CompliantStop);

        let at_half = decide(SignalColor::Red, 12.0, 6.0, 0.5, BRAKE);
        assert!((at_half.required_decel_mps2 - 1.5).abs() < 1e-12);
        assert_eq!(at_half.action, SignalAction::Stop);
        assert_eq!(at_half.reason, ComplianceReason::CompliantStop);
    }

    #[test]
    fn urgency_grows_as_the_gap_shrinks_at_constant_speed() {
        let near = decide(SignalColor::Red, 10.0, 10.0, 1.0, BRAKE);
        let far = decide(SignalColor::Red, 40.0, 10.0, 1.0, BRAKE);
        assert!(near.required_decel_mps2 > far.required_decel_mps2);
    }

    #[test]
    fn a_zero_compliance_driver_never_finds_a_comfortable_stop() {
        // required > 0 for any approach speed, so willingness 0.0 never holds.
        let decision = decide(SignalColor::Yellow, 30.0, 1.0, 0.0, BRAKE);
        assert_eq!(decision.action, SignalAction::Proceed);
        assert_eq!(decision.reason, ComplianceReason::NonCompliantRun);
    }

    #[test]
    fn required_deceleration_is_infinite_without_a_positive_gap() {
        assert!(required_deceleration(0.0, 5.0).is_infinite());
        assert!(required_deceleration(-1.0, 5.0).is_infinite());
        assert_eq!(required_deceleration(10.0, 0.0), 0.0);
    }
}
