//! Scenario source schema, semantic validation, and compiled representation.
//!
//! `tangle-model` is deliberately free of Bevy, windowing, wall-clock time, and
//! the filesystem. The one-way dependency direction is enforced in CI by
//! `scripts/check-dependency-direction.sh`.

/// Version of the scenario model understood by this build.
pub const MODEL_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::MODEL_VERSION;

    #[test]
    fn model_version_is_not_empty() {
        assert!(!MODEL_VERSION.is_empty());
    }
}
