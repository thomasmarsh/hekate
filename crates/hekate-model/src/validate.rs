//! Semantic validation of a [`ScenarioSource`].
//!
//! JSON Schema only expresses structure. These checks cover the semantic rules
//! that a schema cannot: identifier uniqueness, finite geometry, positive
//! dimensions, and portal reachability. Every failure carries a stable
//! [`DiagnosticCode`] plus the identifier of the offending source object so
//! tooling can point at the exact document node.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::compiled::CompiledReferencePath;
use crate::mode_template::body_motion_pair_has_family;
use crate::source::{
    AdjacencySide, ConflictRegionSource, CrossingSource, DemandChoiceSource, DemandSpawnSource,
    FacilityDirection, FacilityKind, FacilitySource, LateralUse, MIN_SUPPORTED_SCHEMA_VERSION,
    ManeuverPolicySource, ModeBodySource, ModeTemplateSource, MotionKind, MovementDirection,
    PassingSide, PathEnd, PathSource, PedestrianRouteShareSource, PedestrianRouteSource,
    PermissionEffect, PermissionKind, PointSource, PolygonSource, PortalSource, ProfileRangeSource,
    RouteShareSource, RuleKind, RuleSource, SUPPORTED_SCHEMA_VERSION, ScenarioSource,
    ScenarioSourceV2, SignalColor, SignalSource, TacticKind, WaitingAreaSource,
};
use glam::DVec2;

/// Stable, machine-readable diagnostic codes.
///
/// The `as_str` values are part of the scenario contract and must not change
/// without a schema-version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticCode {
    /// The scenario declares a schema version this build cannot read.
    UnsupportedSchemaVersion,
    /// A path or portal has an empty identifier.
    EmptyId,
    /// Two authored objects share an identifier.
    DuplicateId,
    /// A coordinate is NaN or infinite.
    NonFiniteCoordinate,
    /// A dimension, duration, or count must be strictly positive.
    NonPositiveValue,
    /// A path has fewer than two vertices.
    EmptyPath,
    /// A path has zero length because all vertices coincide.
    DegeneratePath,
    /// A portal references a path that is not declared.
    PortalUnknownPath,
    /// A portal's path has no traversable geometry to attach to.
    PortalUnreachable,
    /// Two portals attach to the same end of the same path.
    PortalDuplicateEnd,
    /// Two portals overlap in world space.
    PortalOverlap,
    /// A polygon has fewer than three vertices.
    PolygonTooFewVertices,
    /// A polygon encloses no area.
    PolygonDegenerate,
    /// A movement references a portal that is not declared.
    MovementUnknownPortal,
    /// A movement references a path that is not declared.
    MovementUnknownPath,
    /// A movement starts and ends at the same portal.
    MovementSelfLoop,
    /// A movement's portals do not both attach to its guide path.
    MovementPortalPathMismatch,
    /// A crossing crosses no movements.
    CrossingEmpty,
    /// A movement's stop line is non-finite, negative, or past its path end.
    MovementStopLineInvalid,
    /// A crossing references a region that is not declared.
    CrossingUnknownRegion,
    /// A crossing references a movement that is not declared.
    CrossingUnknownMovement,
    /// A pedestrian signal declares no phases.
    CrossingPedestrianSignalEmpty,
    /// A pedestrian signal phase duration is non-finite or non-positive.
    CrossingPedestrianSignalPhaseInvalid,
    /// A conflict region does not name exactly two distinct movements.
    ConflictArity,
    /// A conflict region references a movement that is not declared.
    ConflictUnknownMovement,
    /// A rule references a movement that is not declared.
    RuleUnknownMovement,
    /// A signal rule references a signal that is not declared.
    RuleUnknownSignal,
    /// A rule's signal field disagrees with its kind.
    RuleSignalMismatch,
    /// A signal has no heads or no phases.
    SignalEmpty,
    /// A signal head controls a movement that is not declared.
    SignalUnknownMovement,
    /// Two signal heads in one signal share an identifier.
    SignalDuplicateHead,
    /// A signal phase lists no states.
    SignalPhaseEmpty,
    /// A signal phase references a head that is not declared.
    SignalUnknownHead,
    /// A signal phase lists one head more than once.
    SignalPhaseDuplicateHead,
    /// A signal phase omits a head declared by its signal.
    SignalPhaseMissingHead,
    /// A signal phase shows two conflicting movements a green at once.
    SignalConflictingGreen,
    /// A demand source references a portal that is not declared.
    DemandUnknownPortal,
    /// A demand source lists no routes.
    DemandEmptyRoutes,
    /// A demand route references a movement that is not declared.
    DemandUnknownMovement,
    /// A demand route's movement does not start at the demand portal.
    DemandRoutePortalMismatch,
    /// A demand source lists one movement more than once.
    DemandDuplicateRoute,
    /// A profile range is non-finite, non-positive, or inverted.
    ProfileRangeInvalid,
    /// A compliance propensity range is non-finite or outside `[0, 1]`.
    ProfileComplianceInvalid,
    /// A waiting area references a region that is not declared.
    WaitingAreaUnknownRegion,
    /// A pedestrian route references a portal that is not declared.
    PedestrianRouteUnknownPortal,
    /// A pedestrian route references a path that is not declared.
    PedestrianRouteUnknownPath,
    /// A pedestrian route's portals do not both attach to its guide path.
    PedestrianRoutePortalPathMismatch,
    /// A pedestrian route starts and ends at the same portal.
    PedestrianRouteSelfLoop,
    /// A pedestrian route references a crossing that is not declared.
    PedestrianRouteUnknownCrossing,
    /// A pedestrian route references a waiting area that is not declared.
    PedestrianRouteUnknownWaitingArea,
    /// A pedestrian demand source references a portal that is not declared.
    PedestrianDemandUnknownPortal,
    /// A pedestrian demand source lists no routes.
    PedestrianDemandEmptyRoutes,
    /// A pedestrian demand route references a route that is not declared.
    PedestrianDemandUnknownRoute,
    /// A pedestrian demand route starts at a different portal than the source.
    PedestrianDemandRoutePortalMismatch,
    /// A pedestrian demand source lists one route more than once.
    PedestrianDemandDuplicateRoute,
    /// A version-2 movement's declared direction disagrees with its portal order.
    MovementDirectionMismatch,
    /// A version-2 mode template's body kind cannot use its motion family.
    ModeTemplateBodyMotion,
    /// A version-2 mode template's occupancy or tactics need a passenger capacity.
    ModeTemplateOccupancy,
    /// A version-2 mode template's profiles are missing or unexpected for its family.
    ModeTemplateProfile,
    /// A version-2 demand source references an undeclared mode template.
    DemandUnknownMode,
    /// A version-2 demand source references an undeclared path.
    DemandUnknownPath,
    /// A version-2 demand choice does not match the mode's motion family.
    DemandChoiceMismatch,
    /// A version-2 demand interval is non-finite, negative, or inverted.
    DemandIntervalInvalid,
    /// A version-2 document declares an unusable population demand.
    DemandPopulationInvalid,
    /// A version-2 facility references a region that is not declared.
    FacilityUnknownRegion,
    /// A version-2 facility references a reference path that is not declared.
    FacilityUnknownPath,
    /// A version-2 facility's access names a mode template that is not declared.
    FacilityUnknownMode,
    /// A version-2 facility permits no mode.
    FacilityAccessEmpty,
    /// A version-2 facility's nominal direction needs a reference path.
    FacilityDirectionWithoutPath,
    /// A version-2 facility width is non-finite or non-positive.
    FacilityWidthInvalid,
    /// A version-2 facility speed limit is non-finite or non-positive.
    FacilitySpeedLimitInvalid,
    /// A version-2 facility region lies outside the traversable world.
    FacilityOutsideWorld,
    /// A version-2 facility cannot fit an eligible body plus its clearance.
    FacilityTooNarrow,
    /// A version-2 facility's reference curvature exceeds a mode's turning limit.
    FacilityCurvature,
    /// A version-2 facility connector references a facility that is not declared.
    FacilityConnectorUnknownFacility,
    /// A version-2 facility connector attaches a facility with no reference path.
    FacilityConnectorWithoutReference,
    /// A version-2 facility connector's leaving and entering ends do not coincide.
    FacilityConnectorDiscontinuous,
    /// A version-2 facility's nominal direction is not physically possible.
    FacilityUnreachableDirection,
    /// A version-2 facility permits a mode whose template does not serve facilities.
    FacilityAccessDenied,
    /// A version-2 permission names a holder mode template that is not declared.
    PermissionUnknownHolder,
    /// A version-2 permission names a target object that is not declared.
    PermissionUnknownTarget,
    /// A version-2 permission prohibits a route the facility otherwise grants.
    PermissionRouteProhibited,
    /// A profile range is non-finite, negative, or inverted.
    ProfileNonNegativeInvalid,
    /// A version-2 mode template's `lateral` object is incompatible with its
    /// motion family or its tactics.
    ModeTemplateLateral,
    /// A version-2 lateral target clearance is non-finite or negative.
    ModeLateralClearance,
    /// A version-2 lateral horizon is non-finite or non-positive.
    ModeLateralHorizon,
    /// A version-2 mode template's authored wheelbase is non-finite,
    /// non-positive, inverted, on a non-wheeled motion, or authored without its
    /// steering-angle companion.
    ModeWheelbase,
    /// A version-2 mode template's authored maximum steering angle is
    /// non-finite, outside `(0, pi/2)`, inverted, on a non-wheeled motion, or
    /// authored without its wheelbase companion.
    ModeSteeringAngle,
    /// A version-2 articulated-chain body has fewer than two segments, a lead
    /// segment authoring a hitch offset, a trailing segment omitting one, or a
    /// segment dimension that is non-finite, non-positive, or inverted.
    ModeArticulatedSegments,
    /// A version-2 articulated-chain body's articulation limit is non-finite,
    /// outside `(0, pi)`, or inverted.
    ModeArticulationLimit,
    /// A version-2 mode template declares a lateral tactic but no maneuver policy.
    ManeuverPolicyMissing,
    /// A version-2 mode template declares `reverse_direction` but no wrong-way policy.
    WrongWayPolicyMissing,
    /// A version-2 commit policy value is non-finite or out of range.
    CommitPolicyInvalid,
    /// A version-2 commit clearance floor exceeds a lateral mode's target clearance.
    CommitClearanceExceedsTarget,
    /// A version-2 wrong-way policy value is non-finite or out of range.
    WrongWayPolicyInvalid,
    /// A version-2 clearance band threshold is non-finite or non-positive.
    ClearanceBandThreshold,
    /// Version-2 clearance bands are not in strictly increasing threshold order.
    ClearanceBandOrder,
    /// A version-2 clearance band lists `applies_to_modes` but leaves it empty.
    ClearanceBandModesEmpty,
    /// A version-2 clearance band names a mode template that is not declared.
    ClearanceBandUnknownMode,
    /// A version-2 facility lateral policy has no reference path to name a side on.
    FacilityLateralWithoutReference,
    /// A version-2 centered facility declares a lateral policy.
    FacilityLateralCentered,
    /// A version-2 facility lateral policy names a side no eligible body can occupy.
    FacilityPassingSideUnusable,
    /// A version-2 facility adjacency names a facility that is not declared.
    FacilityAdjacencyUnknownFacility,
    /// A version-2 facility adjacency joins a facility to itself.
    FacilityAdjacencySelf,
    /// A version-2 facility adjacency attaches a facility with no reference path.
    FacilityAdjacencyWithoutReference,
    /// A version-2 facility adjacency's bands do not touch along a shared boundary.
    FacilityAdjacencyDisjoint,
    /// A version-2 facility adjacency's bands touch on the other side than authored.
    FacilityAdjacencySide,
    /// A version-2 permission's `kind` disagrees with its target object kind.
    PermissionTargetKind,
    /// Two version-2 permission statements share a `(kind, holder, target)`.
    PermissionEffectConflict,
    /// A version-2 overtake statement's holder lacks the overtake tactic.
    PermissionOvertakeCapability,
    /// A version-2 lane-use obligation needs a facility with a fixed passing side.
    PermissionLaneUseObligation,
    /// A version-2 nominal-direction statement targets an `either` object.
    PermissionNominalEither,
}

impl DiagnosticCode {
    /// The stable string form of this code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedSchemaVersion => "E_SCHEMA_VERSION",
            Self::EmptyId => "E_ID_EMPTY",
            Self::DuplicateId => "E_ID_DUPLICATE",
            Self::NonFiniteCoordinate => "E_NON_FINITE",
            Self::NonPositiveValue => "E_NON_POSITIVE",
            Self::EmptyPath => "E_PATH_EMPTY",
            Self::DegeneratePath => "E_PATH_DEGENERATE",
            Self::PortalUnknownPath => "E_PORTAL_UNKNOWN_PATH",
            Self::PortalUnreachable => "E_PORTAL_UNREACHABLE",
            Self::PortalDuplicateEnd => "E_PORTAL_DUPLICATE_END",
            Self::PortalOverlap => "E_PORTAL_OVERLAP",
            Self::PolygonTooFewVertices => "E_POLYGON_TOO_FEW_VERTICES",
            Self::PolygonDegenerate => "E_POLYGON_DEGENERATE",
            Self::MovementUnknownPortal => "E_MOVEMENT_UNKNOWN_PORTAL",
            Self::MovementUnknownPath => "E_MOVEMENT_UNKNOWN_PATH",
            Self::MovementSelfLoop => "E_MOVEMENT_SELF_LOOP",
            Self::MovementPortalPathMismatch => "E_MOVEMENT_PORTAL_PATH_MISMATCH",
            Self::MovementStopLineInvalid => "E_MOVEMENT_STOP_LINE",
            Self::CrossingEmpty => "E_CROSSING_EMPTY",
            Self::CrossingUnknownRegion => "E_CROSSING_UNKNOWN_REGION",
            Self::CrossingUnknownMovement => "E_CROSSING_UNKNOWN_MOVEMENT",
            Self::CrossingPedestrianSignalEmpty => "E_CROSSING_PEDESTRIAN_SIGNAL_EMPTY",
            Self::CrossingPedestrianSignalPhaseInvalid => "E_CROSSING_PEDESTRIAN_SIGNAL_PHASE",
            Self::ConflictArity => "E_CONFLICT_ARITY",
            Self::ConflictUnknownMovement => "E_CONFLICT_UNKNOWN_MOVEMENT",
            Self::RuleUnknownMovement => "E_RULE_UNKNOWN_MOVEMENT",
            Self::RuleUnknownSignal => "E_RULE_UNKNOWN_SIGNAL",
            Self::RuleSignalMismatch => "E_RULE_SIGNAL_MISMATCH",
            Self::SignalEmpty => "E_SIGNAL_EMPTY",
            Self::SignalUnknownMovement => "E_SIGNAL_UNKNOWN_MOVEMENT",
            Self::SignalDuplicateHead => "E_SIGNAL_DUPLICATE_HEAD",
            Self::SignalPhaseEmpty => "E_SIGNAL_PHASE_EMPTY",
            Self::SignalUnknownHead => "E_SIGNAL_UNKNOWN_HEAD",
            Self::SignalPhaseDuplicateHead => "E_SIGNAL_PHASE_DUPLICATE_HEAD",
            Self::SignalPhaseMissingHead => "E_SIGNAL_PHASE_MISSING_HEAD",
            Self::SignalConflictingGreen => "E_SIGNAL_CONFLICTING_GREEN",
            Self::DemandUnknownPortal => "E_DEMAND_UNKNOWN_PORTAL",
            Self::DemandEmptyRoutes => "E_DEMAND_EMPTY_ROUTES",
            Self::DemandUnknownMovement => "E_DEMAND_UNKNOWN_MOVEMENT",
            Self::DemandRoutePortalMismatch => "E_DEMAND_ROUTE_PORTAL_MISMATCH",
            Self::DemandDuplicateRoute => "E_DEMAND_DUPLICATE_ROUTE",
            Self::ProfileRangeInvalid => "E_PROFILE_RANGE",
            Self::ProfileComplianceInvalid => "E_PROFILE_COMPLIANCE",
            Self::WaitingAreaUnknownRegion => "E_WAITING_AREA_UNKNOWN_REGION",
            Self::PedestrianRouteUnknownPortal => "E_PEDESTRIAN_ROUTE_UNKNOWN_PORTAL",
            Self::PedestrianRouteUnknownPath => "E_PEDESTRIAN_ROUTE_UNKNOWN_PATH",
            Self::PedestrianRoutePortalPathMismatch => "E_PEDESTRIAN_ROUTE_PORTAL_PATH_MISMATCH",
            Self::PedestrianRouteSelfLoop => "E_PEDESTRIAN_ROUTE_SELF_LOOP",
            Self::PedestrianRouteUnknownCrossing => "E_PEDESTRIAN_ROUTE_UNKNOWN_CROSSING",
            Self::PedestrianRouteUnknownWaitingArea => "E_PEDESTRIAN_ROUTE_UNKNOWN_WAITING_AREA",
            Self::PedestrianDemandUnknownPortal => "E_PEDESTRIAN_DEMAND_UNKNOWN_PORTAL",
            Self::PedestrianDemandEmptyRoutes => "E_PEDESTRIAN_DEMAND_EMPTY_ROUTES",
            Self::PedestrianDemandUnknownRoute => "E_PEDESTRIAN_DEMAND_UNKNOWN_ROUTE",
            Self::PedestrianDemandRoutePortalMismatch => {
                "E_PEDESTRIAN_DEMAND_ROUTE_PORTAL_MISMATCH"
            }
            Self::PedestrianDemandDuplicateRoute => "E_PEDESTRIAN_DEMAND_DUPLICATE_ROUTE",
            Self::MovementDirectionMismatch => "E_MOVEMENT_DIRECTION",
            Self::ModeTemplateBodyMotion => "E_MODE_TEMPLATE_BODY_MOTION",
            Self::ModeTemplateOccupancy => "E_MODE_TEMPLATE_OCCUPANCY",
            Self::ModeTemplateProfile => "E_MODE_TEMPLATE_PROFILE",
            Self::DemandUnknownMode => "E_DEMAND_UNKNOWN_MODE",
            Self::DemandUnknownPath => "E_DEMAND_UNKNOWN_PATH",
            Self::DemandChoiceMismatch => "E_DEMAND_CHOICE_MISMATCH",
            Self::DemandIntervalInvalid => "E_DEMAND_INTERVAL",
            Self::DemandPopulationInvalid => "E_DEMAND_POPULATION",
            Self::FacilityUnknownRegion => "E_FACILITY_UNKNOWN_REGION",
            Self::FacilityUnknownPath => "E_FACILITY_UNKNOWN_PATH",
            Self::FacilityUnknownMode => "E_FACILITY_UNKNOWN_MODE",
            Self::FacilityAccessEmpty => "E_FACILITY_ACCESS_EMPTY",
            Self::FacilityDirectionWithoutPath => "E_FACILITY_DIRECTION_WITHOUT_PATH",
            Self::FacilityWidthInvalid => "E_FACILITY_WIDTH",
            Self::FacilitySpeedLimitInvalid => "E_FACILITY_SPEED_LIMIT",
            Self::FacilityOutsideWorld => "E_FACILITY_OUTSIDE_WORLD",
            Self::FacilityTooNarrow => "E_FACILITY_TOO_NARROW",
            Self::FacilityCurvature => "E_FACILITY_CURVATURE",
            Self::FacilityConnectorUnknownFacility => "E_FACILITY_CONNECTOR_UNKNOWN_FACILITY",
            Self::FacilityConnectorWithoutReference => "E_FACILITY_CONNECTOR_WITHOUT_REFERENCE",
            Self::FacilityConnectorDiscontinuous => "E_FACILITY_CONNECTOR_DISCONTINUOUS",
            Self::FacilityUnreachableDirection => "E_FACILITY_UNREACHABLE_DIRECTION",
            Self::FacilityAccessDenied => "E_FACILITY_ACCESS_DENIED",
            Self::PermissionUnknownHolder => "E_PERMISSION_UNKNOWN_HOLDER",
            Self::PermissionUnknownTarget => "E_PERMISSION_UNKNOWN_TARGET",
            Self::PermissionRouteProhibited => "E_PERMISSION_ROUTE_PROHIBITED",
            Self::ProfileNonNegativeInvalid => "E_PROFILE_NON_NEGATIVE",
            Self::ModeTemplateLateral => "E_MODE_TEMPLATE_LATERAL",
            Self::ModeLateralClearance => "E_MODE_LATERAL_CLEARANCE",
            Self::ModeLateralHorizon => "E_MODE_LATERAL_HORIZON",
            Self::ModeWheelbase => "E_MODE_WHEELBASE",
            Self::ModeSteeringAngle => "E_MODE_STEERING_ANGLE",
            Self::ModeArticulatedSegments => "E_MODE_ARTICULATED_SEGMENTS",
            Self::ModeArticulationLimit => "E_MODE_ARTICULATION_LIMIT",
            Self::ManeuverPolicyMissing => "E_MANEUVER_POLICY_MISSING",
            Self::WrongWayPolicyMissing => "E_WRONG_WAY_POLICY_MISSING",
            Self::CommitPolicyInvalid => "E_COMMIT_POLICY",
            Self::CommitClearanceExceedsTarget => "E_COMMIT_CLEARANCE",
            Self::WrongWayPolicyInvalid => "E_WRONG_WAY_POLICY",
            Self::ClearanceBandThreshold => "E_CLEARANCE_BAND_THRESHOLD",
            Self::ClearanceBandOrder => "E_CLEARANCE_BAND_ORDER",
            Self::ClearanceBandModesEmpty => "E_CLEARANCE_BAND_MODES_EMPTY",
            Self::ClearanceBandUnknownMode => "E_CLEARANCE_BAND_UNKNOWN_MODE",
            Self::FacilityLateralWithoutReference => "E_FACILITY_LATERAL_WITHOUT_REFERENCE",
            Self::FacilityLateralCentered => "E_FACILITY_LATERAL_CENTERED",
            Self::FacilityPassingSideUnusable => "E_FACILITY_PASSING_SIDE_UNUSABLE",
            Self::FacilityAdjacencyUnknownFacility => "E_FACILITY_ADJACENCY_UNKNOWN_FACILITY",
            Self::FacilityAdjacencySelf => "E_FACILITY_ADJACENCY_SELF",
            Self::FacilityAdjacencyWithoutReference => "E_FACILITY_ADJACENCY_WITHOUT_REFERENCE",
            Self::FacilityAdjacencyDisjoint => "E_FACILITY_ADJACENCY_DISJOINT",
            Self::FacilityAdjacencySide => "E_FACILITY_ADJACENCY_SIDE",
            Self::PermissionTargetKind => "E_PERMISSION_TARGET_KIND",
            Self::PermissionEffectConflict => "E_PERMISSION_EFFECT_CONFLICT",
            Self::PermissionOvertakeCapability => "E_PERMISSION_OVERTAKE_CAPABILITY",
            Self::PermissionLaneUseObligation => "E_PERMISSION_LANE_USE_OBLIGATION",
            Self::PermissionNominalEither => "E_PERMISSION_NOMINAL_EITHER",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable diagnostic code.
    pub code: DiagnosticCode,
    /// Identifier of the offending source object, when one exists.
    pub object: Option<String>,
    /// Actionable, human-readable explanation.
    pub message: String,
}

