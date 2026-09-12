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

use crate::source::{PathEnd, SUPPORTED_SCHEMA_VERSION, ScenarioSource};

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
        .chain(source.portals.iter().map(|portal| portal.id.as_str()));
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
}
