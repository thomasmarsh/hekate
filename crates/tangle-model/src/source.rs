//! Versioned JSON5 source schema.
//!
//! These structures mirror the authored scenario document one-to-one. They are
//! parsed with Serde and intentionally contain no derived geometry or dense
//! identifiers: that is the job of [`crate::compile`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Scenario schema version understood by this build.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// A hand-authored scenario document before validation or compilation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSource {
    /// Schema version the document was written against.
    pub schema_version: u32,
    /// Stable scenario identifier recorded in run provenance.
    pub id: String,
    /// Human-facing names for the two world axes.
    pub coordinate_system: CoordinateSystem,
    /// Guide paths agents travel along.
    pub paths: Vec<PathSource>,
    /// Entry and exit points attached to path ends.
    pub portals: Vec<PortalSource>,
    /// Walking-skeleton population tuning.
    #[serde(default)]
    pub population: PopulationSource,
}

/// Human-facing names for the world coordinate axes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinateSystem {
    /// Name of the positive x axis, for example `east_m`.
    pub x: String,
    /// Name of the positive y axis, for example `north_m`.
    pub y: String,
}

/// One authored guide path as an ordered polyline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathSource {
    /// Stable path identifier, unique across all authored objects.
    pub id: String,
    /// Ordered vertices of the guide path.
    pub points: Vec<PointSource>,
}

/// A world-space vertex in metres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointSource {
    /// East/north coordinate in metres.
    pub x: f64,
    /// East/north coordinate in metres.
    pub y: f64,
}

/// Which end of a path a portal is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PathEnd {
    /// The first vertex of the path.
    Start,
    /// The last vertex of the path.
    End,
}

/// An entry or exit point attached to one end of a path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PortalSource {
    /// Stable portal identifier, unique across all authored objects.
    pub id: String,
    /// Identifier of the path this portal belongs to.
    pub path: String,
    /// Which end of the path the portal marks.
    pub end: PathEnd,
    /// Traversable width of the portal in metres.
    pub width_m: f64,
}

/// Walking-skeleton population tuning.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PopulationSource {
    /// Number of vehicles introduced on the guide path.
    pub vehicle_count: u32,
    /// Constant vehicle speed in metres per second.
    pub vehicle_speed_mps: f64,
    /// Longitudinal gap between adjacent vehicles in metres.
    pub vehicle_spacing_m: f64,
    /// Vehicle body length in metres.
    pub vehicle_length_m: f64,
    /// Vehicle body width in metres.
    pub vehicle_width_m: f64,
}

impl Default for PopulationSource {
    fn default() -> Self {
        Self {
            vehicle_count: 6,
            vehicle_speed_mps: 12.0,
            vehicle_spacing_m: 20.0,
            vehicle_length_m: 4.5,
            vehicle_width_m: 1.8,
        }
    }
}

/// Failure to read a [`ScenarioSource`] from text.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The document was not well-formed JSON5 or did not match the schema.
    #[error("scenario source is not valid JSON5 for the source schema: {0}")]
    Json5(#[from] serde_json5::Error),
}

/// Parse a JSON5 scenario document into its source representation.
///
/// This performs structural parsing only. Semantic validation is a separate,
/// mandatory step ([`crate::validate`]) so that every diagnostic carries a
/// stable code and source object identifier.
pub fn parse_scenario_source(input: &str) -> Result<ScenarioSource, ParseError> {
    let source: ScenarioSource = serde_json5::from_str(input)?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALKING: &str = r#"
    {
      // A single straight guide path with a portal at each end.
      schema_version: 1,
      id: 'walking_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
      ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      population: {
        vehicle_count: 4,
        vehicle_speed_mps: 12.0,
        vehicle_spacing_m: 20.0,
        vehicle_length_m: 4.5,
        vehicle_width_m: 1.8,
      },
    }
    "#;

    #[test]
    fn parses_a_json5_scenario_with_comments_and_trailing_commas() {
        let source = parse_scenario_source(WALKING).expect("walking scenario parses");
        assert_eq!(source.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(source.id, "walking_guide_v1");
        assert_eq!(source.paths.len(), 1);
        assert_eq!(source.portals.len(), 2);
        assert_eq!(source.population.vehicle_count, 4);
    }

    #[test]
    fn rejects_unknown_fields() {
        let input = r#"{ schema_version: 1, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [], portals: [], typo: true }"#;
        assert!(parse_scenario_source(input).is_err());
    }

    #[test]
    fn population_defaults_when_omitted() {
        let input = r#"{ schema_version: 1, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [], portals: [] }"#;
        let source = parse_scenario_source(input).expect("parses");
        assert_eq!(
            source.population.vehicle_count,
            PopulationSource::default().vehicle_count
        );
    }
}