impl Diagnostic {
    fn new(code: DiagnosticCode, object: Option<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            object,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.object {
            Some(object) => write!(f, "[{}] {}: {}", self.code, object, self.message),
            None => write!(f, "[{}] {}", self.code, self.message),
        }
    }
}

/// Validate a version-1 source scenario, returning every diagnostic in source order.
///
/// Version 1 keeps a direct read path until the migration step routes it through
/// version 2. An empty result means the scenario is safe to compile.
pub fn validate(source: &ScenarioSource) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    check_schema_version(source.schema_version, &source.id, &mut diagnostics);
    validate_ids(&source.id, v1_ids(source), &mut diagnostics);
    validate_common(&common_v1(source), &mut diagnostics);
    validate_demand(source, &mut diagnostics);
    validate_pedestrian_demand(source, &mut diagnostics);
    validate_profiles(source, &mut diagnostics);
    validate_pedestrian_profiles(source, &mut diagnostics);
    validate_population(source, &mut diagnostics);
    diagnostics
}

/// Validate a version-2 source scenario, returning every diagnostic in source order.
///
/// An empty result means the scenario is safe to compile.
pub fn validate_v2(source: &ScenarioSourceV2) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    check_schema_version(source.schema_version, &source.id, &mut diagnostics);
    validate_ids(&source.id, v2_ids(source), &mut diagnostics);
    validate_common(&common_v2(source), &mut diagnostics);
    validate_mode_templates(
        &source.mode_templates,
        source.maneuver_policy.as_ref(),
        &mut diagnostics,
    );
    validate_maneuver_policy(source, &mut diagnostics);
    validate_clearance_bands(source, &mut diagnostics);
    validate_facilities(source, &mut diagnostics);
    validate_facility_connectors(source, &mut diagnostics);
    validate_facility_adjacencies(source, &mut diagnostics);
    validate_facility_reachability(source, &mut diagnostics);
    validate_permissions(source, &mut diagnostics);
    validate_demand_v2(source, &mut diagnostics);
    diagnostics
}

/// Reject a schema version this build cannot read with the stable
/// `E_SCHEMA_VERSION` diagnostic.
fn check_schema_version(version: u32, id: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !(MIN_SUPPORTED_SCHEMA_VERSION..=SUPPORTED_SCHEMA_VERSION).contains(&version) {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnsupportedSchemaVersion,
            Some(id.to_owned()),
            format!(
                "schema_version {version} is not supported; this build reads versions \
                 {MIN_SUPPORTED_SCHEMA_VERSION} through {SUPPORTED_SCHEMA_VERSION}"
            ),
        ));
    }
}

/// The movement identity fields shared by the version-1 and version-2 shapes.
struct MovementView<'a> {
    id: &'a str,
    from: &'a str,
    to: &'a str,
    path: &'a str,
    stop_line_m: f64,
    direction: Option<MovementDirection>,
}

/// The collections shared by the version-1 and version-2 source shapes.
struct Common<'a> {
    paths: &'a [PathSource],
    portals: &'a [PortalSource],
    boundaries: &'a [PolygonSource],
    regions: &'a [PolygonSource],
    movements: Vec<MovementView<'a>>,
    crossings: &'a [CrossingSource],
    waiting_areas: &'a [WaitingAreaSource],
    pedestrian_routes: &'a [PedestrianRouteSource],
    conflict_regions: &'a [ConflictRegionSource],
    rules: &'a [RuleSource],
    signals: &'a [SignalSource],
}

fn common_v1(source: &ScenarioSource) -> Common<'_> {
    Common {
        paths: &source.paths,
        portals: &source.portals,
        boundaries: &source.boundaries,
        regions: &source.regions,
        movements: source
            .movements
            .iter()
            .map(|movement| MovementView {
                id: &movement.id,
                from: &movement.from,
                to: &movement.to,
                path: &movement.path,
                stop_line_m: movement.stop_line_m,
                direction: None,
            })
            .collect(),
        crossings: &source.crossings,
        waiting_areas: &source.waiting_areas,
        pedestrian_routes: &source.pedestrian_routes,
        conflict_regions: &source.conflict_regions,
        rules: &source.rules,
        signals: &source.signals,
    }
}

fn common_v2(source: &ScenarioSourceV2) -> Common<'_> {
    Common {
        paths: &source.paths,
        portals: &source.portals,
        boundaries: &source.boundaries,
        regions: &source.regions,
        movements: source
            .movements
            .iter()
            .map(|movement| MovementView {
                id: &movement.id,
                from: &movement.from,
                to: &movement.to,
                path: &movement.path,
                stop_line_m: movement.stop_line_m,
                direction: Some(movement.direction),
            })
            .collect(),
        crossings: &source.crossings,
        waiting_areas: &source.waiting_areas,
        pedestrian_routes: &source.pedestrian_routes,
        conflict_regions: &source.conflict_regions,
        rules: &source.rules,
        signals: &source.signals,
    }
}

fn validate_common(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    validate_paths(common, diagnostics);
    validate_portals(common, diagnostics);
    validate_polygons(common, diagnostics);
    validate_movements(common, diagnostics);
    validate_crossings(common, diagnostics);
    validate_conflict_regions(common, diagnostics);
    validate_rules(common, diagnostics);
    validate_signals(common, diagnostics);
    validate_waiting_areas(common, diagnostics);
    validate_pedestrian_routes(common, diagnostics);
}

/// Movement identifiers in authored order, for cross-reference checks.
fn movement_ids<'a>(common: &'a Common<'_>) -> Vec<&'a str> {
    common
        .movements
        .iter()
        .map(|movement| movement.id)
        .collect()
}

fn v1_ids(source: &ScenarioSource) -> impl Iterator<Item = &str> {
    source
        .paths
        .iter()
        .map(|path| path.id.as_str())
        .chain(source.portals.iter().map(|portal| portal.id.as_str()))
        .chain(source.boundaries.iter().map(|polygon| polygon.id.as_str()))
        .chain(source.regions.iter().map(|polygon| polygon.id.as_str()))
        .chain(source.movements.iter().map(|movement| movement.id.as_str()))
        .chain(source.crossings.iter().map(|crossing| crossing.id.as_str()))
        .chain(
            source
                .conflict_regions
                .iter()
                .map(|conflict| conflict.id.as_str()),
        )
        .chain(source.rules.iter().map(|rule| rule.id.as_str()))
        .chain(source.signals.iter().map(|signal| signal.id.as_str()))
        .chain(source.waiting_areas.iter().map(|area| area.id.as_str()))
        .chain(
            source
                .pedestrian_routes
                .iter()
                .map(|route| route.id.as_str()),
        )
        .chain(source.demand.iter().map(|demand| demand.id.as_str()))
        .chain(
            source
                .pedestrian_demand
                .iter()
                .map(|demand| demand.id.as_str()),
        )
}

fn v2_ids(source: &ScenarioSourceV2) -> impl Iterator<Item = &str> {
    source
        .paths
        .iter()
        .map(|path| path.id.as_str())
        .chain(source.portals.iter().map(|portal| portal.id.as_str()))
        .chain(source.boundaries.iter().map(|polygon| polygon.id.as_str()))
        .chain(source.regions.iter().map(|polygon| polygon.id.as_str()))
        .chain(source.movements.iter().map(|movement| movement.id.as_str()))
        .chain(source.crossings.iter().map(|crossing| crossing.id.as_str()))
        .chain(
            source
                .conflict_regions
                .iter()
                .map(|conflict| conflict.id.as_str()),
        )
        .chain(source.rules.iter().map(|rule| rule.id.as_str()))
        .chain(source.signals.iter().map(|signal| signal.id.as_str()))
        .chain(source.waiting_areas.iter().map(|area| area.id.as_str()))
        .chain(
            source
                .pedestrian_routes
                .iter()
                .map(|route| route.id.as_str()),
        )
        .chain(
            source
                .mode_templates
                .iter()
                .map(|template| template.id.as_str()),
        )
        .chain(
            source
                .facilities
                .iter()
                .map(|facility| facility.id.as_str()),
        )
        .chain(
            source
                .facility_connectors
                .iter()
                .map(|connector| connector.id.as_str()),
        )
        .chain(
            source
                .permissions
                .iter()
                .map(|permission| permission.id.as_str()),
        )
        .chain(source.demand.iter().map(|demand| demand.id.as_str()))
}

fn validate_ids<'a>(
    scenario_id: &str,
    authored: impl Iterator<Item = &'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if scenario_id.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::EmptyId,
            None,
            "scenario id must not be empty",
        ));
    }

    let mut seen: HashSet<&str> = HashSet::new();
    for id in authored {
        if id.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::EmptyId,
                None,
                "authored object identifier must not be empty",
            ));
        } else if !seen.insert(id) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateId,
                Some(id.to_owned()),
                format!("identifier '{id}' is used more than once"),
            ));
        }
    }
}

fn validate_paths(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for path in common.paths {
        let object = Some(path.id.clone());
        if path.points.len() < 2 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::EmptyPath,
                object,
                "a guide path needs at least two vertices",
            ));
            continue;
        }

        for point in &path.points {
            if !point.x.is_finite() || !point.y.is_finite() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::NonFiniteCoordinate,
                    Some(path.id.clone()),
                    format!(
                        "path '{}' has a non-finite coordinate ({}, {})",
                        path.id, point.x, point.y
                    ),
                ));
            }
        }
        let length = polyline_length(&path.points);
        if !length.is_finite() || length <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DegeneratePath,
                Some(path.id.clone()),
                format!("path '{}' has no traversable length", path.id),
            ));
        }
    }
}

/// Total traversable length of a polyline in metres.
fn polyline_length(points: &[PointSource]) -> f64 {
    points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

fn validate_portals(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for portal in common.portals {
        if !portal.width_m.is_finite() || portal.width_m <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonPositiveValue,
                Some(portal.id.clone()),
                format!(
                    "portal '{}' width_m must be finite and positive, got {}",
                    portal.id, portal.width_m
                ),
            ));
        }

        let Some(path) = common.paths.iter().find(|path| path.id == portal.path) else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PortalUnknownPath,
                Some(portal.id.clone()),
                format!(
                    "portal '{}' references undeclared path '{}'",
                    portal.id, portal.path
                ),
            ));
            continue;
        };

        let has_endpoint = match portal.end {
            PathEnd::Start => path.points.first(),
            PathEnd::End => path.points.last(),
        };
        let endpoint_ok = has_endpoint.is_some_and(|p| p.x.is_finite() && p.y.is_finite());
        let path_degenerate = path.points.len() < 2
            || path
                .points
                .windows(2)
                .all(|pair| pair[0].x == pair[1].x && pair[0].y == pair[1].y);
        if !endpoint_ok || path_degenerate {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PortalUnreachable,
                Some(portal.id.clone()),
                format!(
                    "portal '{}' cannot reach the '{}' end of path '{}'",
                    portal.id,
                    match portal.end {
                        PathEnd::Start => "start",
                        PathEnd::End => "end",
                    },
                    path.id
                ),
            ));
        }

        let duplicate = common
            .portals
            .iter()
            .filter(|other| other.path == portal.path && other.end == portal.end)
            .count()
            > 1;
        if duplicate {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PortalDuplicateEnd,
                Some(portal.id.clone()),
                format!(
                    "portal '{}' shares the '{}' end of path '{}' with another portal",
                    portal.id,
                    match portal.end {
                        PathEnd::Start => "start",
                        PathEnd::End => "end",
                    },
                    portal.path
                ),
            ));
        }
    }

    // Spawn/absorb points that physically overlap are an ambiguous admission
    // area, so reject them before time zero.
    for (index, portal) in common.portals.iter().enumerate() {
        let Some(position) = portal_position(portal, common.paths) else {
            continue;
        };
        for other in common.portals.iter().skip(index + 1) {
            let Some(other_position) = portal_position(other, common.paths) else {
                continue;
            };
            let gap = (position.x - other_position.x).hypot(position.y - other_position.y);
            let overlap_threshold = (portal.width_m + other.width_m) * 0.5;
            if gap < overlap_threshold {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PortalOverlap,
                    Some(portal.id.clone()),
                    format!(
                        "portal '{}' overlaps portal '{}' (gap {:.3} m, half-widths {:.3} m)",
                        portal.id, other.id, gap, overlap_threshold
                    ),
                ));
            }
        }
    }
}

/// World position of a portal's path endpoint, or `None` when the reference is
/// broken (which another check reports).
fn portal_position(portal: &PortalSource, paths: &[PathSource]) -> Option<PointSource> {
    let path = paths.iter().find(|path| path.id == portal.path)?;
    match portal.end {
        PathEnd::Start => path.points.first().copied(),
        PathEnd::End => path.points.last().copied(),
    }
}

fn validate_demand(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for demand in &source.demand {
        let object = Some(demand.id.clone());
        if !demand.rate_vph.is_finite() || demand.rate_vph <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonPositiveValue,
                object.clone(),
                format!(
                    "demand '{}' rate_vph must be finite and positive, got {}",
                    demand.id, demand.rate_vph
                ),
            ));
        }

        let portal = source.portals.iter().find(|p| p.id == demand.portal);
        if portal.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandUnknownPortal,
                object.clone(),
                format!(
                    "demand '{}' generates at undeclared portal '{}'",
                    demand.id, demand.portal
                ),
            ));
        }

        if demand.routes.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandEmptyRoutes,
                object.clone(),
                format!("demand '{}' lists no routes", demand.id),
            ));
        }

        let mut seen: HashSet<&str> = HashSet::new();
        for route in &demand.routes {
            if !route.weight.is_finite() || route.weight <= 0.0 {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::NonPositiveValue,
                    object.clone(),
                    format!(
                        "demand '{}' route to '{}' weight must be finite and positive, got {}",
                        demand.id, route.movement, route.weight
                    ),
                ));
            }
            if !seen.insert(route.movement.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::DemandDuplicateRoute,
                    object.clone(),
                    format!(
                        "demand '{}' lists movement '{}' more than once",
                        demand.id, route.movement
                    ),
                ));
            }
            match source
                .movements
                .iter()
                .find(|movement| movement.id == route.movement)
            {
                None => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::DemandUnknownMovement,
                    object.clone(),
                    format!(
                        "demand '{}' routes to undeclared movement '{}'",
                        demand.id, route.movement
                    ),
                )),
                Some(movement) if movement.from != demand.portal => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DemandRoutePortalMismatch,
                        object.clone(),
                        format!(
                            "demand '{}' generates at portal '{}' but movement '{}' starts at '{}'",
                            demand.id, demand.portal, route.movement, movement.from
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
}

