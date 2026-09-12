//! Documented contextual pedestrian signal-compliance decision.
//!
//! # Model card
//!
//! When a pedestrian approaches a signal-controlled crossing, this module
//! decides whether it will wait at the crossing or cross against the signal.
//! The decision is a *choice over a physically possible movement*, never a
//! special trajectory: when the action is [`PedestrianSignalAction::Cross`] the
//! pedestrian keeps its normal route and is steered by the documented waypoint
//! controller ([`crate::pedestrian`]) at its normal bounded speed; a
//! non-compliant pedestrian therefore runs the signal under ordinary physics
//! and is never teleported. When the action is [`PedestrianSignalAction::Wait`]
//! the controller commands a bounded stopping profile (see [`wait_speed_target_mps`])
//! that brings the pedestrian to rest just short of the crossing, within the
//! controller's own deceleration bound.
//!
//! ## Inputs
//!
//! The decision is a pure function of the context a pedestrian can perceive:
//!
//! - the governing [`PedestrianSignalColor`] of the crossing, `Walk` or
//!   `DontWalk`, derived from the crossing's fixed-time signal phase (and thus
//!   its elapsed and remaining time in the cycle);
//! - `crossing_gap_m`, the route distance to the crossing stop point, positive
//!   while the pedestrian is upstream of it;
//! - `speed_mps`, the current walking speed;
//! - the urgency, the uniform deceleration `required_deceleration` that would
//!   bring the pedestrian to rest exactly at the crossing;
//! - the pedestrian's stable `compliance` propensity in `[0, 1]`, from the
//!   `compliance` random stream.
//!
//! ## Rule
//!
//! A `Walk` signal, or a pedestrian that has already reached the crossing,
//! crosses. Otherwise the pedestrian obeys the signal when the required
//! deceleration is within the fraction of its bounded stopping deceleration it
//! is willing to use:
//!
//! ```text
//! wait  when  required_deceleration <= bounded_deceleration * compliance
//! ```
//!
//! The pedestrian's comfortable braking is the controller's own bounded
//! deceleration ([`crate::pedestrian::MAX_DECEL_MPS2`]), since the pedestrian
//! profile carries no separate brake parameter. A fully compliant pedestrian
//! (`compliance = 1.0`) waits whenever a bounded stop is possible and otherwise
//! crosses, because braking harder than the controller's bound is not a legal
//! movement. A less compliant pedestrian waits only when the stop is easy and
//! crosses when it is not. The comparison is inclusive: at the exact boundary
//! the decision is [`PedestrianSignalAction::Wait`], the documented tie-breaker.
//!
//! ## Reproducibility
//!
//! The propensity is drawn once per pedestrian from the `compliance` random
//! stream (see [`crate::rng`]), derived from the root seed and the stable agent
//! id. The decision adds no per-tick randomness: it is a deterministic function
//! of its context, so the same seed reproduces every decision.

use crate::compliance::required_deceleration;
use crate::signal::PedestrianSignalColor;

/// Distance in metres a waiting pedestrian leaves between itself and the
/// crossing stop point.
///
/// A positive reserve makes the waiting stop robust: the stopping profile
/// targets `gap - reserve`, so the instantaneous required deceleration stays
/// strictly below the willingness threshold while the pedestrian is braking and
/// the decision cannot flicker between waiting and crossing.
pub const STOP_RESERVE_M: f64 = 0.05;

/// Action selected by a pedestrian signal-compliance decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedestrianSignalAction {
    /// Hold at the crossing: obey a forbidding signal.
    Wait,
    /// Continue across the crossing; ordinary waypoint control still applies.
    Cross,
}

/// Why a pedestrian signal-compliance decision selected its action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedestrianComplianceReason {
    /// The crossing signal permits crossing.
    Walk,
    /// The pedestrian has already reached the crossing.
    AtCrossing,
    /// Stopping at the crossing would need more than the pedestrian's bounded
    /// stopping deceleration.
    CannotStop,
    /// The pedestrian could stop within its bounds and chose to obey.
    CompliantWait,
    /// The pedestrian could stop within its bounds but chose to cross.
    NonCompliantCross,
}

impl PedestrianComplianceReason {
    /// Short stable label for inspectors and traces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::AtCrossing => "at crossing",
            Self::CannotStop => "cannot stop within bounds",
            Self::CompliantWait => "compliant wait",
            Self::NonCompliantCross => "noncompliant cross",
        }
    }
}

