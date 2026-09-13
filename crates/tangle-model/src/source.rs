//! Versioned JSON5 source schema.
//!
//! These structures mirror the authored scenario document one-to-one. They are
//! parsed with Serde and intentionally contain no derived geometry or dense
//! identifiers: that is the job of [`crate::compile`].

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Scenario schema version understood by this build.
///
/// Version 2 is the supported source schema. Version 1 keeps a direct read path
/// until the deterministic migration replaces it; see
/// [`MIN_SUPPORTED_SCHEMA_VERSION`].
pub const SUPPORTED_SCHEMA_VERSION: u32 = 2;

/// Lowest schema version this build still reads directly.
///
/// A version-1 document has a transitional direct path until the migration step
/// (a later leaf) routes it through the version-1 to version-2 transform.
pub const MIN_SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Schema versions this build reads, lowest first.
pub const READABLE_SCHEMA_VERSIONS: [u32; 2] =
    [MIN_SUPPORTED_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION];

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
    /// Named waiting areas where pedestrians stage before crossing.
    #[serde(default)]
    pub waiting_areas: Vec<WaitingAreaSource>,
    /// Pedestrian routes from one portal to another along a guide path.
    #[serde(default)]
    pub pedestrian_routes: Vec<PedestrianRouteSource>,
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
    /// Pedestrian demand generators; when non-empty they generate pedestrian
    /// agents and share the world with vehicle demand.
    #[serde(default)]
    pub pedestrian_demand: Vec<PedestrianDemandSource>,
    /// Passenger-car physical and behavior profile distributions.
    #[serde(default)]
    pub profiles: ProfileSource,
    /// Pedestrian physical and behavior profile distributions.
    #[serde(default)]
    pub pedestrian_profiles: PedestrianProfileSource,
    /// Walking-skeleton population tuning, used only when no demand source of
    /// either mode is declared.
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
    /// Fixed-time pedestrian signal rule for this crossing.
    ///
    /// Omitted (the default) is an uncontrolled crossing: pedestrians cross it
    /// freely, subject to ordinary interaction. Additive schema version 1
    /// field, mirroring the `stop_line_m`/`compliance` precedent.
    #[serde(default)]
    pub pedestrian_signal: Option<PedestrianSignalSource>,
}

/// A fixed-time pedestrian signal rule embedded in one crossing.
///
/// It is the pedestrian analogue of [`SignalSource`] but its controlled object
/// is the owning crossing rather than a movement, so a pedestrian rule needs no
/// vehicle signal. Its phases alternate a walk interval, during which crossing
/// is permitted, and a don't-walk interval, during which the signal forbids
/// crossing; the phases are contiguous and cover the cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PedestrianSignalSource {
    /// Phases in cycle order.
    pub phases: Vec<PedestrianSignalPhaseSource>,
}

/// One fixed-time pedestrian signal phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PedestrianSignalPhaseSource {
    /// Phase duration in seconds.
    pub duration_s: f64,
    /// Whether pedestrians may cross during this phase.
    pub walk: bool,
}

/// A named waiting area where pedestrians stage between crossings.
///
/// The area is the region itself; the named reference gives a pedestrian route
/// a stable object to wait at and a later increment a place to attach staging
/// behavior. It traverses no movement and carries no control of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitingAreaSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Traversable region the waiting area occupies.
    pub region: String,
}

/// A pedestrian route from one portal to another along a guide path.
///
/// A route is the pedestrian routing primitive, mirroring [`MovementSource`]
/// for the vehicle mode. It reuses the same path geometry and portals, and
/// additionally names the crossings and waiting areas it passes through in
/// travel order, so signal compliance and staging can be attached per route
/// without naming a scenario kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PedestrianRouteSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Portal where the route begins.
    pub from: String,
    /// Portal where the route ends.
    pub to: String,
    /// Guide path the route follows.
    pub path: String,
    /// Crossings the route traverses, in travel order.
    #[serde(default)]
    pub crossings: Vec<String>,
    /// Waiting areas the route stages at, in travel order.
    #[serde(default)]
    pub waiting_areas: Vec<String>,
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

impl SignalColor {
    /// Short stable label for inspectors and traces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Yellow => "yellow",
            Self::Green => "green",
        }
    }
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