fn validate_pedestrian_demand(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for demand in &source.pedestrian_demand {
        let object = Some(demand.id.clone());
        if !demand.rate_pph.is_finite() || demand.rate_pph <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonPositiveValue,
                object.clone(),
                format!(
                    "pedestrian demand '{}' rate_pph must be finite and positive, got {}",
                    demand.id, demand.rate_pph
                ),
            ));
        }

        if !source
            .portals
            .iter()
            .any(|portal| portal.id == demand.portal)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianDemandUnknownPortal,
                object.clone(),
                format!(
                    "pedestrian demand '{}' generates at undeclared portal '{}'",
                    demand.id, demand.portal
                ),
            ));
        }

        if demand.routes.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianDemandEmptyRoutes,
                object.clone(),
                format!("pedestrian demand '{}' lists no routes", demand.id),
            ));
        }

        let mut seen: HashSet<&str> = HashSet::new();
        for share in &demand.routes {
            if !share.weight.is_finite() || share.weight <= 0.0 {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::NonPositiveValue,
                    object.clone(),
                    format!(
                        "pedestrian demand '{}' route to '{}' weight must be finite and \
                         positive, got {}",
                        demand.id, share.route, share.weight
                    ),
                ));
            }
            if !seen.insert(share.route.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PedestrianDemandDuplicateRoute,
                    object.clone(),
                    format!(
                        "pedestrian demand '{}' lists route '{}' more than once",
                        demand.id, share.route
                    ),
                ));
            }
            match source
                .pedestrian_routes
                .iter()
                .find(|route| route.id == share.route)
            {
                None => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PedestrianDemandUnknownRoute,
                    object.clone(),
                    format!(
                        "pedestrian demand '{}' routes to undeclared route '{}'",
                        demand.id, share.route
                    ),
                )),
                Some(route) if route.from != demand.portal => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::PedestrianDemandRoutePortalMismatch,
                        object.clone(),
                        format!(
                            "pedestrian demand '{}' generates at portal '{}' but route '{}' \
                             starts at '{}'",
                            demand.id, demand.portal, share.route, route.from
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
}

fn validate_profiles(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let ranges = [
        ("speed_mps", source.profiles.speed_mps),
        ("length_m", source.profiles.length_m),
        ("width_m", source.profiles.width_m),
        ("time_gap_s", source.profiles.time_gap_s),
        ("max_accel_mps2", source.profiles.max_accel_mps2),
        (
            "comfortable_brake_mps2",
            source.profiles.comfortable_brake_mps2,
        ),
    ];
    for (field, range) in ranges {
        validate_profile_range(&source.id, field, range, diagnostics);
    }
    validate_compliance_range(
        &source.id,
        "profiles.compliance",
        source.profiles.compliance,
        diagnostics,
    );
}

fn validate_pedestrian_profiles(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let ranges = [
        (
            "pedestrian_profiles.radius_m",
            source.pedestrian_profiles.radius_m,
        ),
        (
            "pedestrian_profiles.speed_mps",
            source.pedestrian_profiles.speed_mps,
        ),
    ];
    for (field, range) in ranges {
        validate_profile_range(&source.id, field, range, diagnostics);
    }
    validate_compliance_range(
        &source.id,
        "pedestrian_profiles.compliance",
        source.pedestrian_profiles.compliance,
        diagnostics,
    );
}

/// A compliance propensity is a fraction, so unlike the physical ranges it is
/// allowed to be zero but must stay within `[0, 1]`.
fn validate_compliance_range(
    object: &str,
    field: &str,
    range: ProfileRangeSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let finite = range.min.is_finite() && range.max.is_finite();
    if !finite || range.min < 0.0 || range.max > 1.0 || range.min > range.max {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::ProfileComplianceInvalid,
            Some(object.to_owned()),
            format!(
                "{field} must be finite, within [0, 1], and non-inverted, got [{}, {}]",
                range.min, range.max
            ),
        ));
    }
}

/// A lateral-clearance preference is a distance, so unlike the strictly
/// positive physical ranges a zero clearance is allowed; only a negative,
/// non-finite, or inverted range is rejected.
fn validate_non_negative_range(
    object: &str,
    field: &str,
    range: ProfileRangeSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let finite = range.min.is_finite() && range.max.is_finite();
    if !finite || range.min < 0.0 || range.min > range.max {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::ProfileNonNegativeInvalid,
            Some(object.to_owned()),
            format!(
                "{field} must be finite, non-negative, and non-inverted, got [{}, {}]",
                range.min, range.max
            ),
        ));
    }
}

fn validate_profile_range(
    object: &str,
    field: &str,
    range: ProfileRangeSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let finite = range.min.is_finite() && range.max.is_finite();
    if !finite || range.min <= 0.0 || range.min > range.max {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::ProfileRangeInvalid,
            Some(object.to_owned()),
            format!(
                "profiles.{field} must be finite, positive, and non-inverted, got [{}, {}]",
                range.min, range.max
            ),
        ));
    }
}

fn validate_population(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    let population = &source.population;
    let non_positive = [
        ("vehicle_speed_mps", population.vehicle_speed_mps),
        ("vehicle_spacing_m", population.vehicle_spacing_m),
        ("vehicle_length_m", population.vehicle_length_m),
        ("vehicle_width_m", population.vehicle_width_m),
    ];
    for (field, value) in non_positive {
        if !value.is_finite() || value <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonPositiveValue,
                Some(source.id.clone()),
                format!("population.{field} must be finite and positive, got {value}"),
            ));
        }
    }
}

/// Signed area of a polygon ring via the shoelace formula.
fn polygon_area(points: &[PointSource]) -> f64 {
    let count = points.len();
    let mut sum = 0.0;
    for index in 0..count {
        let current = points[index];
        let next = points[(index + 1) % count];
        sum += current.x * next.y - next.x * current.y;
    }
    sum * 0.5
}

/// Validate one authored polygon ring.
fn validate_polygon(id: &str, points: &[PointSource], diagnostics: &mut Vec<Diagnostic>) {
    for point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonFiniteCoordinate,
                Some(id.to_owned()),
                format!(
                    "polygon '{id}' has a non-finite coordinate ({}, {})",
                    point.x, point.y
                ),
            ));
        }
    }
    if points.len() < 3 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::PolygonTooFewVertices,
            Some(id.to_owned()),
            format!(
                "polygon '{id}' needs at least three vertices, got {}",
                points.len()
            ),
        ));
        return;
    }
    let area = polygon_area(points);
    if !area.is_finite() || area.abs() <= f64::EPSILON {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::PolygonDegenerate,
            Some(id.to_owned()),
            format!("polygon '{id}' encloses no area"),
        ));
    }
}

fn validate_polygons(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for polygon in common.boundaries {
        validate_polygon(&polygon.id, &polygon.points, diagnostics);
    }
    for polygon in common.regions {
        validate_polygon(&polygon.id, &polygon.points, diagnostics);
    }
}

fn validate_movements(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for movement in &common.movements {
        let from = common
            .portals
            .iter()
            .find(|portal| portal.id == movement.from);
        let to = common
            .portals
            .iter()
            .find(|portal| portal.id == movement.to);
        let path = common.paths.iter().find(|path| path.id == movement.path);

        if from.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementUnknownPortal,
                Some(movement.id.to_owned()),
                format!(
                    "movement '{}' starts at undeclared portal '{}'",
                    movement.id, movement.from
                ),
            ));
        }
        if to.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementUnknownPortal,
                Some(movement.id.to_owned()),
                format!(
                    "movement '{}' ends at undeclared portal '{}'",
                    movement.id, movement.to
                ),
            ));
        }
        if movement.from == movement.to {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementSelfLoop,
                Some(movement.id.to_owned()),
                format!(
                    "movement '{}' starts and ends at portal '{}'",
                    movement.id, movement.from
                ),
            ));
        }
        if path.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementUnknownPath,
                Some(movement.id.to_owned()),
                format!(
                    "movement '{}' follows undeclared path '{}'",
                    movement.id, movement.path
                ),
            ));
        } else {
            for (role, portal) in [("from", from), ("to", to)] {
                if let Some(portal) = portal
                    && portal.path != movement.path
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::MovementPortalPathMismatch,
                        Some(movement.id.to_owned()),
                        format!(
                            "movement '{}' {role} portal '{}' attaches to path '{}', not '{}'",
                            movement.id, portal.id, portal.path, movement.path
                        ),
                    ));
                }
            }
        }

        if let Some(path) = path {
            let path_length = polyline_length(&path.points);
            if !movement.stop_line_m.is_finite()
                || movement.stop_line_m < 0.0
                || movement.stop_line_m > path_length
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::MovementStopLineInvalid,
                    Some(movement.id.to_owned()),
                    format!(
                        "movement '{}' stop_line_m must be finite, non-negative, and no greater \
                         than its path length {path_length}, got {}",
                        movement.id, movement.stop_line_m
                    ),
                ));
            }
        }

        // A version-2 direction must agree with the direction the portal order
        // already fixes on the movement's path.
        if let (Some(from), Some(direction)) = (from, movement.direction) {
            let expected = match from.end {
                PathEnd::Start => MovementDirection::Forward,
                PathEnd::End => MovementDirection::Reverse,
            };
            if direction != expected {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::MovementDirectionMismatch,
                    Some(movement.id.to_owned()),
                    format!(
                        "movement '{}' declares direction {:?} but its from portal fixes {:?}",
                        movement.id, direction, expected
                    ),
                ));
            }
        }
    }
}

fn validate_crossings(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let movements = movement_ids(common);
    for crossing in common.crossings {
        if crossing.movements.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::CrossingEmpty,
                Some(crossing.id.clone()),
                format!("crossing '{}' crosses no movements", crossing.id),
            ));
        }
        if !common
            .regions
            .iter()
            .any(|region| region.id == crossing.region)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::CrossingUnknownRegion,
                Some(crossing.id.clone()),
                format!(
                    "crossing '{}' occupies undeclared region '{}'",
                    crossing.id, crossing.region
                ),
            ));
        }
        for movement in &crossing.movements {
            if !movements.contains(&movement.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::CrossingUnknownMovement,
                    Some(crossing.id.clone()),
                    format!(
                        "crossing '{}' crosses undeclared movement '{}'",
                        crossing.id, movement
                    ),
                ));
            }
        }
        if let Some(signal) = &crossing.pedestrian_signal {
            if signal.phases.is_empty() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::CrossingPedestrianSignalEmpty,
                    Some(crossing.id.clone()),
                    format!(
                        "crossing '{}' pedestrian signal needs at least one phase",
                        crossing.id
                    ),
                ));
            }
            for (index, phase) in signal.phases.iter().enumerate() {
                if !phase.duration_s.is_finite() || phase.duration_s <= 0.0 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::CrossingPedestrianSignalPhaseInvalid,
                        Some(crossing.id.clone()),
                        format!(
                            "crossing '{}' pedestrian signal phase {} duration must be finite and positive, got {}",
                            crossing.id, index, phase.duration_s
                        ),
                    ));
                }
            }
        }
    }
}

fn validate_waiting_areas(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for area in common.waiting_areas {
        if !common.regions.iter().any(|region| region.id == area.region) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::WaitingAreaUnknownRegion,
                Some(area.id.clone()),
                format!(
                    "waiting area '{}' occupies undeclared region '{}'",
                    area.id, area.region
                ),
            ));
        }
    }
}

fn validate_pedestrian_routes(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let crossings: Vec<&str> = common
        .crossings
        .iter()
        .map(|crossing| crossing.id.as_str())
        .collect();
    let waiting_areas: Vec<&str> = common
        .waiting_areas
        .iter()
        .map(|area| area.id.as_str())
        .collect();
    for route in common.pedestrian_routes {
        let object = Some(route.id.clone());
        let from = common.portals.iter().find(|portal| portal.id == route.from);
        let to = common.portals.iter().find(|portal| portal.id == route.to);
        let path = common.paths.iter().find(|path| path.id == route.path);

        if from.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianRouteUnknownPortal,
                object.clone(),
                format!(
                    "pedestrian route '{}' starts at undeclared portal '{}'",
                    route.id, route.from
                ),
            ));
        }
        if to.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianRouteUnknownPortal,
                object.clone(),
                format!(
                    "pedestrian route '{}' ends at undeclared portal '{}'",
                    route.id, route.to
                ),
            ));
        }
        if route.from == route.to {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianRouteSelfLoop,
                object.clone(),
                format!(
                    "pedestrian route '{}' starts and ends at portal '{}'",
                    route.id, route.from
                ),
            ));
        }
        if path.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianRouteUnknownPath,
                object.clone(),
                format!(
                    "pedestrian route '{}' follows undeclared path '{}'",
                    route.id, route.path
                ),
            ));
        } else {
            for (role, portal) in [("from", from), ("to", to)] {
                if let Some(portal) = portal
                    && portal.path != route.path
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::PedestrianRoutePortalPathMismatch,
                        object.clone(),
                        format!(
                            "pedestrian route '{}' {role} portal '{}' attaches to path '{}', \
                             not '{}'",
                            route.id, portal.id, portal.path, route.path
                        ),
                    ));
                }
            }
        }

        for crossing in &route.crossings {
            if !crossings.contains(&crossing.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PedestrianRouteUnknownCrossing,
                    object.clone(),
                    format!(
                        "pedestrian route '{}' traverses undeclared crossing '{}'",
                        route.id, crossing
                    ),
                ));
            }
        }
        for waiting_area in &route.waiting_areas {
            if !waiting_areas.contains(&waiting_area.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PedestrianRouteUnknownWaitingArea,
                    object.clone(),
                    format!(
                        "pedestrian route '{}' stages at undeclared waiting area '{}'",
                        route.id, waiting_area
                    ),
                ));
            }
        }
    }
}

fn validate_conflict_regions(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let movements = movement_ids(common);
    for conflict in common.conflict_regions {
        validate_polygon(&conflict.id, &conflict.points, diagnostics);
        let distinct =
            conflict.movements.len() == 2 && conflict.movements[0] != conflict.movements[1];
        if !distinct {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ConflictArity,
                Some(conflict.id.clone()),
                format!(
                    "conflict region '{}' must name exactly two distinct movements, got [{}]",
                    conflict.id,
                    conflict.movements.join(", ")
                ),
            ));
        }
        for movement in &conflict.movements {
            if !movements.contains(&movement.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ConflictUnknownMovement,
                    Some(conflict.id.clone()),
                    format!(
                        "conflict region '{}' names undeclared movement '{}'",
                        conflict.id, movement
                    ),
                ));
            }
        }
    }
}

fn validate_rules(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let movements = movement_ids(common);
    let signals: Vec<&str> = common
        .signals
        .iter()
        .map(|signal| signal.id.as_str())
        .collect();
    for rule in common.rules {
        if !movements.contains(&rule.movement.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::RuleUnknownMovement,
                Some(rule.id.clone()),
                format!(
                    "rule '{}' governs undeclared movement '{}'",
                    rule.id, rule.movement
                ),
            ));
        }
        match (rule.kind, rule.signal.as_deref()) {
            (RuleKind::Signal, Some(signal)) => {
                if !signals.contains(&signal) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::RuleUnknownSignal,
                        Some(rule.id.clone()),
                        format!("rule '{}' names undeclared signal '{}'", rule.id, signal),
                    ));
                }
            }
            (RuleKind::Signal, None) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::RuleSignalMismatch,
                Some(rule.id.clone()),
                format!(
                    "rule '{}' is signal-controlled but names no signal",
                    rule.id
                ),
            )),
            (_, Some(signal)) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::RuleSignalMismatch,
                Some(rule.id.clone()),
                format!(
                    "rule '{}' is not signal-controlled but names signal '{}'",
                    rule.id, signal
                ),
            )),
            (_, None) => {}
        }
    }
}

fn validate_signals(common: &Common<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let movements = movement_ids(common);
    for signal in common.signals {
        if signal.heads.is_empty() || signal.phases.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SignalEmpty,
                Some(signal.id.clone()),
                format!(
                    "signal '{}' needs at least one head and one phase",
                    signal.id
                ),
            ));
        }

        let mut head_ids: HashSet<&str> = HashSet::new();
        for head in &signal.heads {
            if !head_ids.insert(head.id.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::SignalDuplicateHead,
                    Some(signal.id.clone()),
                    format!(
                        "signal '{}' declares head '{}' more than once",
                        signal.id, head.id
                    ),
                ));
            }
            if !movements.contains(&head.movement.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::SignalUnknownMovement,
                    Some(signal.id.clone()),
                    format!(
                        "signal '{}' head '{}' controls undeclared movement '{}'",
                        signal.id, head.id, head.movement
                    ),
                ));
            }
        }

        for (index, phase) in signal.phases.iter().enumerate() {
            if !phase.duration_s.is_finite() || phase.duration_s <= 0.0 {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::NonPositiveValue,
                    Some(signal.id.clone()),
                    format!(
                        "signal '{}' phase {} duration must be finite and positive, got {}",
                        signal.id, index, phase.duration_s
                    ),
                ));
            }
            if phase.states.is_empty() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::SignalPhaseEmpty,
                    Some(signal.id.clone()),
                    format!("signal '{}' phase {} lists no states", signal.id, index),
                ));
            }

            let mut seen: HashSet<&str> = HashSet::new();
            for state in &phase.states {
                if !head_ids.contains(state.head.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::SignalUnknownHead,
                        Some(signal.id.clone()),
                        format!(
                            "signal '{}' phase {} names undeclared head '{}'",
                            signal.id, index, state.head
                        ),
                    ));
                } else if !seen.insert(state.head.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::SignalPhaseDuplicateHead,
                        Some(signal.id.clone()),
                        format!(
                            "signal '{}' phase {} lists head '{}' more than once",
                            signal.id, index, state.head
                        ),
                    ));
                }
            }
            for head in &signal.heads {
                if !seen.contains(head.id.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::SignalPhaseMissingHead,
                        Some(signal.id.clone()),
                        format!(
                            "signal '{}' phase {} omits head '{}'",
                            signal.id, index, head.id
                        ),
                    ));
                }
            }

            for conflict in common.conflict_regions {
                if conflict.movements.len() != 2 {
                    continue;
                }
                let green = |movement: &str| {
                    signal.heads.iter().any(|head| {
                        head.movement == movement
                            && phase.states.iter().any(|state| {
                                state.head == head.id && state.color == SignalColor::Green
                            })
                    })
                };
                if green(&conflict.movements[0]) && green(&conflict.movements[1]) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::SignalConflictingGreen,
                        Some(signal.id.clone()),
                        format!(
                            "signal '{}' phase {} shows green to conflicting movements '{}' and '{}'",
                            signal.id, index, conflict.movements[0], conflict.movements[1]
                        ),
                    ));
                }
            }
        }
    }
}

/// The profile parameters each version-2 body/motion family requires.
///
/// Shared with the template compiler ([`crate::mode_template`]) so the
/// parameters a family needs have a single definition. The narrow wheeled
/// family — a capsule body steering on a reference path — additionally needs
/// its steering response and lateral-clearance preference alongside the
/// Increment 0 wheeled parameters. A `single_body_wheeled` template that
/// declares a `lateral` object additionally needs the lateral maneuver set —
/// `steering_rate_max_rad_s`, `lateral_accel_max_mps2`, and
/// `lateral_clearance_m` — whichever body it carries; every other family keeps
/// its Increment 0 or Increment 1 set.
///
/// `lateral` is the caller's answer to whether the template declares a
/// `lateral` object, so the required set depends only on the body/motion pair
/// and that one fact, never on the template id.
pub(crate) fn required_profile_params(
    body: &ModeBodySource,
    motion: MotionKind,
    lateral: bool,
) -> &'static [&'static str] {
    match (body, motion) {
        (ModeBodySource::Capsule { .. }, MotionKind::SingleBodyWheeled) if lateral => &[
            "speed_mps",
            "max_accel_mps2",
            "comfortable_brake_mps2",
            "time_gap_s",
            "steering_rate_max_rad_s",
            "lateral_accel_max_mps2",
            "lateral_clearance_m",
            "compliance",
        ],
        (ModeBodySource::Capsule { .. }, MotionKind::SingleBodyWheeled) => &[
            "speed_mps",
            "max_accel_mps2",
            "comfortable_brake_mps2",
            "time_gap_s",
            "steering_rate_max_rad_s",
            "lateral_clearance_m",
            "compliance",
        ],
        (_, MotionKind::SingleBodyWheeled) if lateral => &[
            "speed_mps",
            "max_accel_mps2",
            "comfortable_brake_mps2",
            "time_gap_s",
            "steering_rate_max_rad_s",
            "lateral_accel_max_mps2",
            "lateral_clearance_m",
            "compliance",
        ],
        (_, MotionKind::SingleBodyWheeled) => &[
            "speed_mps",
            "max_accel_mps2",
            "comfortable_brake_mps2",
            "time_gap_s",
            "compliance",
        ],
        (_, MotionKind::HolonomicWalking) => &["speed_mps", "compliance"],
        // Increment 3 claims no new dynamics for the articulated-wheeled
        // family: it reuses the plain wheeled parameter set, matching
        // `compiled_profile`'s choice of `AgentBehaviorProfile::wheeled`.
        (_, MotionKind::ArticulatedWheeled) => &[
            "speed_mps",
            "max_accel_mps2",
            "comfortable_brake_mps2",
            "time_gap_s",
            "compliance",
        ],
    }
}

