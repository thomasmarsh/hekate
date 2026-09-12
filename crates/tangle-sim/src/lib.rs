//! Deterministic, fixed-step simulation kernel.
//!
//! The kernel owns the authoritative clock. It must never read wall-clock time,
//! and it must not depend on Bevy, a window, or the filesystem.

pub use tangle_model::MODEL_VERSION;

#[cfg(test)]
mod tests {
    use super::MODEL_VERSION;

    #[test]
    fn reexports_model_version() {
        assert!(!MODEL_VERSION.is_empty());
    }
}
