//! Run-level configuration owned by the caller rather than the scenario.

use crate::units::Seconds;

/// Default fixed physics step: the Standard fidelity preset from
/// `PHASE_1_PLAN.md`.
pub const DEFAULT_STEP: Seconds = Seconds::from_secs(0.05);

/// Configuration for one run of a compiled scenario.
///
/// The scenario carries authored content; `RunConfig` carries the choices a
/// caller makes when executing it. Fidelity values live in the manifest and
/// here, never only in code defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunConfig {
    seed: u64,
    step: Seconds,
}

impl RunConfig {
    /// A run with the Standard fixed step and the given root seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            step: DEFAULT_STEP,
        }
    }

    /// Override the fixed step. [`crate::Simulation::new`] rejects a step that
    /// is not finite and positive.
    pub fn with_step(mut self, step: Seconds) -> Self {
        self.step = step;
        self
    }

    /// The root seed recorded in run provenance.
    pub const fn seed(self) -> u64 {
        self.seed
    }

    /// The fixed step duration.
    pub const fn step(self) -> Seconds {
        self.step
    }
}

impl Default for RunConfig {
    fn default() -> Self {
        Self::new(0)
    }
}
