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

use crate::source::{
    PathEnd, PointSource, ProfileRangeSource, RuleKind, SUPPORTED_SCHEMA_VERSION, ScenarioSource,
    SignalColor,
};

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
    /// A crossing references a region that is not declared.
    CrossingUnknownRegion,
    /// A crossing references a movement that is not declared.
    CrossingUnknownMovement,
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
            Self::CrossingEmpty => "E_CROSSING_EMPTY",
            Self::CrossingUnknownRegion => "E_CROSSING_UNKNOWN_REGION",
            Self::CrossingUnknownMovement => "E_CROSSING_UNKNOWN_MOVEMENT",
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

/// Validate a source scenario, returning every diagnostic in source order.
///
/// An empty result means the scenario is safe to compile.
pub fn validate(source: &ScenarioSource) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if source.schema_version != SUPPORTED_SCHEMA_VERSION {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnsupportedSchemaVersion,
            Some(source.id.clone()),
            format!(
                "schema_version {} is not supported; this build reads version {}",
                source.schema_version, SUPPORTED_SCHEMA_VERSION
            ),
        ));
    }

    validate_ids(source, &mut diagnostics);
    validate_paths(source, &mut diagnostics);
    validate_portals(source, &mut diagnostics);
    validate_polygons(source, &mut diagnostics);
    validate_movements(source, &mut diagnostics);
    validate_crossings(source, &mut diagnostics);
    validate_conflict_regions(source, &mut diagnostics);
    validate_rules(source, &mut diagnostics);
    validate_signals(source, &mut diagnostics);
    validate_demand(source, &mut diagnostics);
    validate_profiles(source, &mut diagnostics);
    validate_population(source, &mut diagnostics);

    diagnostics
}

fn validate_ids(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    if source.id.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::EmptyId,
            None,
            "scenario id must not be empty",
        ));
    }

    let authored = source
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
        .chain(source.demand.iter().map(|demand| demand.id.as_str()));
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

fn validate_paths(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for path in &source.paths {
        let object = Some(path.id.clone());
        if path.points.len() < 2 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::EmptyPath,
                object,
                "a guide path needs at least two vertices",
            ));
            continue;
        }

        let mut length = 0.0;
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
        for pair in path.points.windows(2) {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            length += (dx * dx + dy * dy).sqrt();
        }
        if !length.is_finite() || length <= 0.0 {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DegeneratePath,
                Some(path.id.clone()),
                format!("path '{}' has no traversable length", path.id),
            ));
        }
    }
}

fn validate_portals(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for portal in &source.portals {
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

        let Some(path) = source.paths.iter().find(|path| path.id == portal.path) else {
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

        let duplicate = source
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
    for (index, portal) in source.portals.iter().enumerate() {
        let Some(position) = portal_position(source, portal) else {
            continue;
        };
        for other in source.portals.iter().skip(index + 1) {
            let Some(other_position) = portal_position(source, other) else {
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
fn portal_position(
    source: &ScenarioSource,
    portal: &crate::source::PortalSource,
) -> Option<PointSource> {
    let path = source.paths.iter().find(|path| path.id == portal.path)?;
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
        validate_profile_range(source, field, range, diagnostics);
    }
}

fn validate_profile_range(
    source: &ScenarioSource,
    field: &str,
    range: ProfileRangeSource,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let finite = range.min.is_finite() && range.max.is_finite();
    if !finite || range.min <= 0.0 || range.min > range.max {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::ProfileRangeInvalid,
            Some(source.id.clone()),
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

fn validate_polygons(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for polygon in &source.boundaries {
        validate_polygon(&polygon.id, &polygon.points, diagnostics);
    }
    for polygon in &source.regions {
        validate_polygon(&polygon.id, &polygon.points, diagnostics);
    }
}

fn validate_movements(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for movement in &source.movements {
        let from = source
            .portals
            .iter()
            .find(|portal| portal.id == movement.from);
        let to = source
            .portals
            .iter()
            .find(|portal| portal.id == movement.to);
        let path = source.paths.iter().find(|path| path.id == movement.path);

        if from.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementUnknownPortal,
                Some(movement.id.clone()),
                format!(
                    "movement '{}' starts at undeclared portal '{}'",
                    movement.id, movement.from
                ),
            ));
        }
        if to.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementUnknownPortal,
                Some(movement.id.clone()),
                format!(
                    "movement '{}' ends at undeclared portal '{}'",
                    movement.id, movement.to
                ),
            ));
        }
        if movement.from == movement.to {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementSelfLoop,
                Some(movement.id.clone()),
                format!(
                    "movement '{}' starts and ends at portal '{}'",
                    movement.id, movement.from
                ),
            ));
        }
        if path.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MovementUnknownPath,
                Some(movement.id.clone()),
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
                        Some(movement.id.clone()),
                        format!(
                            "movement '{}' {role} portal '{}' attaches to path '{}', not '{}'",
                            movement.id, portal.id, portal.path, movement.path
                        ),
                    ));
                }
            }
        }
    }
}

fn validate_crossings(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for crossing in &source.crossings {
        if crossing.movements.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::CrossingEmpty,
                Some(crossing.id.clone()),
                format!("crossing '{}' crosses no movements", crossing.id),
            ));
        }
        if !source
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
            if !source
                .movements
                .iter()
                .any(|candidate| candidate.id == *movement)
            {
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
    }
}

fn validate_conflict_regions(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for conflict in &source.conflict_regions {
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
            if !source
                .movements
                .iter()
                .any(|candidate| candidate.id == *movement)
            {
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

fn validate_rules(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for rule in &source.rules {
        if !source
            .movements
            .iter()
            .any(|movement| movement.id == rule.movement)
        {
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
                if !source
                    .signals
                    .iter()
                    .any(|candidate| candidate.id == signal)
                {
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

fn validate_signals(source: &ScenarioSource, diagnostics: &mut Vec<Diagnostic>) {
    for signal in &source.signals {
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
            if !source
                .movements
                .iter()
                .any(|movement| movement.id == head.movement)
            {
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

            for conflict in &source.conflict_regions {
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
        source.schema_version = 2;
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
}