/// Whether an authored tactic selects a lateral maneuver.
///
/// These are the four Increment 2 values whose presence couples a template to
/// the scenario-scoped `maneuver_policy`; `follow`, `stop`, and `yield` keep
/// their Increment 0 meaning.
fn is_lateral_tactic(tactic: TacticKind) -> bool {
    matches!(
        tactic,
        TacticKind::ChangeLane
            | TacticKind::Overtake
            | TacticKind::Pass
            | TacticKind::ReverseDirection
    )
}

/// Validate version-2 mode templates: body and motion must coexist, the
/// profile parameters must be exactly those the motion family and lateral
/// policy use, and a lateral template must be a wheeled, capable one with the
/// maneuver policy it needs.
fn validate_mode_templates(
    templates: &[ModeTemplateSource],
    maneuver_policy: Option<&ManeuverPolicySource>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for template in templates {
        if !body_motion_pair_has_family(&template.body, template.motion) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ModeTemplateBodyMotion,
                Some(template.id.clone()),
                format!(
                    "mode template '{}' pairs an incompatible body kind with its motion family",
                    template.id
                ),
            ));
        }

        let lateral_capable = template.tactics.iter().copied().any(is_lateral_tactic);
        if template.lateral.is_some()
            && (template.motion != MotionKind::SingleBodyWheeled || !lateral_capable)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ModeTemplateLateral,
                Some(template.id.clone()),
                format!(
                    "mode template '{}' declares lateral maneuver parameters but is not a \
                     wheeled template with a lateral tactic",
                    template.id
                ),
            ));
        }
        if lateral_capable && maneuver_policy.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ManeuverPolicyMissing,
                Some(template.id.clone()),
                format!(
                    "mode template '{}' declares a lateral tactic but the document authors no \
                     maneuver_policy",
                    template.id
                ),
            ));
        }
        if template.tactics.contains(&TacticKind::ReverseDirection)
            && maneuver_policy
                .and_then(|policy| policy.wrong_way)
                .is_none()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::WrongWayPolicyMissing,
                Some(template.id.clone()),
                format!(
                    "mode template '{}' declares reverse_direction but the document authors no \
                     maneuver_policy.wrong_way",
                    template.id
                ),
            ));
        }
        if let Some(lateral) = &template.lateral {
            if !lateral.target_clearance_m.is_finite() || lateral.target_clearance_m < 0.0 {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ModeLateralClearance,
                    Some(template.id.clone()),
                    format!(
                        "mode template '{}' lateral.target_clearance_m must be finite and \
                         non-negative, got {}",
                        template.id, lateral.target_clearance_m
                    ),
                ));
            }
            if !lateral.horizon_s.is_finite() || lateral.horizon_s <= 0.0 {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ModeLateralHorizon,
                    Some(template.id.clone()),
                    format!(
                        "mode template '{}' lateral.horizon_s must be finite and strictly \
                         positive, got {}",
                        template.id, lateral.horizon_s
                    ),
                ));
            }
        }

        // Increment 3 axle geometry: a wheelbase and a maximum steering angle
        // are authored together, only on a wheeled motion, and each is a
        // strictly positive non-inverted range (the angle inside `(0, pi/2)`).
        match (&template.wheelbase_m, &template.steering_angle_max_rad) {
            (Some(wheelbase), Some(angle)) => {
                if template.motion != MotionKind::SingleBodyWheeled {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ModeWheelbase,
                        Some(template.id.clone()),
                        format!(
                            "mode template '{}' authors axle geometry but is not a \
                             single_body_wheeled motion",
                            template.id
                        ),
                    ));
                }
                if !wheelbase.min.is_finite()
                    || !wheelbase.max.is_finite()
                    || wheelbase.min <= 0.0
                    || wheelbase.min > wheelbase.max
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ModeWheelbase,
                        Some(template.id.clone()),
                        format!(
                            "mode template '{}' wheelbase_m must be finite, positive, and \
                             non-inverted, got [{}, {}]",
                            template.id, wheelbase.min, wheelbase.max
                        ),
                    ));
                }
                if !angle.min.is_finite()
                    || !angle.max.is_finite()
                    || angle.min <= 0.0
                    || angle.max >= std::f64::consts::FRAC_PI_2
                    || angle.min > angle.max
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ModeSteeringAngle,
                        Some(template.id.clone()),
                        format!(
                            "mode template '{}' steering_angle_max_rad must be finite, inside \
                             (0, pi/2), and non-inverted, got [{}, {}]",
                            template.id, angle.min, angle.max
                        ),
                    ));
                }
            }
            (Some(_), None) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::ModeWheelbase,
                Some(template.id.clone()),
                format!(
                    "mode template '{}' authors wheelbase_m without its steering_angle_max_rad \
                     companion, so the wheelbase alone bounds no curvature",
                    template.id
                ),
            )),
            (None, Some(_)) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::ModeSteeringAngle,
                Some(template.id.clone()),
                format!(
                    "mode template '{}' authors steering_angle_max_rad without its wheelbase_m \
                     companion, so the angle alone bounds no curvature",
                    template.id
                ),
            )),
            (None, None) => {}
        }

        // Increment 3 articulated-chain geometry: at least a tractor and one
        // trailer, hitch offsets on exactly the trailing segments, every
        // segment dimension finite and positive, and a physically sane
        // articulation limit. Off-tracking and corner curvature under
        // articulation stay out of scope; only the authored geometry itself is
        // checked here.
        if let ModeBodySource::ArticulatedChain {
            segments,
            articulation_limit_rad,
        } = &template.body
        {
            if segments.len() < 2 {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ModeArticulatedSegments,
                    Some(template.id.clone()),
                    format!(
                        "mode template '{}' articulated chain must have at least two segments \
                         (a tractor and one trailer), got {}",
                        template.id,
                        segments.len()
                    ),
                ));
            }
            for (index, segment) in segments.iter().enumerate() {
                for (label, dimension) in
                    [("length_m", segment.length_m), ("width_m", segment.width_m)]
                {
                    if !dimension.min.is_finite()
                        || !dimension.max.is_finite()
                        || dimension.min <= 0.0
                        || dimension.min > dimension.max
                    {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::ModeArticulatedSegments,
                            Some(template.id.clone()),
                            format!(
                                "mode template '{}' articulated segment {index} {label} must be \
                                 finite, positive, and non-inverted, got [{}, {}]",
                                template.id, dimension.min, dimension.max
                            ),
                        ));
                    }
                }
                match (index, &segment.hitch_offset_m) {
                    (0, Some(_)) => diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ModeArticulatedSegments,
                        Some(template.id.clone()),
                        format!(
                            "mode template '{}' articulated segment 0 is the lead segment and \
                             must not author a hitch offset",
                            template.id
                        ),
                    )),
                    (0, None) => {}
                    (_, None) => diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ModeArticulatedSegments,
                        Some(template.id.clone()),
                        format!(
                            "mode template '{}' articulated segment {index} trails the lead \
                             segment and must author a hitch offset",
                            template.id
                        ),
                    )),
                    (_, Some(offset)) => {
                        if !offset.min.is_finite()
                            || !offset.max.is_finite()
                            || offset.min <= 0.0
                            || offset.min > offset.max
                        {
                            diagnostics.push(Diagnostic::new(
                                DiagnosticCode::ModeArticulatedSegments,
                                Some(template.id.clone()),
                                format!(
                                    "mode template '{}' articulated segment {index} \
                                     hitch_offset_m must be finite, positive, and non-inverted, \
                                     got [{}, {}]",
                                    template.id, offset.min, offset.max
                                ),
                            ));
                        }
                    }
                }
            }

            // The articulation limit must be a strictly positive angle less
            // than a full reversal (`pi`): at `pi` a trailing segment would
            // fold flat back onto the one ahead of it, which is a jackknife,
            // not a bound on one, so `(0, pi)` is the widest physically
            // meaningful range for this field.
            if !articulation_limit_rad.min.is_finite()
                || !articulation_limit_rad.max.is_finite()
                || articulation_limit_rad.min <= 0.0
                || articulation_limit_rad.max >= std::f64::consts::PI
                || articulation_limit_rad.min > articulation_limit_rad.max
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ModeArticulationLimit,
                    Some(template.id.clone()),
                    format!(
                        "mode template '{}' articulation_limit_rad must be finite, inside \
                         (0, pi), and non-inverted, got [{}, {}]",
                        template.id, articulation_limit_rad.min, articulation_limit_rad.max
                    ),
                ));
            }
        }

        let required =
            required_profile_params(&template.body, template.motion, template.lateral.is_some());
        for name in required {
            if !template.profiles.contains_key(*name) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ModeTemplateProfile,
                    Some(template.id.clone()),
                    format!(
                        "mode template '{}' must declare profile '{name}' for its motion family",
                        template.id
                    ),
                ));
            }
        }
        for (name, range) in &template.profiles {
            if !required.contains(&name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ModeTemplateProfile,
                    Some(template.id.clone()),
                    format!(
                        "mode template '{}' declares profile '{name}', which its motion family \
                         does not use",
                        template.id
                    ),
                ));
                continue;
            }
            if name == "compliance" {
                validate_compliance_range(&template.id, name, *range, diagnostics);
            } else if name == "lateral_clearance_m" {
                validate_non_negative_range(&template.id, name, *range, diagnostics);
            } else {
                validate_profile_range(&template.id, name, *range, diagnostics);
            }
        }
    }
}

/// Validate the scenario-scoped maneuver policy: the commit and wrong-way
/// values must be well-formed, and the commit clearance floor must not exceed
/// any lateral mode's target clearance, which would abort every maneuver.
fn validate_maneuver_policy(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    let Some(policy) = &source.maneuver_policy else {
        return;
    };

    if let Some(commit) = &policy.commit {
        if !commit.min_predicted_clearance_m.is_finite() || commit.min_predicted_clearance_m < 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::CommitPolicyInvalid,
                None,
                format!(
                    "maneuver_policy.commit.min_predicted_clearance_m must be finite and \
                     non-negative, got {}",
                    commit.min_predicted_clearance_m
                ),
            ));
        }
        if !commit.hold_timeout_s.is_finite() || commit.hold_timeout_s <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::CommitPolicyInvalid,
                None,
                format!(
                    "maneuver_policy.commit.hold_timeout_s must be finite and strictly positive, \
                     got {}",
                    commit.hold_timeout_s
                ),
            ));
        }
        for template in &source.mode_templates {
            if let Some(lateral) = &template.lateral
                && commit.min_predicted_clearance_m.is_finite()
                && lateral.target_clearance_m.is_finite()
                && commit.min_predicted_clearance_m > lateral.target_clearance_m
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::CommitClearanceExceedsTarget,
                    Some(template.id.clone()),
                    format!(
                        "maneuver_policy.commit.min_predicted_clearance_m {} exceeds the target \
                         clearance {} of mode '{}', which would abort every maneuver",
                        commit.min_predicted_clearance_m, lateral.target_clearance_m, template.id
                    ),
                ));
            }
        }
    }

    if let Some(wrong_way) = &policy.wrong_way {
        let valid = wrong_way.min_time_saving_s.is_finite()
            && wrong_way.min_time_saving_s >= 0.0
            && wrong_way.max_opposing_density_per_km.is_finite()
            && wrong_way.max_opposing_density_per_km >= 0.0
            && wrong_way.urgency.is_finite()
            && (0.0..=1.0).contains(&wrong_way.urgency);
        if !valid {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::WrongWayPolicyInvalid,
                None,
                format!(
                    "maneuver_policy.wrong_way needs a finite non-negative min_time_saving_s and \
                     max_opposing_density_per_km, and an urgency within [0, 1]; got {} s, {} \
                     per km, {}",
                    wrong_way.min_time_saving_s,
                    wrong_way.max_opposing_density_per_km,
                    wrong_way.urgency
                ),
            ));
        }
    }
}

/// Validate version-2 clearance bands: each threshold is finite and strictly
/// positive, bands are declared in strictly increasing order, and every mode a
/// band names is declared.
fn validate_clearance_bands(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    for (index, band) in source.clearance_bands.iter().enumerate() {
        let object = Some(band.id.clone());
        if !band.threshold_m.is_finite() || band.threshold_m <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ClearanceBandThreshold,
                object.clone(),
                format!(
                    "clearance band '{}' threshold_m must be finite and strictly positive, got {}",
                    band.id, band.threshold_m
                ),
            ));
        }
        if index > 0 {
            let previous = source.clearance_bands[index - 1].threshold_m;
            if band.threshold_m.is_finite() && previous.is_finite() && band.threshold_m <= previous
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ClearanceBandOrder,
                    object.clone(),
                    format!(
                        "clearance band '{}' threshold_m {} must be strictly greater than the \
                         preceding band's {}",
                        band.id, band.threshold_m, previous
                    ),
                ));
            }
        }
        match &band.applies_to_modes {
            Some(modes) if modes.is_empty() => diagnostics.push(Diagnostic::new(
                DiagnosticCode::ClearanceBandModesEmpty,
                object.clone(),
                format!(
                    "clearance band '{}' declares an empty applies_to_modes; omit the field to \
                     apply to every mode pair",
                    band.id
                ),
            )),
            Some(modes) => {
                for mode in modes {
                    if !source
                        .mode_templates
                        .iter()
                        .any(|template| template.id == *mode)
                    {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::ClearanceBandUnknownMode,
                            object.clone(),
                            format!(
                                "clearance band '{}' applies to undeclared mode template '{}'",
                                band.id, mode
                            ),
                        ));
                    }
                }
            }
            None => {}
        }
    }
}

/// Validate the movement shares of one version-2 vehicle rate demand.
fn validate_demand_movements(
    demand_id: &str,
    portal: &str,
    shares: &[RouteShareSource],
    source: &ScenarioSourceV2,
    object: &Option<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if shares.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::DemandEmptyRoutes,
            object.clone(),
            format!("demand '{demand_id}' lists no movements"),
        ));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for share in shares {
        if !share.weight.is_finite() || share.weight <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonPositiveValue,
                object.clone(),
                format!(
                    "demand '{demand_id}' movement '{}' weight must be finite and positive, got {}",
                    share.movement, share.weight
                ),
            ));
        }
        if !seen.insert(share.movement.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandDuplicateRoute,
                object.clone(),
                format!(
                    "demand '{demand_id}' lists movement '{}' more than once",
                    share.movement
                ),
            ));
        }
        match source
            .movements
            .iter()
            .find(|movement| movement.id == share.movement)
        {
            None => diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandUnknownMovement,
                object.clone(),
                format!(
                    "demand '{demand_id}' routes to undeclared movement '{}'",
                    share.movement
                ),
            )),
            Some(movement) if movement.from != portal => diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandRoutePortalMismatch,
                object.clone(),
                format!(
                    "demand '{demand_id}' generates at portal '{portal}' but movement '{}' \
                     starts at '{}'",
                    share.movement, movement.from
                ),
            )),
            Some(_) => {}
        }
    }
}

/// Validate the route shares of one version-2 pedestrian rate demand.
fn validate_demand_routes(
    demand_id: &str,
    portal: &str,
    shares: &[PedestrianRouteShareSource],
    source: &ScenarioSourceV2,
    object: &Option<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if shares.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::PedestrianDemandEmptyRoutes,
            object.clone(),
            format!("demand '{demand_id}' lists no routes"),
        ));
    }
    let mut seen: HashSet<&str> = HashSet::new();
    for share in shares {
        if !share.weight.is_finite() || share.weight <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::NonPositiveValue,
                object.clone(),
                format!(
                    "demand '{demand_id}' route '{}' weight must be finite and positive, got {}",
                    share.route, share.weight
                ),
            ));
        }
        if !seen.insert(share.route.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianDemandDuplicateRoute,
                object.clone(),
                format!(
                    "demand '{demand_id}' lists route '{}' more than once",
                    share.route
                ),
            ));
        }
        match source
            .pedestrian_routes
            .iter()
            .find(|route| route.id == share.route)
        {
            None => diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianDemandUnknownRoute,
                object.clone(),
                format!(
                    "demand '{demand_id}' routes to undeclared route '{}'",
                    share.route
                ),
            )),
            Some(route) if route.from != portal => diagnostics.push(Diagnostic::new(
                DiagnosticCode::PedestrianDemandRoutePortalMismatch,
                object.clone(),
                format!(
                    "demand '{demand_id}' generates at portal '{portal}' but route '{}' starts \
                     at '{}'",
                    share.route, route.from
                ),
            )),
            Some(_) => {}
        }
    }
}

