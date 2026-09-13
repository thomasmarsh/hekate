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
//!
//! Alongside the compiled scenario, [`AgentComponents`] defines the compiled
//! components of one simulated road user: a compact core plus body, motion,
//! tactical, access, occupancy, and social components, dispatched through the
//! derived [`AgentFamily`] rather than a named mode.

mod compiled;
mod components;
mod migrate;
mod mode_template;
mod schema;
mod source;
mod validate;

pub use compiled::{
    BoundaryId, CompiledBoundary, CompiledConflictRegion, CompiledCrossing, CompiledDemand,
    CompiledMovement, CompiledPath, CompiledPedestrianDemand, CompiledPedestrianProfile,
    CompiledPedestrianRoute, CompiledPedestrianRouteShare, CompiledPedestrianSignal,
    CompiledPedestrianSignalPhase, CompiledPolygon, CompiledPortal, CompiledProfile,
    CompiledRegion, CompiledRouteShare, CompiledRule, CompiledScenario, CompiledSignal,
    CompiledSignalHead, CompiledSignalPhase, CompiledWaitingArea, ConflictRegionId, CrossingId,
    DemandId, IdMap, MovementId, PathId, PedestrianDemandId, PedestrianRouteId, PortalId,
    ProfileRange, RegionId, RuleId, SignalId, WaitingAreaId,
};
pub use components::{
    AgentAccess, AgentBehaviorProfile, AgentBody, AgentComponents, AgentCore, AgentFamily,
    AgentIntent, AgentLifecycle, AgentMotion, AgentOccupancy, AgentPose, AgentRoute, AgentVelocity,
    BodyKind, BodySegment, ComponentMismatch, GroupMembership, GroupRole, NominalDirection,
    PedestrianGroupId, SocialState, SpeedPolicy, TacticalCapabilities, TacticalCapability,
    TransitOccupancy,
};
pub use migrate::{MIGRATION_VERSION, migrate_v1_to_v2, to_canonical_v2_json};
pub use mode_template::{CompiledModeTemplate, compile_mode_template};
pub use schema::{
    scenario_schema, scenario_schema_json, scenario_schema_v1, scenario_schema_v1_json,
};
pub use source::{
    AccessSource, ConflictRegionSource, CoordinateSystem, CrossingSource, DemandChoiceSource,
    DemandPopulationSpawnSource, DemandRateSpawnSource, DemandSource, DemandSourceV2,
    DemandSpawnSource, DocumentReadError, FacilityKind, MIN_SUPPORTED_SCHEMA_VERSION,
    ModeBodySource, ModeTemplateSource, MotionKind, MovementDirection, MovementSource,
    MovementSourceV2, OccupancyKind, ParseError, PathEnd, PathSource, PedestrianDemandSource,
    PedestrianProfileSource, PedestrianRouteShareSource, PedestrianRouteSource,
    PedestrianSignalPhaseSource, PedestrianSignalSource, PointSource, PolygonSource,
    PopulationSource, PortalSource, ProfileRangeSource, ProfileSource, READABLE_SCHEMA_VERSIONS,
    RouteShareSource, RuleKind, RuleSource, SUPPORTED_SCHEMA_VERSION, ScenarioDocument,
    ScenarioSource, ScenarioSourceV2, SignalColor, SignalHeadSource, SignalPhaseSource,
    SignalSource, SignalStateSource, TacticKind, TimeIntervalSource, WaitingAreaSource,
    parse_scenario_document, parse_scenario_source, parse_scenario_source_v2,
};
pub use validate::{Diagnostic, DiagnosticCode, validate, validate_v2};

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
