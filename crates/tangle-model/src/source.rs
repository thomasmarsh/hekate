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
    /// Closed polygons marking the non-traversable world limits.
    #[serde(default)]
    pub boundaries: Vec<PolygonSource>,
    /// Closed polygons marking traversable areas other than guide paths.
    #[serde(default)]
    pub regions: Vec<PolygonSource>,
    /// Movement connectors from one portal to another along a guide path.
    #[serde(default)]
    pub movements: Vec<MovementSource>,
    /// Pedestrian crossings over one or more movements.
    #[serde(default)]
    pub crossings: Vec<CrossingSource>,
    /// Authored conflict regions shared by pairs of movements.
    #[serde(default)]
    pub conflict_regions: Vec<ConflictRegionSource>,
    /// Right-of-way or control rules attached to movements.
    #[serde(default)]
    pub rules: Vec<RuleSource>,
    /// Fixed-time signal controllers with phased signal heads.
    #[serde(default)]
    pub signals: Vec<SignalSource>,
    /// Portal demand generators; when non-empty they replace the static
    /// walking-skeleton population.
    #[serde(default)]
    pub demand: Vec<DemandSource>,
    /// Passenger-car physical and behavior profile distributions.
    #[serde(default)]
    pub profiles: ProfileSource,
    /// Walking-skeleton population tuning, used only when `demand` is empty.
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

/// A closed polygon in metres.
///
/// The ring closes implicitly from the last vertex back to the first, so the
/// authored vertex list must not repeat the first point at the end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolygonSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Ordered ring vertices in metres.
    pub points: Vec<PointSource>,
}

/// A movement connector from one portal to another along a guide path.
///
/// A movement is the Phase 1 routing primitive: it names where an agent enters
/// the modeled area, where it leaves, and the guide path between them. Conflict
/// regions, crossings, and control rules all reference movements rather than
/// naming a scenario kind, so a layout such as a four-leg intersection is data,
/// not a simulator branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovementSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Portal where the movement begins.
    pub from: String,
    /// Portal where the movement ends.
    pub to: String,
    /// Guide path the movement follows.
    pub path: String,
    /// Right-of-way rank; a lower value is honored before a higher one.
    pub priority: u32,
    /// Stop-line arc length in metres from the movement entry, measured along
    /// the movement's direction of travel. Omitted means `0.0`, the entry
    /// portal. A controller that must stop holds the vehicle's front bumper at
    /// this position.
    #[serde(default)]
    pub stop_line_m: f64,
}

/// A pedestrian crossing occupying a traversable region over some movements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CrossingSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Traversable region the crossing occupies.
    pub region: String,
    /// Movements the crossing crosses.
    pub movements: Vec<String>,
}

/// An authored conflict region shared by two movements.
///
/// The geometry lets a renderer or metric show where two movement envelopes
/// cross; the movement pair is what a signal check uses to reject conflicting
/// greens. A scenario may author it explicitly or, later, accept a derived one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConflictRegionSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Ordered ring vertices in metres.
    pub points: Vec<PointSource>,
    /// Exactly two movements whose envelopes conflict in this region.
    pub movements: Vec<String>,
}

/// How a movement's right of way is controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// No control; the movement proceeds subject to ordinary interaction.
    Free,
    /// The movement must yield to the conflicting movements.
    Yield,
    /// The movement must stop before proceeding.
    Stop,
    /// A signal controller governs the movement.
    Signal,
}

/// A control rule attached to one movement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Movement this rule governs.
    pub movement: String,
    /// Kind of control the rule applies.
    pub kind: RuleKind,
    /// Signal controller, present exactly when `kind` is `signal`.
    #[serde(default)]
    pub signal: Option<String>,
}

/// Display color of one signal head during one phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignalColor {
    /// The controlled movement must stop.
    Red,
    /// The controlled movement should prepare to stop.
    Yellow,
    /// The controlled movement may proceed.
    Green,
}

/// One signal head controlling a movement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalHeadSource {
    /// Identifier unique within the owning signal.
    pub id: String,
    /// Movement this head controls.
    pub movement: String,
}

/// One head's color during one phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalStateSource {
    /// Head identifier within the owning signal.
    pub head: String,
    /// Color shown on that head during this phase.
    pub color: SignalColor,
}