/// Validate version-2 mode-tagged demand: declared mode template, declared
/// portal or path, valid interval, and a choice matching the mode's family.
fn validate_demand_v2(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    let mut population_ids: Vec<&str> = Vec::new();
    for entry in &source.demand {
        let object = Some(entry.id.clone());
        let template = source
            .mode_templates
            .iter()
            .find(|template| template.id == entry.mode);
        if template.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandUnknownMode,
                object.clone(),
                format!(
                    "demand '{}' references undeclared mode template '{}'",
                    entry.id, entry.mode
                ),
            ));
        }

        match &entry.spawn {
            DemandSpawnSource::Rate(rate) => {
                if !rate.rate_per_hour.is_finite() || rate.rate_per_hour <= 0.0 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::NonPositiveValue,
                        object.clone(),
                        format!(
                            "demand '{}' rate_per_hour must be finite and positive, got {}",
                            entry.id, rate.rate_per_hour
                        ),
                    ));
                }
                let interval = rate.interval_s;
                if !interval.start_s.is_finite()
                    || interval.start_s < 0.0
                    || interval
                        .end_s
                        .is_some_and(|end| !end.is_finite() || end <= interval.start_s)
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DemandIntervalInvalid,
                        object.clone(),
                        format!(
                            "demand '{}' interval_s needs a finite non-negative start and a null \
                             or greater end",
                            entry.id
                        ),
                    ));
                }
                if !source.portals.iter().any(|portal| portal.id == rate.portal) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DemandUnknownPortal,
                        object.clone(),
                        format!(
                            "demand '{}' generates at undeclared portal '{}'",
                            entry.id, rate.portal
                        ),
                    ));
                }
                match (&rate.choice, template.map(|template| template.motion)) {
                    (
                        DemandChoiceSource::Movements(shares),
                        Some(MotionKind::SingleBodyWheeled | MotionKind::ArticulatedWheeled),
                    ) => validate_demand_movements(
                        &entry.id,
                        &rate.portal,
                        shares,
                        source,
                        &object,
                        diagnostics,
                    ),
                    (DemandChoiceSource::Routes(shares), Some(MotionKind::HolonomicWalking)) => {
                        validate_demand_routes(
                            &entry.id,
                            &rate.portal,
                            shares,
                            source,
                            &object,
                            diagnostics,
                        )
                    }
                    (_, Some(_)) => diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DemandChoiceMismatch,
                        object.clone(),
                        format!(
                            "demand '{}' choice does not match the '{}' mode's motion family",
                            entry.id, entry.mode
                        ),
                    )),
                    (_, None) => {}
                }
            }
            DemandSpawnSource::Population(population) => {
                population_ids.push(entry.id.as_str());
                if !source.paths.iter().any(|path| path.id == population.path) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DemandUnknownPath,
                        object.clone(),
                        format!(
                            "demand '{}' places its population on undeclared path '{}'",
                            entry.id, population.path
                        ),
                    ));
                }
                if !population.speed_mps.is_finite() || population.speed_mps <= 0.0 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::NonPositiveValue,
                        object.clone(),
                        format!(
                            "demand '{}' population speed_mps must be finite and positive, got {}",
                            entry.id, population.speed_mps
                        ),
                    ));
                }
                if !population.spacing_m.is_finite() || population.spacing_m <= 0.0 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::NonPositiveValue,
                        object.clone(),
                        format!(
                            "demand '{}' population spacing_m must be finite and positive, got {}",
                            entry.id, population.spacing_m
                        ),
                    ));
                }
                if template.is_some_and(|template| template.motion != MotionKind::SingleBodyWheeled)
                {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DemandChoiceMismatch,
                        object.clone(),
                        format!(
                            "demand '{}' population spawn requires a single_body_wheeled mode",
                            entry.id
                        ),
                    ));
                }
            }
        }
    }

    // A population reproduces the Phase 1 walking skeleton, which is incompatible
    // with a rate demand in the same document; compiling both would silently drop
    // one of them.
    if !population_ids.is_empty() && source.demand.len() > 1 {
        for id in population_ids {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DemandPopulationInvalid,
                Some(id.to_owned()),
                format!(
                    "demand '{id}' declares a population spawn alongside other demand; a \
                     population must be the only demand in the document"
                ),
            ));
        }
    }
}

/// Tolerance, in metres, within which two connector ends count as coincident.
const CONNECTOR_CONTINUITY_TOLERANCE_M: f64 = 1e-6;

/// Tolerance, in metres, within which two facility bands count as touching
/// along a shared boundary, and within which a contact point counts as off a
/// reference centerline.
const ADJACENCY_TOLERANCE_M: f64 = 1e-6;

/// Tolerance, in `1/m`, within which a reference curvature counts as within a
/// mode's turning limit.
const TURNING_CURVATURE_TOLERANCE: f64 = 1e-9;

/// Largest lateral envelope width of a version-2 mode template's body, in
/// metres: `2 * radius` for a circle or capsule, `width` for a box.
fn body_envelope_width_m(body: &ModeBodySource) -> f64 {
    match body {
        ModeBodySource::Box { width_m, .. } => width_m.max,
        ModeBodySource::Circle { radius_m } => 2.0 * radius_m.max,
        ModeBodySource::Capsule { radius_m, .. } => 2.0 * radius_m.max,
        ModeBodySource::ArticulatedChain { segments, .. } => segments
            .iter()
            .map(|segment| segment.width_m.max)
            .fold(0.0, f64::max),
    }
}

/// A mode template's preferred lateral clearance, in metres; zero when it
/// declares none.
fn mode_lateral_clearance_m(template: &ModeTemplateSource) -> f64 {
    template
        .profiles
        .get("lateral_clearance_m")
        .map(|range| range.max)
        .filter(|clearance| clearance.is_finite() && *clearance >= 0.0)
        .unwrap_or(0.0)
}

/// The tightest steady-turn curvature a wheeled mode can follow, or `None`
/// when the mode declares no usable steering limit.
///
/// Two independent bounds are intersected. The **rate** bound is the tightest
/// steady curve the mode can hold at its top desired speed from its authored
/// maximum heading rate (`steering_rate_max_rad_s / speed_mps`), the Increment 1
/// rule. The **axle-geometry** bound is the tightest curve its wheelbase and
/// maximum steering angle allow under the single-track relation
/// `kappa = tan(steering_angle) / wheelbase`; it is the Increment 3 addition
/// that gives an authored wheelbase real effect, because a heading-rate limit
/// alone is independent of axle geometry. Across a sampled range the
/// conservative (least capable) end is the longest wheelbase and the smallest
/// steering angle, so a caller never admits a route some sampled agent could
/// not follow.
fn mode_turning_limit_curvature(template: &ModeTemplateSource) -> Option<f64> {
    let rate_bound = template
        .profiles
        .get("steering_rate_max_rad_s")
        .zip(template.profiles.get("speed_mps"))
        .and_then(|(steering, speed)| {
            let (steering, speed) = (steering.max, speed.max);
            (steering.is_finite() && steering > 0.0 && speed.is_finite() && speed > 0.0)
                .then(|| steering / speed)
        });
    let geometry_bound = template
        .wheelbase_m
        .zip(template.steering_angle_max_rad)
        .and_then(|(wheelbase, angle)| {
            let (wheelbase, angle) = (wheelbase.max, angle.min);
            (wheelbase.is_finite()
                && wheelbase > 0.0
                && angle.is_finite()
                && angle > 0.0
                && angle < std::f64::consts::FRAC_PI_2)
                .then(|| angle.tan() / wheelbase)
        });
    match (rate_bound, geometry_bound) {
        (Some(rate), Some(geometry)) => Some(rate.min(geometry)),
        (bound, None) | (None, bound) => bound,
    }
}

/// The largest absolute curvature anywhere on a compiled reference path.
///
/// Curvature is piecewise constant between vertices, so sampling densely enough
/// to visit every compiled segment recovers the maximum on both the straight
/// polylines authored facilities use and the analytic arcs the compiled
/// geometry exposes.
fn reference_max_abs_curvature(reference: &CompiledReferencePath) -> f64 {
    let length = reference.length();
    if length <= 0.0 {
        return 0.0;
    }
    const SAMPLE_INTERVAL_M: f64 = 0.5;
    let samples = (length / SAMPLE_INTERVAL_M).ceil().max(1.0) as usize;
    let mut max = 0.0_f64;
    for index in 0..=samples {
        let s = length * index as f64 / samples as f64;
        max = max.max(reference.curvature_at(s).abs());
    }
    max
}

/// The largest discrete corner curvature, in `1/m`, of an authored polyline.
///
/// An authored reference path is a polyline, so its compiled segments are
/// straight and [`reference_max_abs_curvature`] reads zero along them even when
/// the vertices trace a curve. The direction change at each interior vertex
/// over the mean adjacent segment length is the discrete curvature of the curve
/// the polyline approximates: for points sampled on a circle it recovers
/// `1 / radius` to within the chord discretization error, so a chord
/// approximation of a constant-radius turn is measured at its true curvature.
/// A straight or degenerate polyline is zero.
fn authored_max_corner_curvature(points: &[PointSource]) -> f64 {
    let mut max = 0.0_f64;
    for window in points.windows(3) {
        let [a, b, c] = window else { continue };
        let ab = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let bc = ((c.x - b.x).powi(2) + (c.y - b.y).powi(2)).sqrt();
        let mean = 0.5 * (ab + bc);
        if !mean.is_finite() || mean <= 0.0 {
            continue;
        }
        let incoming = (b.y - a.y).atan2(b.x - a.x);
        let outgoing = (c.y - b.y).atan2(c.x - b.x);
        let mut turn = (outgoing - incoming) % std::f64::consts::TAU;
        if turn > std::f64::consts::PI {
            turn -= std::f64::consts::TAU;
        } else if turn < -std::f64::consts::PI {
            turn += std::f64::consts::TAU;
        }
        max = max.max(turn.abs() / mean);
    }
    max
}

/// The curvature diagnostics for one facility reference against the turning
/// limits of the modes permitted on it.
///
/// A mode that authors axle geometry (`wheelbase_m`) is additionally held to
/// the authored polyline's discrete corner curvature, because such a mode's
/// turning limit is a genuine geometric radius and an authored chord
/// approximation of a curve otherwise reads as curvature-free. A mode without
/// axle geometry keeps the Increment 1 rule and sees only the compiled
/// segment/arc curvature, so no Increment 1/2 fixture changes behaviour.
fn facility_curvature_diagnostics(
    facility_id: &str,
    reference: &CompiledReferencePath,
    authored_corner_curvature: f64,
    modes: &[&ModeTemplateSource],
) -> Vec<Diagnostic> {
    let segment_curvature = reference_max_abs_curvature(reference);
    for template in modes {
        let curvature = if template.wheelbase_m.is_some() {
            segment_curvature.max(authored_corner_curvature)
        } else {
            segment_curvature
        };
        if let Some(limit) = mode_turning_limit_curvature(template)
            && curvature > limit + TURNING_CURVATURE_TOLERANCE
        {
            return vec![Diagnostic::new(
                DiagnosticCode::FacilityCurvature,
                Some(facility_id.to_owned()),
                format!(
                    "facility '{facility_id}' reference curvature {curvature} exceeds the \
                     turning limit {limit} of mode '{}'",
                    template.id
                ),
            )];
        }
    }
    Vec::new()
}

/// Ray-casting point-in-polygon test over an implicitly closed ring.
fn point_in_polygon(point: PointSource, ring: &[PointSource]) -> bool {
    let count = ring.len();
    if count < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = count - 1;
    for current in 0..count {
        let a = ring[current];
        let b = ring[previous];
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < x {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

/// Whether every vertex of a facility region lies inside the traversable world.
///
/// The world limit is the union of the declared `boundaries` polygons; with no
/// boundary authored there is no world limit to contain the region.
fn polygon_inside_world(region: &[PointSource], boundaries: &[PolygonSource]) -> bool {
    region.iter().all(|vertex| {
        boundaries
            .iter()
            .any(|boundary| point_in_polygon(*vertex, &boundary.points))
    })
}

/// Compile one facility's authored reference path into its `(s, d)` geometry,
/// or `None` when the facility declares no path or the path is undeclared.
fn facility_reference(
    source: &ScenarioSourceV2,
    facility: &FacilitySource,
) -> Option<CompiledReferencePath> {
    let name = facility.reference_path.as_ref()?;
    let path = source.paths.iter().find(|path| path.id == *name)?;
    let points: Vec<DVec2> = path
        .points
        .iter()
        .map(|point| DVec2::new(point.x, point.y))
        .collect();
    Some(CompiledReferencePath::from_polyline(&points))
}

/// The world point at which a connector meets a reference-path traversal.
///
/// A forward traversal leaves at the reference path's end and enters at its
/// start; a reverse traversal leaves at its start and enters at its end.
fn traversal_end(
    reference: &CompiledReferencePath,
    direction: MovementDirection,
    leaving: bool,
) -> DVec2 {
    let at_end = leaving == (direction == MovementDirection::Forward);
    if at_end {
        reference.position_at(reference.length())
    } else {
        reference.position_at(0.0)
    }
}

/// Push `direction` unless it is already present.
fn push_direction(directions: &mut Vec<MovementDirection>, direction: MovementDirection) {
    if !directions.contains(&direction) {
        directions.push(direction);
    }
}

/// Validate a version-2 facility's references, dimensions, containment, usable
/// width, curvature, and mode access.
fn validate_facilities(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    for facility in &source.facilities {
        let object = Some(facility.id.clone());

        // Reference integrity first: `CompiledScenario::compile_v2` indexes
        // these references after validation, so an undeclared one must be
        // rejected here rather than panicking during compilation.
        let region = source
            .regions
            .iter()
            .find(|region| region.id == facility.region);
        if region.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityUnknownRegion,
                object.clone(),
                format!(
                    "facility '{}' occupies undeclared region '{}'",
                    facility.id, facility.region
                ),
            ));
        }

        let reference_path = facility
            .reference_path
            .as_ref()
            .and_then(|name| source.paths.iter().find(|path| path.id == *name));
        if let Some(name) = &facility.reference_path
            && reference_path.is_none()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityUnknownPath,
                object.clone(),
                format!(
                    "facility '{}' references undeclared path '{name}'",
                    facility.id
                ),
            ));
        }

        if facility.access.modes.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAccessEmpty,
                object.clone(),
                format!("facility '{}' permits no mode", facility.id),
            ));
        }

        if matches!(
            facility.nominal_direction,
            FacilityDirection::Forward | FacilityDirection::Reverse
        ) && facility.reference_path.is_none()
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityDirectionWithoutPath,
                object.clone(),
                format!(
                    "facility '{}' declares a directional nominal_direction without a \
                     reference_path; only 'either' may omit one",
                    facility.id
                ),
            ));
        }

        if !facility.width_m.is_finite() || facility.width_m <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityWidthInvalid,
                object.clone(),
                format!(
                    "facility '{}' width_m must be finite and positive, got {}",
                    facility.id, facility.width_m
                ),
            ));
        }

        if let Some(limit) = facility.speed_policy.limit_mps.value()
            && (!limit.is_finite() || limit <= 0.0)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilitySpeedLimitInvalid,
                object.clone(),
                format!(
                    "facility '{}' speed_policy.limit_mps must be finite and positive, got \
                     {limit}",
                    facility.id
                ),
            ));
        }

        // Resolve the permitted modes once: an undeclared one is reported here,
        // and every later geometric rule uses only the resolved templates.
        let mut modes: Vec<&ModeTemplateSource> = Vec::new();
        for mode in &facility.access.modes {
            match source
                .mode_templates
                .iter()
                .find(|template| template.id == *mode)
            {
                Some(template) => modes.push(template),
                None => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::FacilityUnknownMode,
                    object.clone(),
                    format!(
                        "facility '{}' permits undeclared mode template '{mode}'",
                        facility.id
                    ),
                )),
            }
        }

        // Mode-to-facility access: a permitted mode's template must serve the
        // continuous facility kind.
        for template in &modes {
            if !template
                .access
                .facility_kinds
                .contains(&FacilityKind::Facility)
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::FacilityAccessDenied,
                    object.clone(),
                    format!(
                        "facility '{}' permits mode '{}', which does not list the 'facility' \
                         access kind",
                        facility.id, template.id
                    ),
                ));
            }
        }

        // Containment: the facility region lies inside the traversable world.
        if !source.boundaries.is_empty()
            && let Some(region) = region
            && !polygon_inside_world(&region.points, &source.boundaries)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityOutsideWorld,
                object.clone(),
                format!(
                    "facility '{}' region '{}' lies outside the traversable world",
                    facility.id, facility.region
                ),
            ));
        }

        // Usable width, which is also the spawn-clearance rule: with a constant
        // authored width the usable lateral interval is the same at every arc
        // length, so a spawn point on the facility has room exactly when every
        // permitted mode's largest body plus its lateral clearance fits.
        if facility.width_m.is_finite() && facility.width_m > 0.0 {
            for template in &modes {
                let envelope = body_envelope_width_m(&template.body);
                let clearance = mode_lateral_clearance_m(template);
                if envelope + 2.0 * clearance > facility.width_m {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::FacilityTooNarrow,
                        object.clone(),
                        format!(
                            "facility '{}' width_m {} cannot fit mode '{}' (body {envelope} m \
                             plus 2 x clearance {clearance} m)",
                            facility.id, facility.width_m, template.id
                        ),
                    ));
                    break;
                }
            }
        }

        // The Increment 2 lateral policy names a side a body displaces toward,
        // so it needs a reference frame to name the side on, must not sit on a
        // centered facility that offers no lateral target, and must name a side
        // an eligible body can actually occupy.
        if facility.lateral_policy.is_some() {
            if facility.reference_path.is_none() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::FacilityLateralWithoutReference,
                    object.clone(),
                    format!(
                        "facility '{}' declares a lateral_policy without a reference_path; a \
                         facility with no (s, d) frame has no side to name",
                        facility.id
                    ),
                ));
            }
            if matches!(facility.lateral_use, LateralUse::Centered) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::FacilityLateralCentered,
                    object.clone(),
                    format!(
                        "facility '{}' is centered but declares a lateral_policy; a centered \
                         facility offers no lateral target for the side to apply to",
                        facility.id
                    ),
                ));
            }
            if facility.width_m.is_finite() && facility.width_m > 0.0 {
                let side_usable = modes.iter().any(|template| {
                    let half = facility.width_m * 0.5
                        - body_envelope_width_m(&template.body) * 0.5
                        - mode_lateral_clearance_m(template);
                    half > 0.0
                });
                if !side_usable {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::FacilityPassingSideUnusable,
                        object.clone(),
                        format!(
                            "facility '{}' lateral_policy names a side no permitted body can \
                             occupy: every eligible body leaves no nonzero usable interval",
                            facility.id
                        ),
                    ));
                }
            }
        }

        // Curvature against the turning limits of every permitted mode.
        if let Some(reference) = facility_reference(source, facility) {
            let corner_curvature = facility
                .reference_path
                .as_ref()
                .and_then(|name| source.paths.iter().find(|path| path.id == *name))
                .map(|path| authored_max_corner_curvature(&path.points))
                .unwrap_or(0.0);
            diagnostics.extend(facility_curvature_diagnostics(
                &facility.id,
                &reference,
                corner_curvature,
                &modes,
            ));
        }
    }
}

/// Validate a version-2 facility connector's endpoints and continuity.
fn validate_facility_connectors(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    for connector in &source.facility_connectors {
        let object = Some(connector.id.clone());
        let from = source
            .facilities
            .iter()
            .find(|facility| facility.id == connector.from.facility);
        let to = source
            .facilities
            .iter()
            .find(|facility| facility.id == connector.to.facility);

        if from.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityConnectorUnknownFacility,
                object.clone(),
                format!(
                    "connector '{}' leaves undeclared facility '{}'",
                    connector.id, connector.from.facility
                ),
            ));
        }
        if to.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityConnectorUnknownFacility,
                object.clone(),
                format!(
                    "connector '{}' enters undeclared facility '{}'",
                    connector.id, connector.to.facility
                ),
            ));
        }

        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };

        let from_reference = facility_reference(source, from);
        let to_reference = facility_reference(source, to);
        if from_reference.is_none() || to_reference.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityConnectorWithoutReference,
                object.clone(),
                format!(
                    "connector '{}' attaches a facility without a compiled reference path; a \
                     connector joins two reference traversals",
                    connector.id
                ),
            ));
            continue;
        }
        let (Some(from_reference), Some(to_reference)) = (from_reference, to_reference) else {
            continue;
        };

        let leaving = traversal_end(&from_reference, connector.from.direction, true);
        let entering = traversal_end(&to_reference, connector.to.direction, false);
        let gap = leaving.distance(entering);
        if gap > CONNECTOR_CONTINUITY_TOLERANCE_M {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityConnectorDiscontinuous,
                object.clone(),
                format!(
                    "connector '{}' leaving end and entering end do not coincide (gap {gap} m)",
                    connector.id
                ),
            ));
        }
    }
}