/// One pedestrian route's relative share of a pedestrian demand source's
/// arrivals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PedestrianRouteShareSource {
    /// Pedestrian route a generated pedestrian follows.
    pub route: String,
    /// Relative weight; a larger weight is chosen proportionally more often.
    pub weight: f64,
}

/// One pedestrian demand generator: pedestrian arrivals at an entry portal and
/// the routes those pedestrians take.
///
/// This mirrors [`DemandSource`] for the pedestrian mode and shares its shape,
/// so a layout can generate both modes from the same authored primitives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PedestrianDemandSource {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Entry portal where generated pedestrians enter the world.
    pub portal: String,
    /// Mean arrival rate in pedestrians per hour.
    pub rate_pph: f64,
    /// Pedestrian routes a generated pedestrian may follow, with relative
    /// weights.
    pub routes: Vec<PedestrianRouteShareSource>,
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
/// Every generated vehicle samples one value from each physical/longitudinal
/// range from the `profile` random stream, so its body and longitudinal
/// behavior are stable for the run. The `compliance` propensity is sampled
/// from the separate `compliance` stream. Pedestrian bodies and gait are the
/// separate [`PedestrianProfileSource`] distributions.
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
    /// Signal-compliance propensity, a fraction of the driver's comfortable
    /// braking they are willing to use to obey a stop-required head.
    ///
    /// `1.0` obeys whenever a comfortable stop is possible and `0.0` never
    /// yields to the head. One value per vehicle is drawn from the `compliance`
    /// random stream. Additive schema version 1 field: omitted means the fully
    /// compliant default.
    #[serde(default = "default_compliance")]
    pub compliance: ProfileRangeSource,
}

/// Default compliance range: every driver obeys whenever a comfortable stop is
/// possible. Noncompliance is opted into by an authored range.
fn default_compliance() -> ProfileRangeSource {
    ProfileRangeSource { min: 1.0, max: 1.0 }
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
            compliance: default_compliance(),
        }
    }
}

/// Pedestrian physical and behavior profile distributions.
///
/// Every generated pedestrian samples one value from each range from the
/// `profile` random stream using its stable agent id, so its body and gait are
/// stable for the run. Its signal-compliance propensity is sampled from the
/// separate `compliance` stream under the same agent id. A pedestrian body is a
/// circle, so the physical range is a radius rather than a length and width.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PedestrianProfileSource {
    /// Body radius in metres.
    pub radius_m: ProfileRangeSource,
    /// Desired walking speed in metres per second.
    pub speed_mps: ProfileRangeSource,
    /// Signal-compliance propensity, a fraction of the pedestrian's bounded
    /// stopping deceleration they are willing to use to wait at a crossing.
    ///
    /// `1.0` waits whenever a bounded stop is possible and `0.0` never waits.
    /// One value per pedestrian is drawn from the `compliance` random stream.
    /// Additive schema version 1 field: omitted means the fully compliant
    /// default.
    #[serde(default = "default_compliance")]
    pub compliance: ProfileRangeSource,
}

impl Default for PedestrianProfileSource {
    fn default() -> Self {
        // Provisional engineering defaults, not calibrated scientific claims.
        Self {
            radius_m: ProfileRangeSource {
                min: 0.20,
                max: 0.30,
            },
            speed_mps: ProfileRangeSource { min: 1.0, max: 1.6 },
            compliance: default_compliance(),
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

/// Nominal traversal direction of a version-2 movement along its reference path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MovementDirection {
    /// The movement follows the authored vertex order of its path.
    Forward,
    /// The movement follows the reverse of the authored vertex order.
    Reverse,
}

/// A version-2 movement connector with an explicit nominal direction.
///
/// Every version-1 movement field is carried forward unchanged; version 2 adds
/// `direction` so the normalized document is self-describing instead of leaving
/// the direction implicit in the `from`/`to` portal order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovementSourceV2 {
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
    /// Stop-line arc length in metres from the movement entry.
    #[serde(default)]
    pub stop_line_m: f64,
    /// Nominal traversal direction of `path`.
    pub direction: MovementDirection,
}

/// Body geometry distributions of one version-2 mode template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModeBodySource {
    /// A rectangular body; dimensions are ranges in metres.
    Box {
        /// Body length range in metres.
        length_m: ProfileRangeSource,
        /// Body width range in metres.
        width_m: ProfileRangeSource,
    },
    /// A circular body; the radius is a range in metres.
    Circle {
        /// Body radius range in metres.
        radius_m: ProfileRangeSource,
    },
    /// A capsule body: a segment of `length_m` with a constant `radius_m`.
    ///
    /// Additive Increment 1 kind for the narrow wheeled family; it compiles to
    /// the existing `AgentBody::Capsule` and the `wheeled_capsule` family.
    Capsule {
        /// Body length range in metres.
        length_m: ProfileRangeSource,
        /// Body radius range in metres.
        radius_m: ProfileRangeSource,
    },
}

