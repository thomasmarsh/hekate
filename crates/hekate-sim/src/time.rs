//! Authoritative simulation clock.
//!
//! Kernel time is derived only from a fixed step and a completed-tick count.
//! It is never derived from wall-clock time, so rendering frame rate and host
//! speed cannot change simulation results.

use crate::units::Seconds;

/// The kernel's authoritative time.
///
/// `SimTime` is `tick * step`: the number of completed fixed steps times the
/// configured step duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimTime {
    tick: u64,
    step: Seconds,
}

impl SimTime {
    /// Time zero for a run whose fixed step is `step`.
    pub const fn zero(step: Seconds) -> Self {
        Self { tick: 0, step }
    }

    /// The time after `tick` fixed steps of `step`.
    pub const fn from_tick(tick: u64, step: Seconds) -> Self {
        Self { tick, step }
    }

    /// Number of completed fixed steps.
    pub const fn tick(self) -> u64 {
        self.tick
    }

    /// The fixed step duration.
    pub const fn step(self) -> Seconds {
        self.step
    }

    /// Elapsed simulated seconds, `tick * step`.
    pub fn seconds(self) -> f64 {
        self.tick as f64 * self.step.as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_is_tick_times_step() {
        let time = SimTime::from_tick(200, Seconds::from_secs(0.05));
        assert_eq!(time.tick(), 200);
        assert_eq!(time.step(), Seconds::from_secs(0.05));
        assert!((time.seconds() - 10.0).abs() < 1e-12);
    }

    #[test]
    fn zero_has_no_completed_ticks() {
        let zero = SimTime::zero(Seconds::from_secs(0.02));
        assert_eq!(zero.tick(), 0);
        assert_eq!(zero.seconds(), 0.0);
    }

    #[test]
    fn step_is_validated_by_the_kernel() {
        assert!(Seconds::from_secs(0.05).is_finite_positive());
        assert!(!Seconds::from_secs(0.0).is_finite_positive());
        assert!(!Seconds::from_secs(-0.05).is_finite_positive());
        assert!(!Seconds::from_secs(f64::NAN).is_finite_positive());
        assert!(!Seconds::from_secs(f64::INFINITY).is_finite_positive());
    }
}