/// The midpoint of the longest collinear boundary segment two polygon rings
/// share within [`ADJACENCY_TOLERANCE_M`], or `None` when they share none.
///
/// A shared boundary is what "the bands touch along a stretch of positive
/// length" means: two rings touching at a single corner share no segment and
/// return `None`. Validation and compilation call this one implementation: the
/// compiler reuses it to derive the compiled shared boundary of a side-by-side
/// adjacency, so the two never disagree on what touching means.
pub(crate) fn shared_boundary_midpoint(a: &[DVec2], b: &[DVec2]) -> Option<DVec2> {
    if a.len() < 3 || b.len() < 3 {
        return None;
    }
    let mut best: Option<(f64, DVec2)> = None;
    for index in 0..a.len() {
        let a0 = a[index];
        let a1 = a[(index + 1) % a.len()];
        let dx = a1.x - a0.x;
        let dy = a1.y - a0.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= ADJACENCY_TOLERANCE_M {
            continue;
        }
        for other in 0..b.len() {
            let b0 = b[other];
            let b1 = b[(other + 1) % b.len()];
            let off0 = ((b0.x - a0.x) * dy - (b0.y - a0.y) * dx).abs() / length;
            let off1 = ((b1.x - a0.x) * dy - (b1.y - a0.y) * dx).abs() / length;
            if off0 > ADJACENCY_TOLERANCE_M || off1 > ADJACENCY_TOLERANCE_M {
                continue;
            }
            let length_sq = length * length;
            let t0 = ((b0.x - a0.x) * dx + (b0.y - a0.y) * dy) / length_sq;
            let t1 = ((b1.x - a0.x) * dx + (b1.y - a0.y) * dy) / length_sq;
            let low = t0.min(t1).clamp(0.0, 1.0);
            let high = t0.max(t1).clamp(0.0, 1.0);
            let overlap = (high - low) * length;
            if overlap <= ADJACENCY_TOLERANCE_M {
                continue;
            }
            if best.as_ref().is_none_or(|(longest, _)| overlap > *longest) {
                let mid = 0.5 * (low + high);
                best = Some((overlap, a0 + (a1 - a0) * mid));
            }
        }
    }
    best.map(|(_, midpoint)| midpoint)
}

/// The side of `reference`'s forward direction on which a world point lies, or
/// `None` when the point sits on the centerline within
/// [`ADJACENCY_TOLERANCE_M`].
fn reference_side_of(reference: &CompiledReferencePath, point: DVec2) -> Option<AdjacencySide> {
    let s = reference.project(point).s();
    let signed = (point - reference.position_at(s)).dot(reference.normal_at(s));
    if signed > ADJACENCY_TOLERANCE_M {
        Some(AdjacencySide::Left)
    } else if signed < -ADJACENCY_TOLERANCE_M {
        Some(AdjacencySide::Right)
    } else {
        None
    }
}

/// Validate version-2 facility adjacencies: two declared, distinct facilities
/// that each declare a reference path, whose bands touch along a shared
/// boundary of positive length on the declared side.
///
/// This is what rejects a "transition" between two facilities that are not
/// actually side by side; proximity of polygons is never inferred into a
/// transition, and an omitted adjacency is how a document expresses "no lateral
/// transitions exist".
fn validate_facility_adjacencies(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    for adjacency in &source.facility_adjacencies {
        let object = Some(adjacency.id.clone());
        let first = source
            .facilities
            .iter()
            .find(|facility| facility.id == adjacency.first);
        let second = source
            .facilities
            .iter()
            .find(|facility| facility.id == adjacency.second);
        if first.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAdjacencyUnknownFacility,
                object.clone(),
                format!(
                    "adjacency '{}' names undeclared facility '{}'",
                    adjacency.id, adjacency.first
                ),
            ));
        }
        if second.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAdjacencyUnknownFacility,
                object.clone(),
                format!(
                    "adjacency '{}' names undeclared facility '{}'",
                    adjacency.id, adjacency.second
                ),
            ));
        }
        if adjacency.first == adjacency.second {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAdjacencySelf,
                object.clone(),
                format!(
                    "adjacency '{}' joins facility '{}' to itself",
                    adjacency.id, adjacency.first
                ),
            ));
        }
        let (Some(first), Some(second)) = (first, second) else {
            continue;
        };
        if first.reference_path.is_none() || second.reference_path.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAdjacencyWithoutReference,
                object.clone(),
                format!(
                    "adjacency '{}' attaches a facility without a reference_path; a lateral \
                     transition joins two reference bands",
                    adjacency.id
                ),
            ));
            continue;
        }

        // The bands are the two facility regions. An undeclared or malformed
        // region is already reported by the facility and polygon rules, so the
        // geometry rule stays out of their way.
        let first_region = source
            .regions
            .iter()
            .find(|region| region.id == first.region);
        let second_region = source
            .regions
            .iter()
            .find(|region| region.id == second.region);
        let (Some(first_region), Some(second_region)) = (first_region, second_region) else {
            continue;
        };
        let first_ring: Vec<DVec2> = first_region
            .points
            .iter()
            .map(|point| DVec2::new(point.x, point.y))
            .collect();
        let second_ring: Vec<DVec2> = second_region
            .points
            .iter()
            .map(|point| DVec2::new(point.x, point.y))
            .collect();
        let Some(midpoint) = shared_boundary_midpoint(&first_ring, &second_ring) else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAdjacencyDisjoint,
                object.clone(),
                format!(
                    "adjacency '{}' joins facilities '{}' and '{}', whose bands do not touch \
                     along a shared boundary of positive length",
                    adjacency.id, first.id, second.id
                ),
            ));
            continue;
        };
        let Some(reference) = facility_reference(source, first) else {
            continue;
        };
        if reference_side_of(&reference, midpoint) != Some(adjacency.side) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityAdjacencySide,
                object.clone(),
                format!(
                    "adjacency '{}' declares side {:?} of facility '{}', but the bands touch on \
                     the other side",
                    adjacency.id, adjacency.side, first.id
                ),
            ));
        }
    }
}

/// Validate that each facility's declared nominal direction is physically
/// possible through the connector graph.
///
/// Physical possibility is decided only by the connector graph and stays
/// separate from the authored nominal direction and any permitted direction.
fn validate_facility_reachability(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    let mut possible: Vec<Vec<MovementDirection>> = vec![Vec::new(); source.facilities.len()];
    let mut attached = vec![false; source.facilities.len()];
    for connector in &source.facility_connectors {
        if let Some(index) = source
            .facilities
            .iter()
            .position(|facility| facility.id == connector.from.facility)
        {
            attached[index] = true;
            push_direction(&mut possible[index], connector.from.direction);
        }
        if let Some(index) = source
            .facilities
            .iter()
            .position(|facility| facility.id == connector.to.facility)
        {
            attached[index] = true;
            push_direction(&mut possible[index], connector.to.direction);
        }
    }

    for (index, facility) in source.facilities.iter().enumerate() {
        let nominal = match facility.nominal_direction {
            FacilityDirection::Forward => Some(MovementDirection::Forward),
            FacilityDirection::Reverse => Some(MovementDirection::Reverse),
            FacilityDirection::Either => None,
        };
        if let Some(nominal) = nominal
            && attached[index]
            && !possible[index].contains(&nominal)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::FacilityUnreachableDirection,
                Some(facility.id.clone()),
                format!(
                    "facility '{}' declares nominal direction {:?}, which the connector graph \
                     does not make physically possible",
                    facility.id, nominal
                ),
            ));
        }
    }
}

/// The declared object kind a permission statement's `target` names, resolved
/// without regard to the statement's `kind`.
enum PermissionObject<'a> {
    /// A continuous-width facility; the statement resolves against it.
    Facility(&'a FacilitySource),
    /// A movement connector.
    Movement,
    /// A pedestrian crossing.
    Crossing,
}

/// Resolve a permission target id to the declared object it names, or `None`
/// when no declared object carries that id.
fn resolve_permission_object<'a>(
    source: &'a ScenarioSourceV2,
    target: &str,
) -> Option<PermissionObject<'a>> {
    if let Some(facility) = source
        .facilities
        .iter()
        .find(|facility| facility.id == target)
    {
        return Some(PermissionObject::Facility(facility));
    }
    if source
        .movements
        .iter()
        .any(|movement| movement.id == target)
    {
        return Some(PermissionObject::Movement);
    }
    if source
        .crossings
        .iter()
        .any(|crossing| crossing.id == target)
    {
        return Some(PermissionObject::Crossing);
    }
    None
}