/// Motion family a version-2 mode template uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MotionKind {
    /// Walking on the Phase 1 pedestrian geometry.
    HolonomicWalking,
    /// A single wheeled body following a reference path.
    SingleBodyWheeled,
}

/// Tactical capability a version-2 mode template supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TacticKind {
    /// Follow the agent ahead.
    Follow,
    /// Stop for a control or an obstruction.
    Stop,
    /// Yield to a conflicting movement.
    Yield,
}

/// Traversable object kind a version-2 mode may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FacilityKind {
    /// A guide path.
    Path,
    /// A pedestrian crossing.
    Crossing,
    /// A pedestrian waiting area.
    WaitingArea,
    /// A continuous-width facility: a region with an optional reference path.
    /// Additive Increment 1 kind, so an Increment 0 template keeps its meaning.
    Facility,
}

/// Direction a mode may travel along a facility reference path, relative to
/// the authored vertex order of that path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FacilityDirection {
    /// Only the authored forward direction.
    Forward,
    /// Only the reverse of the authored direction.
    Reverse,
    /// Either direction.
    Either,
}

/// Whether a facility's usable lateral interval is one shared space or a
/// centered lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LateralUse {
    /// Agents may occupy any position within the usable lateral interval.
    Shared,
    /// Agents hold the reference centerline.
    Centered,
}

/// An enforced speed limit: a finite positive value, or an explicit `null` for
/// no enforced limit beyond the mode's own motion limits.
///
/// The value is a required, nullable field, so an absent `limit_mps` is a parse
/// error rather than a silent default; a present `null` is the explicit "no
/// limit" form and mirrors the unbounded `TimeIntervalSource.end_s` precedent.
/// Deserialization is written by hand so a missing field is rejected: serde's
/// `Option`-derived newtype would otherwise treat absence as `null`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct SpeedLimitMps(Option<f64>);

impl SpeedLimitMps {
    /// No enforced limit (the explicit `null` form).
    pub const fn unlimited() -> Self {
        Self(None)
    }

    /// An enforced limit in metres per second.
    pub const fn limited(limit_mps: f64) -> Self {
        Self(Some(limit_mps))
    }

    /// The limit in metres per second, or `None` for an explicit `null`.
    pub const fn value(self) -> Option<f64> {
        self.0
    }
}

impl<'de> Deserialize<'de> for SpeedLimitMps {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SpeedLimitVisitor;

        impl<'de> serde::de::Visitor<'de> for SpeedLimitVisitor {
            type Value = SpeedLimitMps;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a metres-per-second limit or null")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(SpeedLimitMps(None))
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(SpeedLimitMps(None))
            }

            fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
            where
                D2: serde::Deserializer<'de>,
            {
                Ok(SpeedLimitMps(Some(<f64 as Deserialize>::deserialize(
                    deserializer,
                )?)))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(SpeedLimitMps(Some(value)))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(SpeedLimitMps(Some(value as f64)))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(SpeedLimitMps(Some(value as f64)))
            }
        }

        deserializer.deserialize_any(SpeedLimitVisitor)
    }
}

/// A speed policy: a finite positive limit, or none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpeedPolicySource {
    /// Enforced limit in metres per second; `null` means no enforced limit.
    pub limit_mps: SpeedLimitMps,
}

/// Facility access of a version-2 mode template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessSource {
    /// Traversable object kinds the mode may use.
    pub facility_kinds: Vec<FacilityKind>,
    /// Direction the mode may travel on its facilities.
    ///
    /// Omitted means `either`, the value Increment 0 fixed for the compiled
    /// access direction because nothing authored it. Additive Increment 1
    /// field: an Increment 0 template omits it and keeps that meaning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal_direction: Option<FacilityDirection>,
    /// Speed policy the mode travels under.
    ///
    /// Omitted means no enforced limit, the value Increment 0 fixed for the
    /// compiled access. Additive Increment 1 field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_policy: Option<SpeedPolicySource>,
}

