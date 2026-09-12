//! Scenario source schema, semantic validation, and compiled representation.
//!
//! `tangle-model` is deliberately free of Bevy, windowing, wall-clock time, and
//! the filesystem. The one-way dependency direction is enforced in CI by
//! `scripts/check-dependency-direction.sh`.
//!
//! The authored document flows through three stages:
//!
//! 1. [`parse_scenario_source`] reads JSON5 into the [`ScenarioSource`] structs.
//! 2. [`validate`] returns stable [`Diagnostic`]s for semantic errors.
//! 3. [`CompiledScenario::compile`] produces the immutable, dense-indexed
//!    representation the kernel consumes.

mod compiled;
mod schema;
mod source;
mod validate;

pub use compiled::{CompiledPath, CompiledPortal, CompiledScenario, IdMap, PathId, PortalId};
pub use schema::{scenario_schema, scenario_schema_json};
pub use source::{
    CoordinateSystem, ParseError, PathEnd, PathSource, PointSource, PopulationSource, PortalSource,
    SUPPORTED_SCHEMA_VERSION, ScenarioSource, parse_scenario_source,
};
pub use validate::{Diagnostic, DiagnosticCode, validate};

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