/// Record of one pedestrian signal-compliance decision.
///
/// The record is small and `Copy` so an observer snapshot can carry the latest
/// one per pedestrian cheaply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PedestrianComplianceDecision {
    /// Action the decision selected.
    pub action: PedestrianSignalAction,
    /// Why the action was selected.
    pub reason: PedestrianComplianceReason,
    /// The governing pedestrian signal state at the decision.
    pub signal: PedestrianSignalColor,
    /// Route distance to the crossing stop point in metres.
    pub crossing_gap_m: f64,
    /// Required deceleration to rest at the crossing in metres per second
    /// squared.
    pub required_decel_mps2: f64,
}

/// Decide whether a pedestrian obeys a forbidding crossing signal.
///
/// See the module model card for the rule, the inputs, and the tie-breaker. The
/// function is pure, so the same context always yields the same decision.
/// `bounded_deceleration_mps2` is the controller's bounded stopping
/// deceleration, the pedestrian's comfortable braking.
pub(crate) fn decide(
    signal: PedestrianSignalColor,
    crossing_gap_m: f64,
    speed_mps: f64,
    compliance: f64,
    bounded_deceleration_mps2: f64,
) -> PedestrianComplianceDecision {
    if signal == PedestrianSignalColor::Walk {
        return PedestrianComplianceDecision {
            action: PedestrianSignalAction::Cross,
            reason: PedestrianComplianceReason::Walk,
            signal,
            crossing_gap_m: crossing_gap_m.max(0.0),
            required_decel_mps2: 0.0,
        };
    }

    if crossing_gap_m <= 0.0 {
        return PedestrianComplianceDecision {
            action: PedestrianSignalAction::Cross,
            reason: PedestrianComplianceReason::AtCrossing,
            signal,
            crossing_gap_m: 0.0,
            required_decel_mps2: 0.0,
        };
    }

    let required = required_deceleration(crossing_gap_m, speed_mps);
    let willingness = bounded_deceleration_mps2.max(0.0) * compliance.clamp(0.0, 1.0);
    let (action, reason) = if required <= willingness {
        (
            PedestrianSignalAction::Wait,
            PedestrianComplianceReason::CompliantWait,
        )
    } else if required <= bounded_deceleration_mps2.max(0.0) {
        (
            PedestrianSignalAction::Cross,
            PedestrianComplianceReason::NonCompliantCross,
        )
    } else {
        (
            PedestrianSignalAction::Cross,
            PedestrianComplianceReason::CannotStop,
        )
    };

    PedestrianComplianceDecision {
        action,
        reason,
        signal,
        crossing_gap_m,
        required_decel_mps2: required,
    }
}