/// Mode-template access of a version-2 facility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FacilityAccessSource {
    /// Mode-template ids permitted to use the facility; non-empty.
    pub modes: Vec<String>,
}

/// One version-2 continuous-width facility: a traversable region plus an
/// optional reference path, usable width, nominal direction, mode access,
/// lateral-use policy, and speed policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FacilitySource {
    /// Stable facility id, unique across all authored objects.
    pub id: String,
    /// Region the facility occupies.
    pub region: String,
    /// Guide path giving the facility its `(s, d)` frame; omitted means the
    /// facility exposes region geometry only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_path: Option<String>,
    /// Usable traversable width in metres, measured across the reference path.
    pub width_m: f64,
    /// Nominal direction relative to the authored vertex order of
    /// `reference_path`.
    pub nominal_direction: FacilityDirection,
    /// Mode templates permitted to use the facility.
    pub access: FacilityAccessSource,
    /// Whether the usable lateral interval is shared or centered.
    pub lateral_use: LateralUse,
    /// Speed policy on the facility; a missing field is a parse error.
    pub speed_policy: SpeedPolicySource,
}

/// One end of a version-2 facility connector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FacilityConnectorEndSource {
    /// Facility the connector joins.
    pub facility: String,
    /// Traversal direction along that facility: `forward` or `reverse`.
    pub direction: MovementDirection,
}

/// A version-2 directed connector joining two facility traversals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FacilityConnectorSource {
    /// Stable connector id, unique across all authored objects.
    pub id: String,
    /// Facility and traversal direction the connector leaves.
    pub from: FacilityConnectorEndSource,
    /// Facility and traversal direction the connector enters.
    pub to: FacilityConnectorEndSource,
}

/// What an authored permission or obligation statement is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    /// Travel against a facility's or movement's nominal direction.
    NominalDirection,
    /// Permitted lateral position or lane use on a facility.
    LaneUse,
    /// Overtaking or passing on a facility.
    Overtake,
    /// Permission or obligation at a crossing.
    Crossing,
    /// Stop-service obligation and priority.
    StopService,
}

/// Whether a permission statement permits, prohibits, or obligates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionEffect {
    /// The holder may do the statement's action.
    Permit,
    /// The holder must not do it.
    Prohibit,
    /// The holder must do it.
    Obligate,
}

/// One authored permission or obligation statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PermissionSource {
    /// Stable id, unique across all authored objects.
    pub id: String,
    /// Kind of statement; it fixes the target object kind.
    pub kind: PermissionKind,
    /// Mode-template id the statement binds.
    pub holder: String,
    /// Id of the object the statement is about.
    pub target: String,
    /// Whether the statement permits, prohibits, or obligates.
    pub effect: PermissionEffect,
}

/// Occupancy of a version-2 mode template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OccupancyKind {
    /// One operator, no passengers.
    OperatorOnly,
}

/// One version-2 mode template: a named, validated bundle of body, motion,
/// tactical capability, access, occupancy, and profile distributions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModeTemplateSource {
    /// Stable template id, unique across authored objects.
    pub id: String,
    /// Body geometry distributions, tagged by `kind`.
    pub body: ModeBodySource,
    /// Motion family the template uses.
    pub motion: MotionKind,
    /// Tactical capabilities the template supports.
    pub tactics: Vec<TacticKind>,
    /// Traversable object kinds the mode may use.
    pub access: AccessSource,
    /// Occupancy the template carries.
    pub occupancy: OccupancyKind,
    /// Profile distributions keyed by parameter name.
    pub profiles: BTreeMap<String, ProfileRangeSource>,
}

/// The time interval of a version-2 rate demand spawn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeIntervalSource {
    /// Interval start in seconds.
    pub start_s: f64,
    /// Interval end in seconds; `null` is unbounded.
    pub end_s: Option<f64>,
}

/// Choice of movements or routes for a version-2 rate demand spawn.
///
/// Vehicle modes choose among movements; pedestrian modes choose among
/// pedestrian routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DemandChoiceSource {
    /// Movements a generated vehicle may follow, with relative weights.
    Movements(Vec<RouteShareSource>),
    /// Pedestrian routes a generated pedestrian may follow, with weights.
    Routes(Vec<PedestrianRouteShareSource>),
}