/// Validate version-2 permission statements: a declared holder, a target whose
/// object kind its `kind` fixes, at most one statement per specificity, an
/// `overtake` holder that can overtake, a `lane_use` obligation with a fixed
/// passing side, a nominal direction with an opposite to name, and the
/// legal-versus-physically-possible separation.
///
/// A `prohibit` that contradicts a facility's granted access is an illegal
/// route and is reported with [`DiagnosticCode::PermissionRouteProhibited`],
/// distinct from the codes a physically impossible route carries
/// ([`DiagnosticCode::FacilityUnreachableDirection`],
/// [`DiagnosticCode::FacilityCurvature`], [`DiagnosticCode::FacilityTooNarrow`]).
///
/// A `nominal_direction` statement whose target has no physically connected
/// opposing traversal is *not* rejected: the permitted set never leaves the
/// physically possible set, so the statement is inert for that traversal and
/// the runtime wrong-way decision closes the case with `no_opposing_path`.
fn validate_permissions(source: &ScenarioSourceV2, diagnostics: &mut Vec<Diagnostic>) {
    // Specificity has exactly one axis, `(kind, holder, target)`. Two
    // statements that agree on it — whatever their effects, identical or
    // contradictory — are rejected rather than resolved by a silent override.
    let mut specificity: Vec<(PermissionKind, &str, &str)> = Vec::new();
    for permission in &source.permissions {
        let key = (
            permission.kind,
            permission.holder.as_str(),
            permission.target.as_str(),
        );
        if specificity.contains(&key) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PermissionEffectConflict,
                Some(permission.id.clone()),
                format!(
                    "permission '{}' repeats the (kind, holder, target) of another statement; a \
                     second statement at equal specificity is rejected, not overridden",
                    permission.id
                ),
            ));
        } else {
            specificity.push(key);
        }
    }

    for permission in &source.permissions {
        let object = Some(permission.id.clone());
        let holder = source
            .mode_templates
            .iter()
            .find(|template| template.id == permission.holder);
        if holder.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PermissionUnknownHolder,
                object.clone(),
                format!(
                    "permission '{}' binds undeclared mode template '{}'",
                    permission.id, permission.holder
                ),
            ));
        }

        // `stop_service` targets a `bus_stop` Increment 4 owns, so it stays
        // shape-only: its target is never resolved and it binds no traversal.
        if permission.kind == PermissionKind::StopService {
            continue;
        }

        let Some(resolved) = resolve_permission_object(source, &permission.target) else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PermissionUnknownTarget,
                object.clone(),
                format!(
                    "permission '{}' names undeclared target '{}'",
                    permission.id, permission.target
                ),
            ));
            continue;
        };

        // `kind` fixes which object kind the target names.
        let expected = match permission.kind {
            PermissionKind::NominalDirection => matches!(
                &resolved,
                PermissionObject::Facility(_) | PermissionObject::Movement
            ),
            PermissionKind::LaneUse | PermissionKind::Overtake => {
                matches!(&resolved, PermissionObject::Facility(_))
            }
            PermissionKind::Crossing => matches!(&resolved, PermissionObject::Crossing),
            PermissionKind::StopService => false,
        };
        if !expected {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PermissionTargetKind,
                object.clone(),
                format!(
                    "permission '{}' is a {:?} statement but target '{}' names a different \
                     object kind",
                    permission.id, permission.kind, permission.target
                ),
            ));
            continue;
        }

        let PermissionObject::Facility(facility) = resolved else {
            // A movement- or crossing-targeted statement never carries the
            // facility rules below.
            continue;
        };

        if permission.kind == PermissionKind::NominalDirection {
            // A nominal-direction statement needs an opposite direction to
            // permit, prohibit, or oblige; an `either` object has none.
            if facility.nominal_direction == FacilityDirection::Either {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PermissionNominalEither,
                    object.clone(),
                    format!(
                        "permission '{}' is a nominal_direction statement about 'either' \
                         facility '{}', which has no opposite direction",
                        permission.id, facility.id
                    ),
                ));
            }
            if permission.effect == PermissionEffect::Prohibit
                && let Some(holder) = holder
                && facility.access.modes.iter().any(|mode| mode == &holder.id)
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PermissionRouteProhibited,
                    object.clone(),
                    format!(
                        "permission '{}' prohibits mode '{}' on facility '{}', which grants it \
                         access; the route is illegal",
                        permission.id, holder.id, facility.id
                    ),
                ));
            }
        }

        if permission.kind == PermissionKind::LaneUse
            && permission.effect == PermissionEffect::Obligate
        {
            let fixed_side = facility
                .lateral_policy
                .is_some_and(|policy| policy.passing_side != PassingSide::MostClearance);
            if !fixed_side {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PermissionLaneUseObligation,
                    object.clone(),
                    format!(
                        "permission '{}' obligates lane use on facility '{}', which names no \
                         fixed passing side (left or right)",
                        permission.id, facility.id
                    ),
                ));
            }
        }

        if permission.kind == PermissionKind::Overtake
            && let Some(holder) = holder
            && !holder.tactics.contains(&TacticKind::Overtake)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::PermissionOvertakeCapability,
                object.clone(),
                format!(
                    "permission '{}' grants overtaking to mode '{}', which does not declare the \
                     overtake tactic",
                    permission.id, holder.id
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::parse_scenario_source;

    fn base(body: &str) -> ScenarioSource {
        let text = format!(
            "{{ schema_version: 1, id: 'case', coordinate_system: {{ x: 'east_m', y: 'north_m' }}, {body} }}"
        );
        parse_scenario_source(&text).expect("test scenario parses")
    }

    fn codes(source: &ScenarioSource) -> Vec<&'static str> {
        validate(source).iter().map(|d| d.code.as_str()).collect()
    }

    const OK_PATHS: &str =
        "paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] } ]";
    const OK_PORTALS: &str = "portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ]";

    #[test]
    fn accepts_a_well_formed_scenario() {
        let source = base(&format!("{OK_PATHS}, {OK_PORTALS}"));
        assert_eq!(validate(&source), Vec::new());
    }

    #[test]
    fn flags_unsupported_schema_version() {
        let source = base(&format!("{OK_PATHS}, {OK_PORTALS}"));
        let mut source = source;
        source.schema_version = 3;
        assert_eq!(codes(&source), ["E_SCHEMA_VERSION"]);
    }

    #[test]
    fn flags_duplicate_ids_across_object_kinds() {
        let source = base(
            "paths: [ { id: 'x', points: [ { x: 0, y: 0 }, { x: 1, y: 0 } ] } ], \
             portals: [ { id: 'x', path: 'x', end: 'start', width_m: 3.0 } ]",
        );
        assert!(codes(&source).contains(&"E_ID_DUPLICATE"));
    }

    #[test]
    fn flags_non_finite_coordinates() {
        let source = base(
            "paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 1, y: 0 }, { x: 1, y: 0 } ] } ], \
             portals: []",
        );
        // JSON5 has no NaN literal, so craft the struct directly.
        let mut source = source;
        source.paths[0].points[1].x = f64::INFINITY;
        assert!(codes(&source).contains(&"E_NON_FINITE"));
    }

    #[test]
    fn flags_non_positive_dimensions() {
        let source = base(&format!(
            "{OK_PATHS}, portals: [ {{ id: 'a', path: 'guide', end: 'start', width_m: 0.0 }}, \
             {{ id: 'b', path: 'guide', end: 'end', width_m: 3.0 }} ]"
        ));
        assert!(codes(&source).contains(&"E_NON_POSITIVE"));
    }

    #[test]
    fn flags_empty_and_degenerate_paths() {
        let empty = base("paths: [ { id: 'guide', points: [] } ], portals: []");
        assert!(codes(&empty).contains(&"E_PATH_EMPTY"));

        let degenerate = base(
            "paths: [ { id: 'guide', points: [ { x: 1, y: 1 }, { x: 1, y: 1 } ] } ], portals: []",
        );
        assert!(codes(&degenerate).contains(&"E_PATH_DEGENERATE"));
    }

    #[test]
    fn flags_unknown_and_unreachable_portals() {
        let unknown = base(
            "paths: [], portals: [ { id: 'a', path: 'missing', end: 'start', width_m: 3.0 } ]",
        );
        assert!(codes(&unknown).contains(&"E_PORTAL_UNKNOWN_PATH"));

        let unreachable = base(
            "paths: [ { id: 'guide', points: [ { x: 1, y: 1 }, { x: 1, y: 1 } ] } ], \
             portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 } ]",
        );
        assert!(codes(&unreachable).contains(&"E_PORTAL_UNREACHABLE"));
    }

    #[test]
    fn flags_two_portals_on_the_same_end() {
        let source = base(&format!(
            "{OK_PATHS}, portals: [ {{ id: 'a', path: 'guide', end: 'start', width_m: 3.0 }}, \
             {{ id: 'b', path: 'guide', end: 'start', width_m: 3.0 }} ]"
        ));
        assert!(codes(&source).contains(&"E_PORTAL_DUPLICATE_END"));
    }

    #[test]
    fn flags_bad_population_values() {
        let source = base(&format!(
            "{OK_PATHS}, {OK_PORTALS}, population: {{ vehicle_count: 1, vehicle_speed_mps: -1.0, \
             vehicle_spacing_m: 1.0, vehicle_length_m: 1.0, vehicle_width_m: 1.0 }}"
        ));
        assert!(codes(&source).contains(&"E_NON_POSITIVE"));
    }

    #[test]
    fn diagnostics_are_displayable_with_code_and_object() {
        let source = base(&format!(
            "{OK_PATHS}, portals: [ {{ id: 'a', path: 'nope', end: 'start', width_m: 3.0 }} ]"
        ));
        let rendered: Vec<String> = validate(&source).iter().map(ToString::to_string).collect();
        assert!(
            rendered
                .iter()
                .any(|line| line.starts_with("[E_PORTAL_UNKNOWN_PATH] a:"))
        );
    }

    /// A complete car/pedestrian four-way layout: two crossing paths, two
    /// movements, a conflict region, a crossing, signal rules, and a two-phase
    /// fixed-time controller. It uses only general primitives, so it is the
    /// fixture the gate means by "no intersection type".
    const SIGNALIZED: &str = "
        paths: [
            { id: 'ew', points: [ { x: -20, y: 0 }, { x: 20, y: 0 } ] },
            { id: 'ns', points: [ { x: 0, y: -20 }, { x: 0, y: 20 } ] },
        ],
        portals: [
            { id: 'west', path: 'ew', end: 'start', width_m: 3.5 },
            { id: 'east', path: 'ew', end: 'end', width_m: 3.5 },
            { id: 'south', path: 'ns', end: 'start', width_m: 3.5 },
            { id: 'north', path: 'ns', end: 'end', width_m: 3.5 },
        ],
        boundaries: [ { id: 'world', points: [
            { x: -30, y: -30 }, { x: 30, y: -30 }, { x: 30, y: 30 }, { x: -30, y: 30 }
        ] } ],
        regions: [ { id: 'crossing_area', points: [
            { x: -3, y: -3 }, { x: 3, y: -3 }, { x: 3, y: 3 }, { x: -3, y: 3 }
        ] } ],
        movements: [
            { id: 'ew_through', from: 'west', to: 'east', path: 'ew', priority: 0 },
            { id: 'ns_through', from: 'south', to: 'north', path: 'ns', priority: 0 },
        ],
        crossings: [ { id: 'north_crossing', region: 'crossing_area',
            movements: [ 'ew_through', 'ns_through' ] } ],
        conflict_regions: [ { id: 'center', points: [
            { x: -1, y: -1 }, { x: 1, y: -1 }, { x: 1, y: 1 }, { x: -1, y: 1 }
        ], movements: [ 'ew_through', 'ns_through' ] } ],
        rules: [
            { id: 'r_ew', movement: 'ew_through', kind: 'signal', signal: 'main' },
            { id: 'r_ns', movement: 'ns_through', kind: 'signal', signal: 'main' },
        ],
        signals: [ { id: 'main',
            heads: [ { id: 'ew', movement: 'ew_through' }, { id: 'ns', movement: 'ns_through' } ],
            phases: [
                { duration_s: 20.0, states: [ { head: 'ew', color: 'green' }, { head: 'ns', color: 'red' } ] },
                { duration_s: 4.0, states: [ { head: 'ew', color: 'yellow' }, { head: 'ns', color: 'red' } ] },
                { duration_s: 20.0, states: [ { head: 'ew', color: 'red' }, { head: 'ns', color: 'green' } ] },
                { duration_s: 4.0, states: [ { head: 'ew', color: 'red' }, { head: 'ns', color: 'yellow' } ] },
            ]
        } ],
    ";

    #[test]
    fn accepts_a_general_signalized_layout() {
        let source = base(SIGNALIZED);
        assert_eq!(validate(&source), Vec::new());
    }

    #[test]
    fn flags_polygon_vertex_and_area_errors() {
        let too_few = base(&format!(
            "{OK_PATHS}, {OK_PORTALS}, boundaries: [ {{ id: 'b', points: [ {{ x: 0, y: 0 }}, {{ x: 1, y: 0 }} ] }} ]"
        ));
        assert!(codes(&too_few).contains(&"E_POLYGON_TOO_FEW_VERTICES"));

        let degenerate = base(&format!(
            "{OK_PATHS}, {OK_PORTALS}, regions: [ {{ id: 'r', points: [
                {{ x: 0, y: 0 }}, {{ x: 1, y: 0 }}, {{ x: 2, y: 0 }}
            ] }} ]"
        ));
        assert!(codes(&degenerate).contains(&"E_POLYGON_DEGENERATE"));
    }

    #[test]
    fn flags_movement_reference_errors() {
        let source = base(
            "paths: [ { id: 'ew', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] }, \
             { id: 'ns', points: [ { x: 0, y: 0 }, { x: 0, y: 10 } ] } ], \
             portals: [ { id: 'west', path: 'ew', end: 'start', width_m: 3.0 }, \
             { id: 'north', path: 'ns', end: 'end', width_m: 3.0 } ], \
             movements: [ { id: 'm', from: 'west', to: 'north', path: 'ew', priority: 0 } ]",
        );
        let found = codes(&source);
        assert!(found.contains(&"E_MOVEMENT_PORTAL_PATH_MISMATCH"));

        let unknown = base(
            "paths: [], portals: [], \
             movements: [ { id: 'm', from: 'nope', to: 'nada', path: 'gone', priority: 0 } ]",
        );
        let found = codes(&unknown);
        assert!(found.contains(&"E_MOVEMENT_UNKNOWN_PORTAL"));
        assert!(found.contains(&"E_MOVEMENT_UNKNOWN_PATH"));

        let loop_back = base(
            "paths: [ { id: 'ew', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'ew', end: 'start', width_m: 3.0 } ], \
             movements: [ { id: 'm', from: 'west', to: 'west', path: 'ew', priority: 0 } ]",
        );
        assert!(codes(&loop_back).contains(&"E_MOVEMENT_SELF_LOOP"));
    }

    #[test]
    fn flags_crossing_and_conflict_reference_errors() {
        let crossing = base(&format!(
            "{OK_PATHS}, {OK_PORTALS}, \
             crossings: [ {{ id: 'c', region: 'missing', movements: [ 'ghost' ] }} ]"
        ));
        let found = codes(&crossing);
        assert!(found.contains(&"E_CROSSING_UNKNOWN_REGION"));
        assert!(found.contains(&"E_CROSSING_UNKNOWN_MOVEMENT"));

        let empty = base(&format!(
            "{OK_PATHS}, {OK_PORTALS}, crossings: [ {{ id: 'c', region: 'r', movements: [] }} ]"
        ));
        assert!(codes(&empty).contains(&"E_CROSSING_EMPTY"));

        let conflict = base(&format!(
            "{OK_PATHS}, {OK_PORTALS}, conflict_regions: [ {{ id: 'x', points: [
                {{ x: 0, y: 0 }}, {{ x: 1, y: 0 }}, {{ x: 1, y: 1 }}
            ], movements: [ 'a', 'a' ] }} ]"
        ));
        let found = codes(&conflict);
        assert!(found.contains(&"E_CONFLICT_ARITY"));
        assert!(found.contains(&"E_CONFLICT_UNKNOWN_MOVEMENT"));
    }

    #[test]
    fn flags_conflicting_greens() {
        let green_both = SIGNALIZED.replace(
            "{ duration_s: 20.0, states: [ { head: 'ew', color: 'green' }, { head: 'ns', color: 'red' } ] }",
            "{ duration_s: 20.0, states: [ { head: 'ew', color: 'green' }, { head: 'ns', color: 'green' } ] }",
        );
        let source = base(&green_both);
        assert!(codes(&source).contains(&"E_SIGNAL_CONFLICTING_GREEN"));
    }

    #[test]
    fn flags_signal_phase_head_errors() {
        let missing = SIGNALIZED.replace(
            "{ duration_s: 4.0, states: [ { head: 'ew', color: 'yellow' }, { head: 'ns', color: 'red' } ] }",
            "{ duration_s: 4.0, states: [ { head: 'ew', color: 'yellow' } ] }",
        );
        assert!(codes(&base(&missing)).contains(&"E_SIGNAL_PHASE_MISSING_HEAD"));

        let unknown = SIGNALIZED.replace("head: 'ns', color: 'red'", "head: 'ghost', color: 'red'");
        assert!(codes(&base(&unknown)).contains(&"E_SIGNAL_UNKNOWN_HEAD"));

        let duplicate = SIGNALIZED.replace(
            "{ duration_s: 4.0, states: [ { head: 'ew', color: 'red' }, { head: 'ns', color: 'yellow' } ] }",
            "{ duration_s: 4.0, states: [ { head: 'ew', color: 'red' }, { head: 'ew', color: 'yellow' } ] }",
        );
        assert!(codes(&base(&duplicate)).contains(&"E_SIGNAL_PHASE_DUPLICATE_HEAD"));
    }

    #[test]
    fn flags_rule_signal_mismatch() {
        let no_signal = SIGNALIZED.replace(
            "{ id: 'r_ew', movement: 'ew_through', kind: 'signal', signal: 'main' }",
            "{ id: 'r_ew', movement: 'ew_through', kind: 'signal' }",
        );
        assert!(codes(&base(&no_signal)).contains(&"E_RULE_SIGNAL_MISMATCH"));

        let stray_signal = SIGNALIZED.replace(
            "{ id: 'r_ns', movement: 'ns_through', kind: 'signal', signal: 'main' }",
            "{ id: 'r_ns', movement: 'ns_through', kind: 'yield', signal: 'main' }",
        );
        assert!(codes(&base(&stray_signal)).contains(&"E_RULE_SIGNAL_MISMATCH"));

        let unknown = SIGNALIZED.replace("signal: 'main' }", "signal: 'ghost' }");
        assert!(codes(&base(&unknown)).contains(&"E_RULE_UNKNOWN_SIGNAL"));
    }

    #[test]
    fn flags_overlapping_portals() {
        let source = base(
            "paths: [ { id: 'a', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] }, \
             { id: 'b', points: [ { x: 1, y: 0 }, { x: 20, y: 0 } ] } ], \
             portals: [ { id: 'pa', path: 'a', end: 'start', width_m: 4.0 }, \
             { id: 'pb', path: 'b', end: 'start', width_m: 4.0 } ]",
        );
        assert!(codes(&source).contains(&"E_PORTAL_OVERLAP"));
    }

    const FLOW: &str = "paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] } ], \
         portals: [ { id: 'entry', path: 'guide', end: 'start', width_m: 3.0 }, \
         { id: 'exit', path: 'guide', end: 'end', width_m: 3.0 } ], \
         movements: [ { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 } ]";

    #[test]
    fn accepts_a_portal_demand_source() {
        let source = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'entry', rate_vph: 600.0, \
             routes: [ {{ movement: 'through', weight: 1.0 }} ] }} ]"
        ));
        assert_eq!(validate(&source), Vec::new());
    }

    #[test]
    fn flags_demand_reference_errors() {
        let unknown_portal = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'nope', rate_vph: 600.0, \
             routes: [ {{ movement: 'through', weight: 1.0 }} ] }} ]"
        ));
        assert!(codes(&unknown_portal).contains(&"E_DEMAND_UNKNOWN_PORTAL"));

        let unknown_movement = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'entry', rate_vph: 600.0, \
             routes: [ {{ movement: 'ghost', weight: 1.0 }} ] }} ]"
        ));
        assert!(codes(&unknown_movement).contains(&"E_DEMAND_UNKNOWN_MOVEMENT"));

        let mismatched = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'exit', rate_vph: 600.0, \
             routes: [ {{ movement: 'through', weight: 1.0 }} ] }} ]"
        ));
        assert!(codes(&mismatched).contains(&"E_DEMAND_ROUTE_PORTAL_MISMATCH"));
    }

    #[test]
    fn flags_empty_duplicate_and_non_positive_demand() {
        let empty = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'entry', rate_vph: 600.0, routes: [] }} ]"
        ));
        assert!(codes(&empty).contains(&"E_DEMAND_EMPTY_ROUTES"));

        let duplicate = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'entry', rate_vph: 600.0, \
             routes: [ {{ movement: 'through', weight: 1.0 }}, \
             {{ movement: 'through', weight: 2.0 }} ] }} ]"
        ));
        assert!(codes(&duplicate).contains(&"E_DEMAND_DUPLICATE_ROUTE"));

        let non_positive = base(&format!(
            "{FLOW}, demand: [ {{ id: 'inflow', portal: 'entry', rate_vph: 0.0, \
             routes: [ {{ movement: 'through', weight: 1.0 }} ] }} ]"
        ));
        assert!(codes(&non_positive).contains(&"E_NON_POSITIVE"));
    }

    #[test]
    fn flags_invalid_movement_stop_lines() {
        fn with_stop_line(stop_line_m: f64) -> String {
            format!(
                "paths: [ {{ id: 'guide', points: [ {{ x: 0, y: 0 }}, {{ x: 100, y: 0 }} ] }} ], \
                 portals: [ {{ id: 'entry', path: 'guide', end: 'start', width_m: 3.0 }}, \
                 {{ id: 'exit', path: 'guide', end: 'end', width_m: 3.0 }} ], \
                 movements: [ {{ id: 'through', from: 'entry', to: 'exit', path: 'guide', \
                 priority: 0, stop_line_m: {stop_line_m} }} ]"
            )
        }

        // Valid: the stop line sits inside the 100 m path and defaults to zero.
        assert_eq!(validate(&base(FLOW)), Vec::new());
        assert_eq!(validate(&base(&with_stop_line(40.0))), Vec::new());

        assert!(codes(&base(&with_stop_line(-1.0))).contains(&"E_MOVEMENT_STOP_LINE"));
        assert!(codes(&base(&with_stop_line(120.0))).contains(&"E_MOVEMENT_STOP_LINE"));
    }

    #[test]
    fn flags_invalid_profile_ranges() {
        let inverted = base(&format!(
            "{FLOW}, profiles: {{ speed_mps: {{ min: 15.0, max: 9.0 }}, length_m: {{ min: 4.0, max: 5.0 }}, \
             width_m: {{ min: 1.8, max: 2.0 }}, time_gap_s: {{ min: 1.0, max: 2.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 2.5 }}, comfortable_brake_mps2: {{ min: 2.0, max: 3.5 }} }}"
        ));
        assert!(codes(&inverted).contains(&"E_PROFILE_RANGE"));

        let non_positive = base(&format!(
            "{FLOW}, profiles: {{ speed_mps: {{ min: 0.0, max: 9.0 }}, length_m: {{ min: 4.0, max: 5.0 }}, \
             width_m: {{ min: 1.8, max: 2.0 }}, time_gap_s: {{ min: 1.0, max: 2.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 2.5 }}, comfortable_brake_mps2: {{ min: 2.0, max: 3.5 }} }}"
        ));
        assert!(codes(&non_positive).contains(&"E_PROFILE_RANGE"));
    }

    #[test]
    fn flags_a_compliance_range_outside_the_unit_interval() {
        let negative = base(&format!(
            "{FLOW}, profiles: {{ speed_mps: {{ min: 9.0, max: 12.0 }}, length_m: {{ min: 4.0, max: 5.0 }}, \
             width_m: {{ min: 1.8, max: 2.0 }}, time_gap_s: {{ min: 1.0, max: 2.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 2.5 }}, comfortable_brake_mps2: {{ min: 2.0, max: 3.5 }}, \
             compliance: {{ min: -0.1, max: 0.5 }} }}"
        ));
        assert!(codes(&negative).contains(&"E_PROFILE_COMPLIANCE"));

        let above_one = base(&format!(
            "{FLOW}, profiles: {{ speed_mps: {{ min: 9.0, max: 12.0 }}, length_m: {{ min: 4.0, max: 5.0 }}, \
             width_m: {{ min: 1.8, max: 2.0 }}, time_gap_s: {{ min: 1.0, max: 2.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 2.5 }}, comfortable_brake_mps2: {{ min: 2.0, max: 3.5 }}, \
             compliance: {{ min: 0.5, max: 1.1 }} }}"
        ));
        assert!(codes(&above_one).contains(&"E_PROFILE_COMPLIANCE"));

        // A fully compliant range and the omitted default both validate.
        let valid = base(&format!(
            "{FLOW}, profiles: {{ speed_mps: {{ min: 9.0, max: 12.0 }}, length_m: {{ min: 4.0, max: 5.0 }}, \
             width_m: {{ min: 1.8, max: 2.0 }}, time_gap_s: {{ min: 1.0, max: 2.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 2.5 }}, comfortable_brake_mps2: {{ min: 2.0, max: 3.5 }}, \
             compliance: {{ min: 0.0, max: 1.0 }} }}"
        ));
        assert_eq!(validate(&valid), Vec::new());
    }

    /// A pedestrian route across a road movement: a walking path crossing the
    /// road, a waiting area and crossing, and one pedestrian demand source.
    const PEDESTRIAN: &str = "
        paths: [
            { id: 'road', points: [ { x: -40, y: 0 }, { x: 40, y: 0 } ] },
            { id: 'walk', points: [ { x: 0, y: -15 }, { x: 0, y: 15 } ] },
        ],
        portals: [
            { id: 'west', path: 'road', end: 'start', width_m: 7.0 },
            { id: 'east', path: 'road', end: 'end', width_m: 7.0 },
            { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
            { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
        ],
        regions: [
            { id: 'crossing_zone', points: [
                { x: -3, y: -3 }, { x: 3, y: -3 }, { x: 3, y: 3 }, { x: -3, y: 3 }
            ] },
            { id: 'south_kerb', points: [
                { x: -3, y: -8 }, { x: 3, y: -8 }, { x: 3, y: -5 }, { x: -3, y: -5 }
            ] },
        ],
        movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'road', priority: 0 } ],
        crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ] } ],
        waiting_areas: [ { id: 'south_wait', region: 'south_kerb' } ],
        pedestrian_routes: [ { id: 'north_crossing', from: 'south', to: 'north', path: 'walk',
            crossings: [ 'cross' ], waiting_areas: [ 'south_wait' ] } ],
        pedestrian_demand: [ { id: 'footfall', portal: 'south', rate_pph: 240.0,
            routes: [ { route: 'north_crossing', weight: 1.0 } ] } ],
        pedestrian_profiles: { radius_m: { min: 0.2, max: 0.3 },
            speed_mps: { min: 1.0, max: 1.6 } },
    ";

    #[test]
    fn accepts_a_pedestrian_route_and_demand() {
        assert_eq!(validate(&base(PEDESTRIAN)), Vec::new());
    }

    #[test]
    fn accepts_a_pedestrian_signal_and_compliance_range() {
        let with_signal = PEDESTRIAN.replace(
            "crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ] } ]",
            "crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ], \
             pedestrian_signal: { phases: [ { duration_s: 20.0, walk: true }, \
             { duration_s: 20.0, walk: false } ] } } ]",
        );
        assert!(
            with_signal.contains("pedestrian_signal"),
            "fixture must be rewritten"
        );
        assert_eq!(validate(&base(&with_signal)), Vec::new());

        let with_compliance = PEDESTRIAN.replace(
            "pedestrian_profiles: { radius_m: { min: 0.2, max: 0.3 },\n            speed_mps: { min: 1.0, max: 1.6 } }",
            "pedestrian_profiles: { radius_m: { min: 0.2, max: 0.3 },\n            speed_mps: { min: 1.0, max: 1.6 }, compliance: { min: 0.0, max: 0.8 } }",
        );
        assert_eq!(validate(&base(&with_compliance)), Vec::new());
    }

    #[test]
    fn flags_pedestrian_signal_and_compliance_errors() {
        let empty = PEDESTRIAN.replace(
            "crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ] } ]",
            "crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ], \
             pedestrian_signal: { phases: [] } } ]",
        );
        assert!(codes(&base(&empty)).contains(&"E_CROSSING_PEDESTRIAN_SIGNAL_EMPTY"));

        let bad_phase = PEDESTRIAN.replace(
            "crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ] } ]",
            "crossings: [ { id: 'cross', region: 'crossing_zone', movements: [ 'ew_through' ], \
             pedestrian_signal: { phases: [ { duration_s: 0.0, walk: true } ] } } ]",
        );
        assert!(codes(&base(&bad_phase)).contains(&"E_CROSSING_PEDESTRIAN_SIGNAL_PHASE"));

        let bad_compliance = PEDESTRIAN.replace(
            "pedestrian_profiles: { radius_m: { min: 0.2, max: 0.3 },\n            speed_mps: { min: 1.0, max: 1.6 } }",
            "pedestrian_profiles: { radius_m: { min: 0.2, max: 0.3 },\n            speed_mps: { min: 1.0, max: 1.6 }, compliance: { min: 0.5, max: 1.5 } }",
        );
        assert!(codes(&base(&bad_compliance)).contains(&"E_PROFILE_COMPLIANCE"));
    }

    #[test]
    fn flags_pedestrian_route_reference_errors() {
        let unknown_portal = PEDESTRIAN.replace("from: 'south'", "from: 'ghost'");
        assert!(codes(&base(&unknown_portal)).contains(&"E_PEDESTRIAN_ROUTE_UNKNOWN_PORTAL"));

        let unknown_path = PEDESTRIAN.replace("path: 'walk'", "path: 'ghost'");
        assert!(codes(&base(&unknown_path)).contains(&"E_PEDESTRIAN_ROUTE_UNKNOWN_PATH"));

        let mismatch = PEDESTRIAN.replace("from: 'south'", "from: 'west'");
        assert!(codes(&base(&mismatch)).contains(&"E_PEDESTRIAN_ROUTE_PORTAL_PATH_MISMATCH"));

        let self_loop =
            PEDESTRIAN.replace("from: 'south', to: 'north'", "from: 'south', to: 'south'");
        assert!(codes(&base(&self_loop)).contains(&"E_PEDESTRIAN_ROUTE_SELF_LOOP"));

        let unknown_crossing =
            PEDESTRIAN.replace("crossings: [ 'cross' ]", "crossings: [ 'ghost' ]");
        assert!(codes(&base(&unknown_crossing)).contains(&"E_PEDESTRIAN_ROUTE_UNKNOWN_CROSSING"));

        let unknown_area = PEDESTRIAN.replace(
            "waiting_areas: [ 'south_wait' ]",
            "waiting_areas: [ 'ghost' ]",
        );
        assert!(codes(&base(&unknown_area)).contains(&"E_PEDESTRIAN_ROUTE_UNKNOWN_WAITING_AREA"));
    }

    #[test]
    fn flags_waiting_area_reference_errors() {
        let unknown_region = PEDESTRIAN.replace(
            "waiting_areas: [ { id: 'south_wait', region: 'south_kerb' } ]",
            "waiting_areas: [ { id: 'south_wait', region: 'ghost' } ]",
        );
        assert!(codes(&base(&unknown_region)).contains(&"E_WAITING_AREA_UNKNOWN_REGION"));
    }

    #[test]
    fn flags_pedestrian_demand_reference_errors() {
        let unknown_portal =
            PEDESTRIAN.replace("portal: 'south', rate_pph", "portal: 'ghost', rate_pph");
        assert!(codes(&base(&unknown_portal)).contains(&"E_PEDESTRIAN_DEMAND_UNKNOWN_PORTAL"));

        let empty_routes = PEDESTRIAN.replace(
            "routes: [ { route: 'north_crossing', weight: 1.0 } ]",
            "routes: []",
        );
        assert!(codes(&base(&empty_routes)).contains(&"E_PEDESTRIAN_DEMAND_EMPTY_ROUTES"));

        let unknown_route = PEDESTRIAN.replace("route: 'north_crossing'", "route: 'ghost'");
        assert!(codes(&base(&unknown_route)).contains(&"E_PEDESTRIAN_DEMAND_UNKNOWN_ROUTE"));

        let mismatch = PEDESTRIAN.replace("portal: 'south', rate_pph", "portal: 'north', rate_pph");
        assert!(codes(&base(&mismatch)).contains(&"E_PEDESTRIAN_DEMAND_ROUTE_PORTAL_MISMATCH"));

        let duplicate = PEDESTRIAN.replace(
            "routes: [ { route: 'north_crossing', weight: 1.0 } ]",
            "routes: [ { route: 'north_crossing', weight: 1.0 }, \
             { route: 'north_crossing', weight: 2.0 } ]",
        );
        assert!(codes(&base(&duplicate)).contains(&"E_PEDESTRIAN_DEMAND_DUPLICATE_ROUTE"));

        let zero_rate = PEDESTRIAN.replace("rate_pph: 240.0", "rate_pph: 0.0");
        assert!(codes(&base(&zero_rate)).contains(&"E_NON_POSITIVE"));
    }

    #[test]
    fn flags_invalid_pedestrian_profile_ranges() {
        let inverted = PEDESTRIAN.replace(
            "speed_mps: { min: 1.0, max: 1.6 }",
            "speed_mps: { min: 1.6, max: 1.0 }",
        );
        let found = codes(&base(&inverted));
        assert!(found.contains(&"E_PROFILE_RANGE"));

        let non_positive = PEDESTRIAN.replace(
            "radius_m: { min: 0.2, max: 0.3 }",
            "radius_m: { min: 0.0, max: 0.3 }",
        );
        assert!(codes(&base(&non_positive)).contains(&"E_PROFILE_RANGE"));
    }

    #[test]
    fn reports_duplicate_ids_across_pedestrian_object_kinds() {
        let duplicate = PEDESTRIAN.replace(
            "waiting_areas: [ { id: 'south_wait', region: 'south_kerb' } ]",
            "waiting_areas: [ { id: 'north_crossing', region: 'south_kerb' } ]",
        );
        assert!(codes(&base(&duplicate)).contains(&"E_ID_DUPLICATE"));
    }

    #[test]
    fn reports_duplicate_ids_across_new_object_kinds() {
        let source = base(
            "paths: [ { id: 'shared', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'a', path: 'shared', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'shared', end: 'end', width_m: 3.0 } ], \
             regions: [ { id: 'shared', points: [
                { x: 0, y: 0 }, { x: 1, y: 0 }, { x: 1, y: 1 }
             ] } ]",
        );
        assert!(codes(&source).contains(&"E_ID_DUPLICATE"));
    }

    /// A capsule `bicycle` template tolerant of only a gentle curve, for the
    /// curvature-against-turning-limit fixtures.
    const CURVING_BICYCLE: &str = r#"{
        schema_version: 2, id: 'curving', coordinate_system: { x: 'a', y: 'b' },
        paths: [], portals: [],
        mode_templates: [ {
            id: 'bicycle',
            body: { kind: 'capsule', length_m: { min: 1.6, max: 1.9 },
                radius_m: { min: 0.30, max: 0.40 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility' ] },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 3.5, max: 6.5 },
                max_accel_mps2: { min: 0.8, max: 1.5 },
                comfortable_brake_mps2: { min: 1.5, max: 3.0 },
                time_gap_s: { min: 0.8, max: 1.4 },
                steering_rate_max_rad_s: { min: 0.6, max: 1.2 },
                lateral_clearance_m: { min: 0.20, max: 0.50 },
                compliance: { min: 0.8, max: 1.0 },
            },
        } ],
    }"#;

    #[test]
    fn rejects_a_reference_curved_beyond_a_modes_turning_limit() {
        let source =
            crate::source::parse_scenario_source_v2(CURVING_BICYCLE).expect("template parses");
        let modes: Vec<&ModeTemplateSource> = source.mode_templates.iter().collect();

        // The turning limit is `steering_rate_max / speed_max`, the tightest
        // steady curve the mode can hold at its top desired speed.
        let limit =
            mode_turning_limit_curvature(&source.mode_templates[0]).expect("a steering limit");
        assert!((limit - 1.2 / 6.5).abs() < 1e-12);

        // A 3 m radius arc (`kappa = 1/3`) is far tighter than the limit.
        let tight = CompiledReferencePath::arc(DVec2::ZERO, 3.0, 0.0, std::f64::consts::FRAC_PI_2);
        let diagnostics = facility_curvature_diagnostics("west", &tight, 0.0, &modes);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::FacilityCurvature);

        // A 50 m radius arc (`kappa = 1/50`) is inside the limit, and a straight
        // polyline reference has zero curvature everywhere.
        let gentle =
            CompiledReferencePath::arc(DVec2::ZERO, 50.0, 0.0, std::f64::consts::FRAC_PI_2);
        assert!(facility_curvature_diagnostics("west", &gentle, 0.0, &modes).is_empty());
        let straight = CompiledReferencePath::from_polyline(&[DVec2::ZERO, DVec2::new(80.0, 0.0)]);
        assert!(facility_curvature_diagnostics("west", &straight, 0.0, &modes).is_empty());
    }

    /// A minimal box `bus` document with an optional authored axle geometry.
    fn axle_bus(geometry: &str, motion: &str) -> ScenarioSourceV2 {
        let document = format!(
            r#"{{ schema_version: 2, id: 'axle', coordinate_system: {{ x: 'a', y: 'b' }},
                paths: [], portals: [],
                mode_templates: [ {{
                    id: 'bus',
                    body: {{ kind: 'box', length_m: {{ min: 12.0, max: 12.0 }},
                        width_m: {{ min: 2.55, max: 2.55 }} }},
                    motion: '{motion}',
                    tactics: [ 'follow', 'stop', 'yield' ],
                    access: {{ facility_kinds: [ 'facility' ] }},
                    occupancy: 'operator_only',
                    {geometry}
                    profiles: {{
                        speed_mps: {{ min: 9.0, max: 9.0 }},
                        max_accel_mps2: {{ min: 0.9, max: 0.9 }},
                        comfortable_brake_mps2: {{ min: 1.8, max: 1.8 }},
                        time_gap_s: {{ min: 1.6, max: 1.6 }},
                        compliance: {{ min: 1.0, max: 1.0 }},
                    }},
                }} ],
            }}"#
        );
        crate::source::parse_scenario_source_v2(&document).expect("the template parses")
    }

    /// A 6.0 m wheelbase and a 0.3 rad steering-angle limit, whose single-track
    /// curvature `tan(0.3) / 6.0` is the mode's turning limit (`0.8 / 9.0` would
    /// be a heading-rate bound, which a non-lateral box does not author).
    const AXLE_GEOMETRY: &str = "wheelbase_m: { min: 6.0, max: 6.0 },\
        steering_angle_max_rad: { min: 0.3, max: 0.3 },";

    #[test]
    fn authored_axle_geometry_bounds_the_turning_limit() {
        let source = axle_bus(AXLE_GEOMETRY, "single_body_wheeled");
        let diagnostics = validate_v2(&source);
        assert!(
            diagnostics.is_empty(),
            "a wheelbase and steering angle on a wheeled motion are valid: {diagnostics:?}"
        );
        let modes: Vec<&ModeTemplateSource> = source.mode_templates.iter().collect();
        let limit = mode_turning_limit_curvature(&source.mode_templates[0]).expect("an axle limit");
        let geometry = 0.3_f64.tan() / 6.0;
        assert!(
            (limit - geometry).abs() < 1e-12,
            "the axle geometry gives the turning limit {geometry}"
        );

        // A constant-radius arc inside the limit is admitted ...
        let gentle =
            CompiledReferencePath::arc(DVec2::ZERO, 40.0, 0.0, std::f64::consts::FRAC_PI_2);
        assert!(facility_curvature_diagnostics("curve", &gentle, 0.0, &modes).is_empty());
        // ... and one beyond it is rejected, even though its endpoints never
        // overlap.
        let tight = CompiledReferencePath::arc(DVec2::ZERO, 10.0, 0.0, std::f64::consts::FRAC_PI_2);
        let diagnostics = facility_curvature_diagnostics("curve", &tight, 0.0, &modes);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::FacilityCurvature);
    }

    #[test]
    fn a_chord_approximation_of_an_authored_curve_is_measured_at_its_radius() {
        // A quarter circle of `radius` as 24 chords: the authored `paths[].points`
        // form a constant-radius fixture actually uses.
        let chord = |radius: f64| -> Vec<PointSource> {
            const SEGMENTS: usize = 24;
            (0..=SEGMENTS)
                .map(|index| {
                    let theta = std::f64::consts::FRAC_PI_2 * index as f64 / SEGMENTS as f64;
                    PointSource {
                        x: radius * theta.cos(),
                        y: radius * theta.sin(),
                    }
                })
                .collect()
        };
        let compile = |points: &[PointSource]| {
            CompiledReferencePath::from_polyline(
                &points
                    .iter()
                    .map(|point| DVec2::new(point.x, point.y))
                    .collect::<Vec<_>>(),
            )
        };
        let source = axle_bus(AXLE_GEOMETRY, "single_body_wheeled");
        let modes: Vec<&ModeTemplateSource> = source.mode_templates.iter().collect();

        let gentle = chord(40.0);
        let corner = authored_max_corner_curvature(&gentle);
        assert!(
            (corner - 1.0 / 40.0).abs() < 1e-5,
            "a 40 m chord approximation reads as 1/40, got {corner}"
        );
        assert!(
            facility_curvature_diagnostics("curve", &compile(&gentle), corner, &modes).is_empty()
        );

        let tight = chord(10.0);
        let corner = authored_max_corner_curvature(&tight);
        assert!((corner - 1.0 / 10.0).abs() < 1e-3);
        assert_eq!(
            facility_curvature_diagnostics("curve", &compile(&tight), corner, &modes).len(),
            1
        );
    }

    #[test]
    fn axle_geometry_must_be_a_wheeled_pair() {
        // A wheelbase without its steering-angle companion bounds no curvature.
        let lone_wheelbase = validate_v2(&axle_bus(
            "wheelbase_m: { min: 6.0, max: 6.0 },",
            "single_body_wheeled",
        ));
        assert!(
            lone_wheelbase
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeWheelbase)
        );
        // A steering angle without its wheelbase companion is equally inert.
        let lone_angle = validate_v2(&axle_bus(
            "steering_angle_max_rad: { min: 0.3, max: 0.3 },",
            "single_body_wheeled",
        ));
        assert!(
            lone_angle
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeSteeringAngle)
        );
        // A steering angle at or beyond a right angle is not a range.
        let right_angle = validate_v2(&axle_bus(
            "wheelbase_m: { min: 6.0, max: 6.0 },\n        steering_angle_max_rad: { min: 0.3, max: 1.6 },",
            "single_body_wheeled",
        ));
        assert!(
            right_angle
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeSteeringAngle)
        );
        // Axle geometry on a walking motion has no axles to bound.
        let walking = validate_v2(&axle_bus(
            "wheelbase_m: { min: 6.0, max: 6.0 },\n        steering_angle_max_rad: { min: 0.3, max: 0.3 },",
            "holonomic_walking",
        ));
        assert!(
            walking
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeWheelbase)
        );
    }

    /// A minimal `tractor_semitrailer` document with a substitutable body
    /// clause, so a test can author a broken chain without repeating the rest
    /// of the template.
    fn articulated_document(body: &str) -> ScenarioSourceV2 {
        let document = format!(
            r#"{{ schema_version: 2, id: 'articulated', coordinate_system: {{ x: 'a', y: 'b' }},
                paths: [], portals: [],
                mode_templates: [ {{
                    id: 'tractor_semitrailer',
                    body: {body},
                    motion: 'articulated_wheeled',
                    tactics: [ 'follow', 'stop', 'yield' ],
                    access: {{ facility_kinds: [ 'facility' ] }},
                    occupancy: 'operator_only',
                    profiles: {{
                        speed_mps: {{ min: 8.0, max: 8.0 }},
                        max_accel_mps2: {{ min: 0.7, max: 0.7 }},
                        comfortable_brake_mps2: {{ min: 1.4, max: 1.4 }},
                        time_gap_s: {{ min: 2.0, max: 2.0 }},
                        compliance: {{ min: 1.0, max: 1.0 }},
                    }},
                }} ],
            }}"#
        );
        crate::source::parse_scenario_source_v2(&document).expect("the template parses")
    }

    /// A valid two-segment tractor-semitrailer chain: a 6.0 m tractor with no
    /// hitch offset and a 13.6 m trailer hitched 1.2 m behind it, under a
    /// 0.9 rad (about 51.5 degree) articulation limit.
    const VALID_CHAIN: &str = "{ kind: 'articulated_chain',
        segments: [
            { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 } },
            { length_m: { min: 13.6, max: 13.6 }, width_m: { min: 2.55, max: 2.55 },
                hitch_offset_m: { min: 1.2, max: 1.2 } },
        ],
        articulation_limit_rad: { min: 0.9, max: 0.9 } }";

    #[test]
    fn a_valid_articulated_chain_is_accepted() {
        let diagnostics = validate_v2(&articulated_document(VALID_CHAIN));
        assert!(
            diagnostics.is_empty(),
            "a two-segment chain with a lead-only-omitted hitch offset and a sane articulation \
             limit is valid: {diagnostics:?}"
        );
    }

    #[test]
    fn an_articulated_chain_needs_at_least_two_segments() {
        let single_segment = "{ kind: 'articulated_chain',
            segments: [ { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 } } ],
            articulation_limit_rad: { min: 0.9, max: 0.9 } }";
        let diagnostics = validate_v2(&articulated_document(single_segment));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeArticulatedSegments)
        );
    }

    #[test]
    fn the_lead_segment_must_not_author_a_hitch_offset() {
        let lead_with_hitch = "{ kind: 'articulated_chain',
            segments: [
                { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 },
                    hitch_offset_m: { min: 1.0, max: 1.0 } },
                { length_m: { min: 13.6, max: 13.6 }, width_m: { min: 2.55, max: 2.55 },
                    hitch_offset_m: { min: 1.2, max: 1.2 } },
            ],
            articulation_limit_rad: { min: 0.9, max: 0.9 } }";
        let diagnostics = validate_v2(&articulated_document(lead_with_hitch));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeArticulatedSegments)
        );
    }

    #[test]
    fn a_trailing_segment_must_author_a_hitch_offset() {
        let trailer_without_hitch = "{ kind: 'articulated_chain',
            segments: [
                { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 } },
                { length_m: { min: 13.6, max: 13.6 }, width_m: { min: 2.55, max: 2.55 } },
            ],
            articulation_limit_rad: { min: 0.9, max: 0.9 } }";
        let diagnostics = validate_v2(&articulated_document(trailer_without_hitch));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeArticulatedSegments)
        );
    }

    #[test]
    fn an_inverted_segment_dimension_is_rejected() {
        let inverted_width = "{ kind: 'articulated_chain',
            segments: [
                { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 } },
                { length_m: { min: 13.6, max: 13.6 }, width_m: { min: 3.0, max: 2.0 },
                    hitch_offset_m: { min: 1.2, max: 1.2 } },
            ],
            articulation_limit_rad: { min: 0.9, max: 0.9 } }";
        let diagnostics = validate_v2(&articulated_document(inverted_width));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeArticulatedSegments)
        );
    }

    #[test]
    fn the_articulation_limit_must_be_inside_zero_pi() {
        let too_wide = "{ kind: 'articulated_chain',
            segments: [
                { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 } },
                { length_m: { min: 13.6, max: 13.6 }, width_m: { min: 2.55, max: 2.55 },
                    hitch_offset_m: { min: 1.2, max: 1.2 } },
            ],
            articulation_limit_rad: { min: 0.9, max: 3.2 } }";
        let diagnostics = validate_v2(&articulated_document(too_wide));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeArticulationLimit)
        );

        let negative = "{ kind: 'articulated_chain',
            segments: [
                { length_m: { min: 6.0, max: 6.0 }, width_m: { min: 2.5, max: 2.5 } },
                { length_m: { min: 13.6, max: 13.6 }, width_m: { min: 2.55, max: 2.55 },
                    hitch_offset_m: { min: 1.2, max: 1.2 } },
            ],
            articulation_limit_rad: { min: -0.1, max: 0.5 } }";
        let diagnostics = validate_v2(&articulated_document(negative));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ModeArticulationLimit)
        );
    }

    #[test]
    fn shared_boundary_midpoint_needs_a_segment_not_a_corner() {
        let left = [
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
        ];
        // A ring sharing only the corner `(10, 10)` yields no coordinate.
        let corner = [
            DVec2::new(10.0, 10.0),
            DVec2::new(20.0, 10.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(10.0, 20.0),
        ];
        assert_eq!(shared_boundary_midpoint(&left, &corner), None);

        // A ring sharing the edge `x = 10` yields its midpoint.
        let right = [
            DVec2::new(10.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 10.0),
            DVec2::new(10.0, 10.0),
        ];
        assert_eq!(
            shared_boundary_midpoint(&left, &right),
            Some(DVec2::new(10.0, 5.0))
        );
    }
}