/// The speed a waiting pedestrian targets at `crossing_gap_m`.
///
/// It is the constant-deceleration stopping profile that brings the pedestrian
/// to rest [`STOP_RESERVE_M`] short of the crossing, using the pedestrian's
/// willingness `bounded_deceleration * compliance`:
///
/// ```text
/// v_target = sqrt(2 * bounded_deceleration * compliance * max(gap - reserve, 0))
/// ```
///
/// The kernel feeds this to [`crate::pedestrian::advance_speed`], so the
/// per-step speed change still obeys the controller's acceleration bounds: a
/// compliant wait is a bounded slowdown, never a teleport or a special
/// trajectory. A zero willingness returns zero, which holds the pedestrian at
/// rest.
pub(crate) fn wait_speed_target_mps(
    crossing_gap_m: f64,
    compliance: f64,
    bounded_deceleration_mps2: f64,
) -> f64 {
    let willingness = bounded_deceleration_mps2.max(0.0) * compliance.clamp(0.0, 1.0);
    let usable = (crossing_gap_m - STOP_RESERVE_M).max(0.0);
    (2.0 * willingness * usable).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The controller's bounded stopping deceleration used across the tests.
    const BOUND: f64 = 2.0;

    fn decision(gap: f64, speed: f64, compliance: f64) -> PedestrianComplianceDecision {
        decide(
            PedestrianSignalColor::DontWalk,
            gap,
            speed,
            compliance,
            BOUND,
        )
    }

    #[test]
    fn a_walk_signal_always_crosses() {
        let decision = decide(PedestrianSignalColor::Walk, 5.0, 1.25, 1.0, BOUND);
        assert_eq!(decision.action, PedestrianSignalAction::Cross);
        assert_eq!(decision.reason, PedestrianComplianceReason::Walk);
    }

    #[test]
    fn a_pedestrian_at_or_past_the_crossing_crosses() {
        for gap in [0.0, -0.5] {
            let decision = decision(gap, 1.25, 1.0);
            assert_eq!(decision.action, PedestrianSignalAction::Cross);
            assert_eq!(decision.reason, PedestrianComplianceReason::AtCrossing);
            assert_eq!(decision.crossing_gap_m, 0.0);
        }
    }

    #[test]
    fn a_compliant_pedestrian_waits_within_its_bounds() {
        // required = 1.25^2 / (2 * 2.0) = 0.39 m/s² < 2.0.
        let decision = decision(2.0, 1.25, 1.0);
        assert_eq!(decision.action, PedestrianSignalAction::Wait);
        assert_eq!(decision.reason, PedestrianComplianceReason::CompliantWait);
        assert!((decision.required_decel_mps2 - 0.390625).abs() < 1e-12);
    }

    #[test]
    fn a_compliant_pedestrian_crosses_when_it_cannot_stop_within_bounds() {
        // required = 3^2 / (2 * 1.0) = 4.5 m/s² > 2.0.
        let decision = decision(1.0, 3.0, 1.0);
        assert_eq!(decision.action, PedestrianSignalAction::Cross);
        assert_eq!(decision.reason, PedestrianComplianceReason::CannotStop);
    }

    #[test]
    fn a_noncompliant_pedestrian_crosses_a_stop_it_could_make() {
        // required 0.39 m/s² is within the bounds but above 2.0 * 0.1.
        let decision = decision(2.0, 1.25, 0.1);
        assert_eq!(decision.action, PedestrianSignalAction::Cross);
        assert_eq!(
            decision.reason,
            PedestrianComplianceReason::NonCompliantCross
        );
    }

    #[test]
    fn the_compliance_boundary_is_inclusive() {
        // required = 2.0^2 / (2 * 2.0) = 1.0 m/s² exactly equals the
        // half-willingness of a 0.5 propensity.
        let at_half = decision(2.0, 2.0, 0.5);
        assert!((at_half.required_decel_mps2 - 1.0).abs() < 1e-12);
        assert_eq!(at_half.action, PedestrianSignalAction::Wait);
        assert_eq!(at_half.reason, PedestrianComplianceReason::CompliantWait);

        // required = 2.0^2 / (2 * 1.0) = 2.0 m/s² exactly equals the full
        // willingness of a fully compliant pedestrian.
        let at_full = decision(1.0, 2.0, 1.0);
        assert!((at_full.required_decel_mps2 - BOUND).abs() < 1e-12);
        assert_eq!(at_full.action, PedestrianSignalAction::Wait);
        assert_eq!(at_full.reason, PedestrianComplianceReason::CompliantWait);
    }

    #[test]
    fn a_zero_compliance_pedestrian_never_finds_a_stop_while_moving() {
        let decision = decision(30.0, 1.25, 0.0);
        assert_eq!(decision.action, PedestrianSignalAction::Cross);
        assert_eq!(
            decision.reason,
            PedestrianComplianceReason::NonCompliantCross
        );
    }

    #[test]
    fn the_wait_target_is_a_bounded_stop_short_of_the_crossing() {
        // The target is zero at or inside the reserve, so a waiting pedestrian
        // never reaches the stop point.
        assert_eq!(wait_speed_target_mps(STOP_RESERVE_M, 1.0, BOUND), 0.0);
        assert_eq!(wait_speed_target_mps(0.0, 1.0, BOUND), 0.0);
        // The boundary speed is `sqrt(2 * BOUND * gap)`; with a positive
        // reserve the target sits strictly below it, so no step can overshoot.
        let gap = 2.0;
        let boundary_speed = (2.0 * BOUND * gap).sqrt();
        let target = wait_speed_target_mps(gap, 1.0, BOUND);
        assert!(
            target < boundary_speed,
            "reserve keeps the target below the onset"
        );
        // A full-compliant, well-upstream target exceeds the desired speed, so
        // a distant pedestrian simply keeps walking.
        assert!(wait_speed_target_mps(50.0, 1.0, BOUND) > 1.6);
    }

    #[test]
    fn the_wait_target_falls_to_zero_with_a_zero_willingness() {
        assert_eq!(wait_speed_target_mps(10.0, 0.0, BOUND), 0.0);
    }
}