/// A rate-driven version-2 demand spawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DemandRateSpawnSource {
    /// Entry portal where generated agents enter the world.
    pub portal: String,
    /// Mean arrival rate in agents per hour.
    pub rate_per_hour: f64,
    /// Interval over which the rate applies.
    pub interval_s: TimeIntervalSource,
    /// Movements or routes a generated agent may follow.
    pub choice: DemandChoiceSource,
}

/// A fixed initial-population version-2 demand spawn, placed at `t = 0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DemandPopulationSpawnSource {
    /// Guide path the population is placed on.
    pub path: String,
    /// Number of agents in the population.
    pub count: u32,
    /// Constant speed in metres per second.
    pub speed_mps: f64,
    /// Longitudinal gap between adjacent agents in metres.
    pub spacing_m: f64,
}

/// How a version-2 demand source places agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DemandSpawnSource {
    /// Arrivals at a portal over a time interval.
    Rate(DemandRateSpawnSource),
    /// A fixed population placed at `t = 0`.
    Population(DemandPopulationSpawnSource),
}

/// One mode-tagged version-2 demand source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DemandSourceV2 {
    /// Stable identifier, unique across all authored objects.
    pub id: String,
    /// Mode-template id this demand produces.
    pub mode: String,
    /// How agents are placed.
    pub spawn: DemandSpawnSource,
}

/// A version-2 scenario document.
///
/// Version 2 replaces the version-1 `profiles`/`pedestrian_profiles`,
/// `population`, `demand`, and `pedestrian_demand` fields with `mode_templates`
/// and a mode-tagged `demand`, and makes each movement's `direction` explicit.
/// The Increment 1 additive arrays `facilities`, `facility_connectors`, and
/// `permissions` extend it in place. Every other version-1 field is carried
/// forward unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSourceV2 {
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
    /// Continuous-width facilities with usable width, nominal direction, mode
    /// access, lateral-use policy, and speed policy. Additive Increment 1
    /// array; omitted or empty means no facilities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facilities: Vec<FacilitySource>,
    /// Directed connectors joining two facility traversals. Additive
    /// Increment 1 array.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facility_connectors: Vec<FacilityConnectorSource>,
    /// Movement connectors with an explicit nominal direction.
    #[serde(default)]
    pub movements: Vec<MovementSourceV2>,
    /// Pedestrian crossings over one or more movements.
    #[serde(default)]
    pub crossings: Vec<CrossingSource>,
    /// Named waiting areas where pedestrians stage before crossing.
    #[serde(default)]
    pub waiting_areas: Vec<WaitingAreaSource>,
    /// Pedestrian routes from one portal to another along a guide path.
    #[serde(default)]
    pub pedestrian_routes: Vec<PedestrianRouteSource>,
    /// Authored conflict regions shared by pairs of movements.
    #[serde(default)]
    pub conflict_regions: Vec<ConflictRegionSource>,
    /// Right-of-way or control rules attached to movements.
    #[serde(default)]
    pub rules: Vec<RuleSource>,
    /// Fixed-time signal controllers with phased signal heads.
    #[serde(default)]
    pub signals: Vec<SignalSource>,
    /// Authored permission and obligation statements. Additive Increment 1
    /// array; the shape is fixed in full, while only `nominal_direction`
    /// statements are populated in Increment 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PermissionSource>,
    /// Named, validated mode bundles; demand references them by id.
    #[serde(default)]
    pub mode_templates: Vec<ModeTemplateSource>,
    /// Mode-tagged demand sources for every mode.
    #[serde(default)]
    pub demand: Vec<DemandSourceV2>,
}

/// Failure to read a [`ScenarioSource`] from text.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The document was not well-formed JSON5 or did not match the schema.
    #[error("scenario source is not valid JSON5 for the source schema: {0}")]
    Json5(#[from] serde_json5::Error),
}

/// A parsed scenario document, tagged by its negotiated schema version.
#[derive(Debug, Clone, PartialEq)]
pub enum ScenarioDocument {
    /// A version-1 document, read directly until the migration replaces this path.
    V1(ScenarioSource),
    /// A version-2 document.
    V2(ScenarioSourceV2),
}

