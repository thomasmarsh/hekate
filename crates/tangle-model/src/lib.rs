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

pub use compiled::{
    BoundaryId, CompiledBoundary, CompiledConflictRegion, CompiledCrossing, CompiledMovement,
    CompiledPath, CompiledPolygon, CompiledPortal, CompiledRegion, CompiledRule, CompiledScenario,
    CompiledSignal, CompiledSignalHead, CompiledSignalPhase, ConflictRegionId, CrossingId, IdMap,
    MovementId, PathId, PortalId, RegionId, RuleId, SignalId,
};
pub use schema::{scenario_schema, scenario_schema_json};
pub use source::{
    ConflictRegionSource, CoordinateSystem, CrossingSource, MovementSource, ParseError, PathEnd,
    PathSource, PointSource, PolygonSource, PopulationSource, PortalSource, RuleKind, RuleSource,
    SUPPORTED_SCHEMA_VERSION, ScenarioSource, SignalColor, SignalHeadSource, SignalPhaseSource,
    SignalSource, SignalStateSource, parse_scenario_source,
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