/// One fixed-time signal phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalPhaseSource {
    /// Phase duration in seconds.
    pub duration_s: f64,
    /// The color of every head in the owning signal during this phase.
    pub states: Vec<SignalStateSource>,
}

/// A fixed-time signal controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignalSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Signal heads controlled together.
    pub heads: Vec<SignalHeadSource>,
    /// Phases in cycle order, each showing every head once.
    pub phases: Vec<SignalPhaseSource>,
}

/// One portal demand generator: arrivals at an entry portal and the routes
/// those vehicles take.
///
/// A demand source names an entry portal (the `from` portal of one or more
/// movements) and a mean arrival rate. Each generated vehicle is assigned one
/// of the listed movements by relative weight, so route assignment is scenario
/// data rather than a simulator branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DemandSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Entry portal where generated vehicles enter the world.
    pub portal: String,
    /// Mean arrival rate in vehicles per hour.
    pub rate_vph: f64,
    /// Movements a generated vehicle may follow, with relative weights.
    pub routes: Vec<RouteShareSource>,
}

/// One movement's relative share of a demand source's arrivals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteShareSource {
    /// Movement a generated vehicle follows.
    pub movement: String,
    /// Relative weight; a larger weight is chosen proportionally more often.
    pub weight: f64,
}

/// An inclusive uniform distribution for one profile parameter.
///
/// A range with `min == max` is a constant. The owning field name carries the
/// physical unit, for example `speed_mps` or `time_gap_s`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileRangeSource {
    /// Lower bound of the distribution.
    pub min: f64,
    /// Upper bound of the distribution.
    pub max: f64,
}

/// Passenger-car physical and behavior profile distributions.
///
/// Every generated vehicle samples one value from each range from the `profile`
/// random stream, so its body and longitudinal behavior are stable for the run.
/// Pedestrian profiles are Increment 3 work.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileSource {
    /// Desired free-flow speed in metres per second.
    pub speed_mps: ProfileRangeSource,
    /// Body length in metres.
    pub length_m: ProfileRangeSource,
    /// Body width in metres.
    pub width_m: ProfileRangeSource,
    /// Desired following time gap in seconds.
    pub time_gap_s: ProfileRangeSource,
    /// Maximum acceleration in metres per second squared.
    pub max_accel_mps2: ProfileRangeSource,
    /// Comfortable deceleration in metres per second squared.
    pub comfortable_brake_mps2: ProfileRangeSource,
}

impl Default for ProfileSource {
    fn default() -> Self {
        // Provisional engineering defaults, not calibrated scientific claims.
        Self {
            speed_mps: ProfileRangeSource {
                min: 9.0,
                max: 15.0,
            },
            length_m: ProfileRangeSource { min: 4.0, max: 5.2 },
            width_m: ProfileRangeSource { min: 1.7, max: 2.0 },
            time_gap_s: ProfileRangeSource { min: 1.0, max: 2.0 },
            max_accel_mps2: ProfileRangeSource { min: 1.2, max: 2.5 },
            comfortable_brake_mps2: ProfileRangeSource { min: 2.0, max: 3.5 },
        }
    }
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
        assert!(source.demand.is_empty());
        assert_eq!(source.profiles, ProfileSource::default());
    }

    #[test]
    fn parses_demand_routes_and_profiles() {
        let input = r#"{
            schema_version: 1, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 50, y: 0 } ] } ],
            portals: [ { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
                       { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 } ],
            movements: [ { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 } ],
            demand: [ { id: 'inflow', portal: 'entry', rate_vph: 720.0,
                routes: [ { movement: 'through', weight: 1.0 } ] } ],
            profiles: {
                speed_mps: { min: 10.0, max: 14.0 },
                length_m: { min: 4.0, max: 5.0 },
                width_m: { min: 1.8, max: 2.0 },
                time_gap_s: { min: 1.2, max: 1.8 },
                max_accel_mps2: { min: 1.5, max: 2.5 },
                comfortable_brake_mps2: { min: 2.0, max: 3.0 },
            },
        }"#;
        let source = parse_scenario_source(input).expect("parses");
        assert_eq!(source.demand.len(), 1);
        assert_eq!(source.demand[0].portal, "entry");
        assert_eq!(source.demand[0].rate_vph, 720.0);
        assert_eq!(source.demand[0].routes[0].movement, "through");
        assert_eq!(source.profiles.speed_mps.min, 10.0);
        assert_eq!(source.profiles.time_gap_s.max, 1.8);
    }
}