/// Failure to read a scenario document at any supported schema version.
#[derive(Debug, thiserror::Error)]
pub enum DocumentReadError {
    /// The document did not match the shape of its declared schema version.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// The document names a schema version this build cannot read.
    #[error(
        "schema_version {version} is not supported; this build reads versions \
         {min} through {max}",
        min = MIN_SUPPORTED_SCHEMA_VERSION,
        max = SUPPORTED_SCHEMA_VERSION
    )]
    UnsupportedVersion {
        /// The version the document declared.
        version: u32,
    },
}

impl DocumentReadError {
    /// The stable `E_SCHEMA_VERSION` diagnostic for an unsupported version.
    ///
    /// Returns `None` for a structural parse failure, which carries no schema
    /// version to report.
    pub fn diagnostic(&self) -> Option<crate::validate::Diagnostic> {
        match self {
            Self::UnsupportedVersion { version } => Some(crate::validate::Diagnostic {
                code: crate::validate::DiagnosticCode::UnsupportedSchemaVersion,
                object: None,
                message: format!(
                    "schema_version {version} is not supported; this build reads versions \
                     {MIN_SUPPORTED_SCHEMA_VERSION} through {SUPPORTED_SCHEMA_VERSION}"
                ),
            }),
            Self::Parse(_) => None,
        }
    }
}

/// Read the declared schema version of a JSON5 scenario document.
#[derive(Deserialize)]
struct SchemaVersionProbe {
    schema_version: u32,
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

/// Parse a JSON5 version-2 scenario document into its source representation.
///
/// This performs structural parsing only; a missing version-2 field is a parse
/// error rather than a fallback to a version-1 default. Semantic validation is
/// the separate [`crate::validate::validate_v2`] step.
pub fn parse_scenario_source_v2(input: &str) -> Result<ScenarioSourceV2, ParseError> {
    let source: ScenarioSourceV2 = serde_json5::from_str(input)?;
    Ok(source)
}

/// Parse a JSON5 scenario document, negotiating its declared schema version.
///
/// Version 1 and version 2 are accepted; any other version is rejected as
/// [`DocumentReadError::UnsupportedVersion`], whose [`DocumentReadError::diagnostic`]
/// is the stable `E_SCHEMA_VERSION` diagnostic. The reader never guesses a
/// version and never applies a default for an absent field.
pub fn parse_scenario_document(input: &str) -> Result<ScenarioDocument, DocumentReadError> {
    let probe: SchemaVersionProbe = serde_json5::from_str(input).map_err(ParseError::from)?;
    match probe.schema_version {
        MIN_SUPPORTED_SCHEMA_VERSION => Ok(ScenarioDocument::V1(parse_scenario_source(input)?)),
        SUPPORTED_SCHEMA_VERSION => Ok(ScenarioDocument::V2(parse_scenario_source_v2(input)?)),
        version => Err(DocumentReadError::UnsupportedVersion { version }),
    }
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
        assert_eq!(source.schema_version, MIN_SUPPORTED_SCHEMA_VERSION);
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
        assert!(source.pedestrian_demand.is_empty());
        assert!(source.pedestrian_routes.is_empty());
        assert!(source.waiting_areas.is_empty());
        assert_eq!(
            source.pedestrian_profiles,
            PedestrianProfileSource::default()
        );
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
        // The additive compliance field defaults to fully compliant.
        assert_eq!(source.profiles.compliance, default_compliance());
    }

