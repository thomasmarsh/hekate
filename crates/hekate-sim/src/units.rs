//! Physical quantity wrappers used at the kernel API boundary.
//!
//! The kernel works in SI units with `f64` internally. Quantities that appear
//! in the public API are wrapped so a call site cannot pass seconds where
//! metres are expected; serialized observer fields still carry unit suffixes
//! such as `_m` or `_mps`.

/// A duration in seconds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(f64);

impl Seconds {
    /// Zero seconds.
    pub const ZERO: Self = Self(0.0);

    /// Wrap a raw second count.
    pub const fn from_secs(secs: f64) -> Self {
        Self(secs)
    }

    /// The raw second count.
    pub const fn as_secs(self) -> f64 {
        self.0
    }

    /// Whether this is a usable fixed step: finite and strictly positive.
    pub fn is_finite_positive(self) -> bool {
        self.0.is_finite() && self.0 > 0.0
    }
}