    #[test]
    fn parses_an_authored_compliance_range() {
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
                compliance: { min: 0.2, max: 0.9 },
            },
        }"#;
        let source = parse_scenario_source(input).expect("parses");
        assert_eq!(source.profiles.compliance.min, 0.2);
        assert_eq!(source.profiles.compliance.max, 0.9);
    }

    #[test]
    fn parses_pedestrian_waiting_areas_routes_demand_and_profiles() {
        let input = r#"{
            schema_version: 1, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [ { id: 'walk', points: [ { x: 0, y: 0 }, { x: 20, y: 0 } ] } ],
            portals: [ { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
                       { id: 'north', path: 'walk', end: 'end', width_m: 2.0 } ],
            regions: [ { id: 'corner', points: [
                { x: 0, y: 0 }, { x: 3, y: 0 }, { x: 3, y: 3 }, { x: 0, y: 3 }
            ] } ],
            crossings: [ { id: 'cross', region: 'corner', movements: [ 'through' ] } ],
            waiting_areas: [ { id: 'south_wait', region: 'corner' } ],
            pedestrian_routes: [ { id: 'crossing_route', from: 'south', to: 'north',
                path: 'walk', crossings: [ 'cross' ], waiting_areas: [ 'south_wait' ] } ],
            pedestrian_demand: [ { id: 'footfall', portal: 'south', rate_pph: 240.0,
                routes: [ { route: 'crossing_route', weight: 1.0 } ] } ],
            pedestrian_profiles: {
                radius_m: { min: 0.2, max: 0.3 },
                speed_mps: { min: 1.1, max: 1.5 },
            },
        }"#;
        let source = parse_scenario_source(input).expect("parses");
        assert_eq!(source.waiting_areas.len(), 1);
        assert_eq!(source.waiting_areas[0].region, "corner");
        assert_eq!(source.pedestrian_routes.len(), 1);
        assert_eq!(source.pedestrian_routes[0].from, "south");
        assert_eq!(source.pedestrian_routes[0].crossings, ["cross"]);
        assert_eq!(source.pedestrian_routes[0].waiting_areas, ["south_wait"]);
        assert_eq!(source.pedestrian_demand.len(), 1);
        assert_eq!(source.pedestrian_demand[0].rate_pph, 240.0);
        assert_eq!(
            source.pedestrian_demand[0].routes[0].route,
            "crossing_route"
        );
        assert_eq!(source.pedestrian_profiles.speed_mps.max, 1.5);
    }

    #[test]
    fn parses_a_pedestrian_signal_and_compliance_range() {
        let input = r#"{
            schema_version: 1, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [ { id: 'walk', points: [ { x: 0, y: 0 }, { x: 20, y: 0 } ] } ],
            portals: [ { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
                       { id: 'north', path: 'walk', end: 'end', width_m: 2.0 } ],
            regions: [ { id: 'corner', points: [
                { x: 0, y: 0 }, { x: 3, y: 0 }, { x: 3, y: 3 }, { x: 0, y: 3 }
            ] } ],
            crossings: [ { id: 'cross', region: 'corner', movements: [ 'through' ],
                pedestrian_signal: { phases: [
                    { duration_s: 20.0, walk: true },
                    { duration_s: 20.0, walk: false },
                ] } } ],
            pedestrian_profiles: {
                radius_m: { min: 0.2, max: 0.3 },
                speed_mps: { min: 1.1, max: 1.5 },
                compliance: { min: 0.4, max: 0.9 },
            },
        }"#;
        let source = parse_scenario_source(input).expect("parses");
        let signal = source.crossings[0]
            .pedestrian_signal
            .as_ref()
            .expect("pedestrian signal parses");
        assert_eq!(signal.phases.len(), 2);
        assert!(signal.phases[0].walk);
        assert_eq!(signal.phases[0].duration_s, 20.0);
        assert!(!signal.phases[1].walk);
        assert_eq!(source.pedestrian_profiles.compliance.min, 0.4);
        assert_eq!(source.pedestrian_profiles.compliance.max, 0.9);
    }

    #[test]
    fn an_omitted_pedestrian_signal_is_uncontrolled_and_compliance_defaults() {
        let input = r#"{
            schema_version: 1, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [ { id: 'walk', points: [ { x: 0, y: 0 }, { x: 20, y: 0 } ] } ],
            portals: [ { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
                       { id: 'north', path: 'walk', end: 'end', width_m: 2.0 } ],
            regions: [ { id: 'corner', points: [
                { x: 0, y: 0 }, { x: 3, y: 0 }, { x: 3, y: 3 }, { x: 0, y: 3 }
            ] } ],
            crossings: [ { id: 'cross', region: 'corner', movements: [ 'through' ] } ],
        }"#;
        let source = parse_scenario_source(input).expect("parses");
        assert!(source.crossings[0].pedestrian_signal.is_none());
        assert_eq!(
            source.pedestrian_profiles.compliance,
            default_compliance(),
            "an omitted pedestrian compliance range defaults to fully compliant"
        );
    }

    const FACILITY_V2: &str = r#"{
        schema_version: 2, id: 'bikeway', coordinate_system: { x: 'east_m', y: 'north_m' },
        paths: [ { id: 'bikeway_centerline',
            points: [ { x: 0.0, y: 0.0 }, { x: 80.0, y: 0.0 } ] } ],
        portals: [],
        regions: [ { id: 'bikeway_band', points: [
            { x: 0.0, y: -1.0 }, { x: 80.0, y: -1.0 },
            { x: 80.0, y: 1.0 }, { x: 0.0, y: 1.0 } ] } ],
        mode_templates: [ {
            id: 'bicycle',
            body: { kind: 'box', length_m: { min: 1.6, max: 1.9 },
                width_m: { min: 0.6, max: 0.8 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
                speed_policy: { limit_mps: null } },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 3.5, max: 6.5 },
                max_accel_mps2: { min: 0.8, max: 1.5 },
                comfortable_brake_mps2: { min: 1.5, max: 3.0 },
                time_gap_s: { min: 0.8, max: 1.4 },
                compliance: { min: 0.8, max: 1.0 },
            },
        } ],
        facilities: [ {
            id: 'bikeway_eastbound', region: 'bikeway_band',
            reference_path: 'bikeway_centerline', width_m: 2.0,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null },
        } ],
        facility_connectors: [],
        permissions: [ {
            id: 'bicycle_nominal_northbound', kind: 'nominal_direction',
            holder: 'bicycle', target: 'bikeway_eastbound', effect: 'obligate',
        } ],
    }"#;

    #[test]
    fn parses_facilities_connectors_access_and_permissions() {
        let source = parse_scenario_source_v2(FACILITY_V2).expect("facility document parses");
        assert_eq!(source.facilities.len(), 1);
        let facility = &source.facilities[0];
        assert_eq!(facility.region, "bikeway_band");
        assert_eq!(
            facility.reference_path.as_deref(),
            Some("bikeway_centerline")
        );
        assert_eq!(facility.nominal_direction, FacilityDirection::Forward);
        assert_eq!(facility.lateral_use, LateralUse::Shared);
        assert_eq!(facility.access.modes, ["bicycle"]);
        assert_eq!(facility.speed_policy.limit_mps.value(), None);

        assert!(
            source.mode_templates[0]
                .access
                .facility_kinds
                .contains(&FacilityKind::Facility)
        );
        assert_eq!(
            source.mode_templates[0].access.nominal_direction,
            Some(FacilityDirection::Either)
        );
        assert_eq!(
            source.mode_templates[0].access.speed_policy,
            Some(SpeedPolicySource {
                limit_mps: SpeedLimitMps::unlimited()
            })
        );

        assert_eq!(source.permissions.len(), 1);
        assert_eq!(source.permissions[0].kind, PermissionKind::NominalDirection);
        assert_eq!(source.permissions[0].effect, PermissionEffect::Obligate);
        assert_eq!(source.permissions[0].target, "bikeway_eastbound");
    }

    #[test]
    fn an_increment_0_access_omits_the_additive_direction_and_policy_fields() {
        // Absent means the Increment 0 meaning: either direction, no enforced
        // limit. The field set stays additive so an Increment 0 template is
        // still valid version 2.
        let input = r#"{
            schema_version: 2, id: 'x', coordinate_system: { x: 'a', y: 'b' },
            paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 20, y: 0 } ] } ],
            portals: [],
            mode_templates: [ {
                id: 'passenger_car',
                body: { kind: 'box', length_m: { min: 4.0, max: 5.2 },
                    width_m: { min: 1.7, max: 2.0 } },
                motion: 'single_body_wheeled',
                tactics: [ 'follow' ],
                access: { facility_kinds: [ 'path' ] },
                occupancy: 'operator_only',
                profiles: {
                    speed_mps: { min: 9.0, max: 15.0 },
                    max_accel_mps2: { min: 1.2, max: 2.5 },
                    comfortable_brake_mps2: { min: 2.0, max: 3.5 },
                    time_gap_s: { min: 1.0, max: 2.0 },
                    compliance: { min: 1.0, max: 1.0 },
                },
            } ],
        }"#;
        let source = parse_scenario_source_v2(input).expect("Increment 0 access parses");
        assert!(source.mode_templates[0].access.nominal_direction.is_none());
        assert!(source.mode_templates[0].access.speed_policy.is_none());
        assert!(source.facilities.is_empty());
        assert!(source.facility_connectors.is_empty());
        assert!(source.permissions.is_empty());
    }
}
