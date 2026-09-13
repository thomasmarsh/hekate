//! Immutable compiled scenario and dense identifiers.
//!
//! Compilation is the boundary between the authored document and the hot
//! kernel. Authored objects keep stable string identifiers everywhere the
//! document is edited, viewed, or reported; the kernel indexes dense integer
//! arrays instead. The string-to-integer mapping is preserved in
//! [`IdMap`] so run provenance can name any identifier the kernel carries.

use std::collections::HashMap;

use glam::DVec2;

use crate::components::{NominalDirection, SpeedPolicy};
use crate::mode_template::{
    CompiledModeTemplate, compile_mode_template, compiled_nominal_direction, compiled_speed_policy,
};
use crate::source::{
    DemandChoiceSource, DemandSource, DemandSpawnSource, LateralUse, ModeBodySource,
    ModeTemplateSource, MovementDirection, MovementSource, PathEnd, PedestrianDemandSource,
    PedestrianProfileSource, PopulationSource, ProfileRangeSource, ProfileSource, RuleKind,
    ScenarioSource, ScenarioSourceV2, SignalColor, SignalSource,
};
use crate::validate::{Diagnostic, validate, validate_v2};

/// Dense index of a compiled guide path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PathId(u32);

impl PathId {
    /// Construct a dense path identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this path.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PortalId(u32);

impl PortalId {
    /// Construct a dense portal identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this portal.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled boundary polygon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoundaryId(u32);

impl BoundaryId {
    /// Construct a dense boundary identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this boundary.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled traversable region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionId(u32);

impl RegionId {
    /// Construct a dense region identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this region.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled movement connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MovementId(u32);

impl MovementId {
    /// Construct a dense movement identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this movement.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled pedestrian crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CrossingId(u32);

impl CrossingId {
    /// Construct a dense crossing identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this crossing.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled conflict region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConflictRegionId(u32);

impl ConflictRegionId {
    /// Construct a dense conflict-region identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this conflict region.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled control rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleId(u32);

impl RuleId {
    /// Construct a dense rule identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this rule.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled fixed-time signal controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SignalId(u32);

impl SignalId {
    /// Construct a dense signal identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this signal.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled demand source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DemandId(u32);

impl DemandId {
    /// Construct a dense demand identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this demand source.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled waiting area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WaitingAreaId(u32);

impl WaitingAreaId {
    /// Construct a dense waiting-area identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this waiting area.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled pedestrian route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PedestrianRouteId(u32);

impl PedestrianRouteId {
    /// Construct a dense pedestrian-route identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this route.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled pedestrian demand source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PedestrianDemandId(u32);

impl PedestrianDemandId {
    /// Construct a dense pedestrian-demand identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this demand source.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled mode template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModeTemplateId(u32);

impl ModeTemplateId {
    /// Construct a dense mode-template identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this template.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled continuous-width facility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FacilityId(u32);

impl FacilityId {
    /// Construct a dense facility identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this facility.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Dense index of a compiled facility connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FacilityConnectorId(u32);

impl FacilityConnectorId {
    /// Construct a dense connector identifier from its array index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based array index of this connector.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Stable string identifiers in dense-index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMap {
    paths: Vec<String>,
    portals: Vec<String>,
    boundaries: Vec<String>,
    regions: Vec<String>,
    movements: Vec<String>,
    crossings: Vec<String>,
    waiting_areas: Vec<String>,
    pedestrian_routes: Vec<String>,
    conflict_regions: Vec<String>,
    rules: Vec<String>,
    signals: Vec<String>,
    demand: Vec<String>,
    pedestrian_demand: Vec<String>,
    facilities: Vec<String>,
    facility_connectors: Vec<String>,
}

impl IdMap {
    /// Path identifiers indexed by [`PathId`].
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Portal identifiers indexed by [`PortalId`].
    pub fn portals(&self) -> &[String] {
        &self.portals
    }

    /// Boundary identifiers indexed by [`BoundaryId`].
    pub fn boundaries(&self) -> &[String] {
        &self.boundaries
    }

    /// Region identifiers indexed by [`RegionId`].
    pub fn regions(&self) -> &[String] {
        &self.regions
    }

    /// Movement identifiers indexed by [`MovementId`].
    pub fn movements(&self) -> &[String] {
        &self.movements
    }

    /// Crossing identifiers indexed by [`CrossingId`].
    pub fn crossings(&self) -> &[String] {
        &self.crossings
    }

    /// Waiting-area identifiers indexed by [`WaitingAreaId`].
    pub fn waiting_areas(&self) -> &[String] {
        &self.waiting_areas
    }

    /// Pedestrian-route identifiers indexed by [`PedestrianRouteId`].
    pub fn pedestrian_routes(&self) -> &[String] {
        &self.pedestrian_routes
    }

    /// Conflict-region identifiers indexed by [`ConflictRegionId`].
    pub fn conflict_regions(&self) -> &[String] {
        &self.conflict_regions
    }

    /// Rule identifiers indexed by [`RuleId`].
    pub fn rules(&self) -> &[String] {
        &self.rules
    }

    /// Signal identifiers indexed by [`SignalId`].
    pub fn signals(&self) -> &[String] {
        &self.signals
    }

    /// Demand-source identifiers indexed by [`DemandId`].
    pub fn demand(&self) -> &[String] {
        &self.demand
    }

    /// Pedestrian-demand identifiers indexed by [`PedestrianDemandId`].
    pub fn pedestrian_demand(&self) -> &[String] {
        &self.pedestrian_demand
    }

    /// Facility identifiers indexed by [`FacilityId`].
    pub fn facilities(&self) -> &[String] {
        &self.facilities
    }

    /// Facility-connector identifiers indexed by [`FacilityConnectorId`].
    pub fn facility_connectors(&self) -> &[String] {
        &self.facility_connectors
    }

    /// Look up the authored name of a compiled path.
    pub fn path_name(&self, id: PathId) -> Option<&str> {
        lookup(&self.paths, id.index())
    }

    /// Look up the authored name of a compiled portal.
    pub fn portal_name(&self, id: PortalId) -> Option<&str> {
        lookup(&self.portals, id.index())
    }

    /// Look up the authored name of a compiled boundary.
    pub fn boundary_name(&self, id: BoundaryId) -> Option<&str> {
        lookup(&self.boundaries, id.index())
    }

    /// Look up the authored name of a compiled region.
    pub fn region_name(&self, id: RegionId) -> Option<&str> {
        lookup(&self.regions, id.index())
    }

    /// Look up the authored name of a compiled movement.
    pub fn movement_name(&self, id: MovementId) -> Option<&str> {
        lookup(&self.movements, id.index())
    }

    /// Look up the authored name of a compiled crossing.
    pub fn crossing_name(&self, id: CrossingId) -> Option<&str> {
        lookup(&self.crossings, id.index())
    }

    /// Look up the authored name of a compiled waiting area.
    pub fn waiting_area_name(&self, id: WaitingAreaId) -> Option<&str> {
        lookup(&self.waiting_areas, id.index())
    }

    /// Look up the authored name of a compiled pedestrian route.
    pub fn pedestrian_route_name(&self, id: PedestrianRouteId) -> Option<&str> {
        lookup(&self.pedestrian_routes, id.index())
    }

    /// Look up the authored name of a compiled conflict region.
    pub fn conflict_region_name(&self, id: ConflictRegionId) -> Option<&str> {
        lookup(&self.conflict_regions, id.index())
    }

    /// Look up the authored name of a compiled rule.
    pub fn rule_name(&self, id: RuleId) -> Option<&str> {
        lookup(&self.rules, id.index())
    }

    /// Look up the authored name of a compiled signal.
    pub fn signal_name(&self, id: SignalId) -> Option<&str> {
        lookup(&self.signals, id.index())
    }

    /// Look up the authored name of a compiled demand source.
    pub fn demand_name(&self, id: DemandId) -> Option<&str> {
        lookup(&self.demand, id.index())
    }

    /// Look up the authored name of a compiled pedestrian demand source.
    pub fn pedestrian_demand_name(&self, id: PedestrianDemandId) -> Option<&str> {
        lookup(&self.pedestrian_demand, id.index())
    }

    /// Look up the authored name of a compiled facility.
    pub fn facility_name(&self, id: FacilityId) -> Option<&str> {
        lookup(&self.facilities, id.index())
    }

    /// Look up the authored name of a compiled facility connector.
    pub fn facility_connector_name(&self, id: FacilityConnectorId) -> Option<&str> {
        lookup(&self.facility_connectors, id.index())
    }
}

/// Look up one name by dense index.
fn lookup(names: &[String], index: usize) -> Option<&str> {
    names.get(index).map(String::as_str)
}

/// A validated guide path with cached arc-length parameterization.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPath {
    id: PathId,
    name: String,
    points: Vec<DVec2>,
    cumulative: Vec<f64>,
    length: f64,
}

impl CompiledPath {
    /// Dense identifier of this path.
    pub fn id(&self) -> PathId {
        self.id
    }

    /// Authored name of this path.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Polyline vertices in order.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }

    /// Total traversable length in metres.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Position at an arc length, clamped to the path extent.
    pub fn position_at(&self, distance: f64) -> DVec2 {
        let distance = distance.clamp(0.0, self.length);
        let segment = self.segment_index(distance);
        let start = self.points[segment];
        let end = self.points[segment + 1];
        let segment_start = self.cumulative[segment];
        let segment_length = self.cumulative[segment + 1] - segment_start;
        if segment_length <= 0.0 {
            return start;
        }
        let t = (distance - segment_start) / segment_length;
        start.lerp(end, t)
    }

    /// Heading in radians at an arc length, clamped to the path extent.
    pub fn heading_at(&self, distance: f64) -> f64 {
        let distance = distance.clamp(0.0, self.length);
        let segment = self.segment_index(distance);
        let start = self.points[segment];
        let end = self.points[segment + 1];
        let delta = end - start;
        delta.y.atan2(delta.x)
    }

    /// Index of the segment containing `distance`, which must lie in range.
    fn segment_index(&self, distance: f64) -> usize {
        debug_assert!(!self.points.is_empty(), "compiled paths have vertices");
        // `partition_point` finds the first cumulative length strictly greater
        // than `distance`; the segment before it contains the point. For the
        // final point this clamps to the last segment.
        let upper = self
            .cumulative
            .partition_point(|&length| length <= distance);
        upper.saturating_sub(1).min(self.points.len() - 2)
    }
}

/// One segment of a compiled reference path.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ReferenceSegment {
    /// A straight segment from `start` to `end`.
    Line {
        /// Segment start point in metres.
        start: DVec2,
        /// Segment end point in metres.
        end: DVec2,
    },
    /// A circular arc of constant signed curvature, swept from
    /// `start_angle_rad` by `sweep_rad` about `center`.
    Arc {
        /// Circle center in metres.
        center: DVec2,
        /// Circle radius in metres, strictly positive.
        radius: f64,
        /// World angle of the arc's start point in radians.
        start_angle_rad: f64,
        /// Signed sweep in radians; positive is counter-clockwise.
        sweep_rad: f64,
    },
}

impl ReferenceSegment {
    /// Arc length of this segment in metres.
    fn length(self) -> f64 {
        match self {
            Self::Line { start, end } => start.distance(end),
            Self::Arc {
                radius, sweep_rad, ..
            } => radius * sweep_rad.abs(),
        }
    }

    /// Point at local arc length `distance` from the segment start.
    fn position_at(self, distance: f64) -> DVec2 {
        match self {
            Self::Line { start, end } => {
                let length = start.distance(end);
                if length <= 0.0 {
                    return start;
                }
                start.lerp(end, (distance / length).clamp(0.0, 1.0))
            }
            Self::Arc { center, radius, .. } => {
                center
                    + radius
                        * self
                            .angle_at(distance)
                            .map_or(DVec2::ZERO, |angle| DVec2::new(angle.cos(), angle.sin()))
            }
        }
    }

    /// The world angle at local arc length `distance`, or `None` for a
    /// degenerate arc.
    fn angle_at(self, distance: f64) -> Option<f64> {
        let Self::Arc {
            radius,
            start_angle_rad,
            sweep_rad,
            ..
        } = self
        else {
            return None;
        };
        let length = radius * sweep_rad.abs();
        if length <= 0.0 {
            return Some(start_angle_rad);
        }
        Some(start_angle_rad + sweep_rad * (distance / length).clamp(0.0, 1.0))
    }

    /// Tangent heading in radians at local arc length `distance`.
    fn heading_at(self, distance: f64) -> f64 {
        match self {
            Self::Line { start, end } => {
                let delta = end - start;
                delta.y.atan2(delta.x)
            }
            Self::Arc { sweep_rad, .. } => {
                let angle = self.angle_at(distance).unwrap_or(0.0);
                angle + sweep_rad.signum() * std::f64::consts::FRAC_PI_2
            }
        }
    }

    /// Signed curvature in `1/m`, positive for a counter-clockwise turn.
    fn curvature(self) -> f64 {
        match self {
            Self::Line { .. } => 0.0,
            Self::Arc {
                radius, sweep_rad, ..
            } => {
                if radius <= 0.0 {
                    0.0
                } else {
                    sweep_rad.signum() / radius
                }
            }
        }
    }

    /// Nearest point on the segment to `point`, with its local arc length.
    fn nearest(self, point: DVec2) -> (DVec2, f64) {
        match self {
            Self::Line { start, end } => {
                let delta = end - start;
                let length_sq = delta.length_squared();
                let t = if length_sq <= 0.0 {
                    0.0
                } else {
                    ((point - start).dot(delta) / length_sq).clamp(0.0, 1.0)
                };
                (start + delta * t, t * length_sq.max(0.0).sqrt())
            }
            Self::Arc {
                center,
                radius,
                start_angle_rad,
                sweep_rad,
            } => {
                let length = radius * sweep_rad.abs();
                if length <= 0.0 {
                    return (self.position_at(0.0), 0.0);
                }
                // Both endpoints are candidates; the radial projection is a
                // third candidate only when it falls inside the swept arc.
                let mut best = (self.position_at(0.0), 0.0);
                let mut best_distance = best.0.distance_squared(point);
                let end = self.position_at(length);
                let end_distance = end.distance_squared(point);
                if end_distance < best_distance {
                    best = (end, length);
                    best_distance = end_distance;
                }
                let phi = (point - center).y.atan2((point - center).x);
                // Signed angular offset from the arc start, measured along the
                // direction of travel, in `[0, TAU)`.
                let swept = if sweep_rad >= 0.0 {
                    (phi - start_angle_rad).rem_euclid(std::f64::consts::TAU)
                } else {
                    (start_angle_rad - phi).rem_euclid(std::f64::consts::TAU)
                };
                if swept <= sweep_rad.abs() {
                    let local = length * (swept / sweep_rad.abs());
                    let foot = self.position_at(local);
                    let distance = foot.distance_squared(point);
                    if distance < best_distance {
                        best = (foot, local);
                    }
                }
                best
            }
        }
    }
}

/// Route coordinates of a world point: arc length `s` along a reference path
/// and signed lateral offset `d`, positive to the left of the direction of
/// travel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteCoordinate {
    s: f64,
    d: f64,
}

impl RouteCoordinate {
    /// Construct a route coordinate from its arc length and lateral offset.
    pub const fn new(s: f64, d: f64) -> Self {
        Self { s, d }
    }

    /// Arc length along the reference path in metres.
    pub const fn s(self) -> f64 {
        self.s
    }

    /// Signed lateral offset in metres, positive to the left of travel.
    pub const fn d(self) -> f64 {
        self.d
    }
}

/// A compiled reference path: an ordered sequence of straight and circular-arc
/// segments with cached arc-length parameterization.
///
/// An authored `paths[]` polyline compiles to straight segments through
/// [`Self::from_polyline`]. [`Self::arc`] builds the exact constant-curvature
/// reference the analytic `T-RT` fixtures use. The `(s, d) <-> world` mapping is
/// `p(s, d) = position_at(s) + d * normal_at(s)` with `s` clamped to
/// `[0, length]`; the inverse is the nearest point on the reference.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledReferencePath {
    segments: Vec<ReferenceSegment>,
    /// Arc length at the start of each segment, plus the total at the end.
    starts: Vec<f64>,
    length: f64,
}

impl CompiledReferencePath {
    /// Build the reference of an authored polyline path.
    pub fn from_polyline(points: &[DVec2]) -> Self {
        let segments = points
            .windows(2)
            .map(|pair| ReferenceSegment::Line {
                start: pair[0],
                end: pair[1],
            })
            .collect();
        Self::new(segments)
    }

    /// Build a single circular-arc reference of constant signed curvature.
    ///
    /// `sweep_rad` is signed: positive sweeps counter-clockwise (positive
    /// curvature) and negative clockwise. A non-positive `radius` produces a
    /// degenerate, zero-length reference.
    pub fn arc(center: DVec2, radius: f64, start_angle_rad: f64, sweep_rad: f64) -> Self {
        Self::new(vec![ReferenceSegment::Arc {
            center,
            radius,
            start_angle_rad,
            sweep_rad,
        }])
    }

    fn new(segments: Vec<ReferenceSegment>) -> Self {
        let mut starts = Vec::with_capacity(segments.len() + 1);
        starts.push(0.0);
        for segment in &segments {
            let next = starts.last().copied().unwrap_or(0.0) + segment.length();
            starts.push(next);
        }
        let length = starts.last().copied().unwrap_or(0.0);
        Self {
            segments,
            starts,
            length,
        }
    }

    /// Total reference arc length in metres.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Whether the reference carries any geometry.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// World point at arc length `s`, clamped to `[0, length]`.
    pub fn position_at(&self, s: f64) -> DVec2 {
        match self.locate(s) {
            Some((index, local)) => self.segments[index].position_at(local),
            None => DVec2::ZERO,
        }
    }

    /// Tangent heading in radians, counter-clockwise from the world x axis, at
    /// arc length `s`, clamped to `[0, length]`.
    pub fn heading_at(&self, s: f64) -> f64 {
        match self.locate(s) {
            Some((index, local)) => self.segments[index].heading_at(local),
            None => 0.0,
        }
    }

    /// Unit tangent `(cos heading, sin heading)` at arc length `s`.
    pub fn tangent_at(&self, s: f64) -> DVec2 {
        let heading = self.heading_at(s);
        DVec2::new(heading.cos(), heading.sin())
    }

    /// Unit left normal, the tangent rotated `+90` degrees.
    pub fn normal_at(&self, s: f64) -> DVec2 {
        let heading = self.heading_at(s);
        DVec2::new(-heading.sin(), heading.cos())
    }

    /// Signed curvature `kappa(s) = d theta / ds` in `1/m`, positive for a
    /// counter-clockwise turn.
    ///
    /// A straight segment has zero curvature. A circular arc reports its
    /// constant `+1 / radius` or `-1 / radius`. A polyline reports the
    /// containing segment's curvature, so a vertex is attributed to the
    /// segment it starts (`position_at` clamps to the segment start there).
    pub fn curvature_at(&self, s: f64) -> f64 {
        match self.locate(s) {
            Some((index, _)) => self.segments[index].curvature(),
            None => 0.0,
        }
    }

    /// World position of the route coordinate `(s, d)`.
    pub fn point_at(&self, s: f64, d: f64) -> DVec2 {
        self.position_at(s) + d * self.normal_at(s)
    }

    /// Project a world point onto the reference: `s` from the nearest point on
    /// the reference and `d = (point - position_at(s)) . normal_at(s)`.
    pub fn project(&self, point: DVec2) -> RouteCoordinate {
        let mut best: Option<(f64, DVec2, f64)> = None;
        for (index, segment) in self.segments.iter().enumerate() {
            let (foot, local) = segment.nearest(point);
            let distance = foot.distance_squared(point);
            if best.is_none_or(|(_, _, best_distance)| distance < best_distance) {
                best = Some((self.starts[index] + local, foot, distance));
            }
        }
        match best {
            Some((s, foot, _)) => RouteCoordinate {
                s,
                d: (point - foot).dot(self.normal_at(s)),
            },
            None => RouteCoordinate { s: 0.0, d: 0.0 },
        }
    }

    /// Index of the segment containing `s` and the local arc length within it,
    /// or `None` for a reference with no geometry.
    fn locate(&self, s: f64) -> Option<(usize, f64)> {
        if self.segments.is_empty() {
            return None;
        }
        let s = s.clamp(0.0, self.length);
        let index = self
            .starts
            .partition_point(|&start| start <= s)
            .saturating_sub(1)
            .min(self.segments.len() - 1);
        Some((index, s - self.starts[index]))
    }
}

/// The compiled reference path of a facility: the authored path it came from
/// and the compiled geometry that gives the facility its `(s, d)` frame.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledFacilityReference {
    path: PathId,
    geometry: CompiledReferencePath,
}

impl CompiledFacilityReference {
    /// The authored reference path this geometry was compiled from.
    pub fn path(&self) -> PathId {
        self.path
    }

    /// The compiled reference geometry.
    pub fn geometry(&self) -> &CompiledReferencePath {
        &self.geometry
    }
}

/// A compiled traversable region and reference path: a continuous-width
/// facility with usable width, nominal direction, mode access, lateral-use
/// policy, speed policy, and connector adjacency.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledFacility {
    id: FacilityId,
    name: String,
    region: RegionId,
    reference: Option<CompiledFacilityReference>,
    width_m: f64,
    nominal_direction: NominalDirection,
    access: Vec<ModeTemplateId>,
    lateral_use: LateralUse,
    speed_policy: SpeedPolicy,
    outgoing: Vec<FacilityConnectorId>,
    incoming: Vec<FacilityConnectorId>,
    physically_possible: Vec<MovementDirection>,
}

impl CompiledFacility {
    /// Dense identifier of this facility.
    pub fn id(&self) -> FacilityId {
        self.id
    }

    /// Authored name of this facility.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The traversable region the facility occupies.
    pub fn region(&self) -> RegionId {
        self.region
    }

    /// The compiled reference path, when the facility declares one.
    pub fn reference(&self) -> Option<&CompiledFacilityReference> {
        self.reference.as_ref()
    }

    /// The authored reference path, when the facility declares one.
    pub fn reference_path(&self) -> Option<PathId> {
        self.reference.as_ref().map(CompiledFacilityReference::path)
    }

    /// Reference arc length in metres, when the facility has a reference path.
    pub fn length(&self) -> Option<f64> {
        self.reference
            .as_ref()
            .map(|reference| reference.geometry.length())
    }

    /// World point at reference arc length `s`, when the facility has a
    /// reference path.
    pub fn position_at(&self, s: f64) -> Option<DVec2> {
        self.reference
            .as_ref()
            .map(|reference| reference.geometry.position_at(s))
    }

    /// Tangent heading at arc length `s` in radians, when the facility has a
    /// reference path.
    pub fn heading_at(&self, s: f64) -> Option<f64> {
        self.reference
            .as_ref()
            .map(|reference| reference.geometry.heading_at(s))
    }

    /// Unit tangent at arc length `s`, when the facility has a reference path.
    pub fn tangent_at(&self, s: f64) -> Option<DVec2> {
        self.reference
            .as_ref()
            .map(|reference| reference.geometry.tangent_at(s))
    }

    /// Unit left normal at arc length `s`, when the facility has a reference
    /// path.
    pub fn normal_at(&self, s: f64) -> Option<DVec2> {
        self.reference
            .as_ref()
            .map(|reference| reference.geometry.normal_at(s))
    }

    /// Signed curvature at arc length `s` in `1/m`, when the facility has a
    /// reference path.
    pub fn curvature_at(&self, s: f64) -> Option<f64> {
        self.reference
            .as_ref()
            .map(|reference| reference.geometry.curvature_at(s))
    }

    /// Usable traversable width in metres, measured across the reference path.
    pub fn width_m(&self) -> f64 {
        self.width_m
    }

    /// The authored nominal direction of the reference path.
    pub fn nominal_direction(&self) -> NominalDirection {
        self.nominal_direction
    }

    /// Mode templates permitted to use this facility, in authored order.
    pub fn access(&self) -> &[ModeTemplateId] {
        &self.access
    }

    /// Whether the facility permits `mode`.
    pub fn permits_mode(&self, mode: ModeTemplateId) -> bool {
        self.access.contains(&mode)
    }

    /// Whether the usable lateral interval is shared or centered.
    pub fn lateral_use(&self) -> LateralUse {
        self.lateral_use
    }

    /// The facility's own speed policy; the effective limit on the facility is
    /// the more restrictive of this and the permitting mode's policy.
    pub fn speed_policy(&self) -> SpeedPolicy {
        self.speed_policy
    }

    /// The directed connectors that leave this facility, in authored order.
    pub fn outgoing_connectors(&self) -> &[FacilityConnectorId] {
        &self.outgoing
    }

    /// The directed connectors that enter this facility, in authored order.
    pub fn incoming_connectors(&self) -> &[FacilityConnectorId] {
        &self.incoming
    }

    /// The traversal directions an attached connector makes physically
    /// possible, in authored connector order.
    ///
    /// This is the third direction property, kept separate from the authored
    /// [`Self::nominal_direction`] and the mode-dependent permitted direction:
    /// a direction is possible when a connector leaves or enters the facility
    /// along it.
    pub fn physically_possible_directions(&self) -> &[MovementDirection] {
        &self.physically_possible
    }

    /// Whether a connected traversal exists in `direction`.
    pub fn is_physically_possible(&self, direction: MovementDirection) -> bool {
        self.physically_possible.contains(&direction)
    }

    /// The usable lateral interval for a body of envelope width
    /// `envelope_width_m` and lateral clearance `clearance_m`.
    ///
    /// The band is centred on the reference path with total width
    /// `width_m`, so a body fits when `|d| + envelope/2 + clearance <= W/2`,
    /// giving `d_min = -(W/2 - envelope/2 - clearance)` and its negation. The
    /// interval is empty when `envelope + 2 * clearance > W`.
    pub fn usable_lateral_interval(
        &self,
        envelope_width_m: f64,
        clearance_m: f64,
    ) -> UsableLateralInterval {
        let half = self.width_m * 0.5 - envelope_width_m * 0.5 - clearance_m;
        UsableLateralInterval {
            d_min: -half,
            d_max: half,
        }
    }
}

/// The signed lateral offsets a body of a given envelope and clearance may
/// occupy inside a facility band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsableLateralInterval {
    d_min: f64,
    d_max: f64,
}

impl UsableLateralInterval {
    /// Lowest usable signed lateral offset in metres.
    pub fn d_min(self) -> f64 {
        self.d_min
    }

    /// Highest usable signed lateral offset in metres.
    pub fn d_max(self) -> f64 {
        self.d_max
    }

    /// Whether no offset fits: the body envelope plus clearance exceeds the
    /// facility width.
    pub fn is_empty(self) -> bool {
        self.d_max < self.d_min
    }

    /// Whether `d` lies within the interval.
    pub fn contains(self, d: f64) -> bool {
        !self.is_empty() && (self.d_min..=self.d_max).contains(&d)
    }
}

/// One facility traversal named by a connector: a facility and the direction
/// of travel along its reference path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FacilityTraversal {
    facility: FacilityId,
    direction: MovementDirection,
}

impl FacilityTraversal {
    /// The facility traversed.
    pub fn facility(self) -> FacilityId {
        self.facility
    }

    /// The traversal direction along that facility.
    pub fn direction(self) -> MovementDirection {
        self.direction
    }
}

/// A compiled directed connector joining two facility traversals.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledFacilityConnector {
    id: FacilityConnectorId,
    name: String,
    from: FacilityTraversal,
    to: FacilityTraversal,
}

impl CompiledFacilityConnector {
    /// Dense identifier of this connector.
    pub fn id(&self) -> FacilityConnectorId {
        self.id
    }

    /// Authored name of this connector.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The traversal the connector leaves.
    pub fn from(&self) -> FacilityTraversal {
        self.from
    }

    /// The traversal the connector enters.
    pub fn to(&self) -> FacilityTraversal {
        self.to
    }
}

/// A validated portal attached to a path end.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPortal {
    id: PortalId,
    name: String,
    path: PathId,
    end: PathEnd,
    position: DVec2,
    heading: f64,
    width_m: f64,
}

impl CompiledPortal {
    /// Dense identifier of this portal.
    pub fn id(&self) -> PortalId {
        self.id
    }

    /// Authored name of this portal.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The path this portal attaches to.
    pub fn path(&self) -> PathId {
        self.path
    }

    /// Which end of the path this portal marks.
    pub fn end(&self) -> PathEnd {
        self.end
    }

    /// World position of the portal in metres.
    pub fn position(&self) -> DVec2 {
        self.position
    }

    /// Inward heading of the portal in radians.
    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// Traversable width in metres.
    pub fn width_m(&self) -> f64 {
        self.width_m
    }
}

/// A closed polygon ring with cached area and centroid.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPolygon {
    ring: Vec<DVec2>,
    signed_area: f64,
    centroid: DVec2,
}

impl CompiledPolygon {
    /// Derive a polygon from its ring vertices, which must not repeat the
    /// first point at the end.
    fn new(ring: Vec<DVec2>) -> Self {
        let (signed_area, centroid) = ring_geometry(&ring);
        Self {
            ring,
            signed_area,
            centroid,
        }
    }

    /// Ring vertices in order; the last vertex connects back to the first.
    pub fn ring(&self) -> &[DVec2] {
        &self.ring
    }

    /// Enclosed area in square metres, always non-negative.
    pub fn area(&self) -> f64 {
        self.signed_area.abs()
    }

    /// Signed area in square metres; positive for counter-clockwise winding.
    pub fn signed_area(&self) -> f64 {
        self.signed_area
    }

    /// Area centroid in metres.
    pub fn centroid(&self) -> DVec2 {
        self.centroid
    }
}

/// Signed area and centroid of a polygon ring; the centroid falls back to the
/// vertex average for a degenerate ring, which validation rejects upstream.
fn ring_geometry(ring: &[DVec2]) -> (f64, DVec2) {
    let count = ring.len();
    let mut twice_area = 0.0;
    let mut weighted = DVec2::ZERO;
    for index in 0..count {
        let current = ring[index];
        let next = ring[(index + 1) % count];
        let cross = current.x * next.y - next.x * current.y;
        twice_area += cross;
        weighted += (current + next) * cross;
    }
    let signed_area = twice_area * 0.5;
    if signed_area.abs() <= f64::EPSILON {
        let average = if count == 0 {
            DVec2::ZERO
        } else {
            ring.iter().copied().sum::<DVec2>() / count as f64
        };
        return (0.0, average);
    }
    (signed_area, weighted / (3.0 * twice_area))
}

/// A compiled boundary polygon marking the non-traversable world limits.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledBoundary {
    id: BoundaryId,
    name: String,
    polygon: CompiledPolygon,
}

impl CompiledBoundary {
    /// Dense identifier of this boundary.
    pub fn id(&self) -> BoundaryId {
        self.id
    }

    /// Authored name of this boundary.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Derived ring geometry of this boundary.
    pub fn polygon(&self) -> &CompiledPolygon {
        &self.polygon
    }
}

/// A compiled traversable region other than a guide path.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRegion {
    id: RegionId,
    name: String,
    polygon: CompiledPolygon,
}

impl CompiledRegion {
    /// Dense identifier of this region.
    pub fn id(&self) -> RegionId {
        self.id
    }

    /// Authored name of this region.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Derived ring geometry of this region.
    pub fn polygon(&self) -> &CompiledPolygon {
        &self.polygon
    }
}

/// A compiled movement connector from one portal to another along a path.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledMovement {
    id: MovementId,
    name: String,
    from: PortalId,
    to: PortalId,
    path: PathId,
    priority: u32,
    stop_line_m: f64,
    entry: DVec2,
    entry_heading: f64,
    exit: DVec2,
    exit_heading: f64,
}

impl CompiledMovement {
    /// Dense identifier of this movement.
    pub fn id(&self) -> MovementId {
        self.id
    }

    /// Authored name of this movement.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Portal where the movement begins.
    pub fn from(&self) -> PortalId {
        self.from
    }

    /// Portal where the movement ends.
    pub fn to(&self) -> PortalId {
        self.to
    }

    /// Guide path the movement follows.
    pub fn path(&self) -> PathId {
        self.path
    }

    /// Lower values are honored before higher ones.
    pub fn priority(&self) -> u32 {
        self.priority
    }

    /// Stop-line arc length in metres from the movement entry, measured along
    /// the movement's direction of travel. `0.0` places it at the entry portal.
    pub fn stop_line_m(&self) -> f64 {
        self.stop_line_m
    }

    /// World position of the entry endpoint in metres.
    pub fn entry(&self) -> DVec2 {
        self.entry
    }

    /// Inward heading at the entry endpoint in radians.
    pub fn entry_heading(&self) -> f64 {
        self.entry_heading
    }

    /// World position of the exit endpoint in metres.
    pub fn exit(&self) -> DVec2 {
        self.exit
    }

    /// Heading at the exit endpoint in radians.
    pub fn exit_heading(&self) -> f64 {
        self.exit_heading
    }
}

/// A compiled pedestrian crossing over a traversable region and its movements.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledCrossing {
    id: CrossingId,
    name: String,
    region: RegionId,
    movements: Vec<MovementId>,
    pedestrian_signal: Option<CompiledPedestrianSignal>,
}

impl CompiledCrossing {
    /// Dense identifier of this crossing.
    pub fn id(&self) -> CrossingId {
        self.id
    }

    /// Authored name of this crossing.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Region the crossing occupies.
    pub fn region(&self) -> RegionId {
        self.region
    }

    /// Movements the crossing crosses, in authored order.
    pub fn movements(&self) -> &[MovementId] {
        &self.movements
    }

    /// Fixed-time pedestrian signal rule, present exactly when the crossing is
    /// signal-controlled. `None` is an uncontrolled crossing.
    pub fn pedestrian_signal(&self) -> Option<&CompiledPedestrianSignal> {
        self.pedestrian_signal.as_ref()
    }
}

/// A compiled fixed-time pedestrian signal rule embedded in one crossing.
///
/// Phases are contiguous and ordered by `start_s`, so the active phase at a
/// cycle time is well defined; `cycle_s` is their total duration.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPedestrianSignal {
    phases: Vec<CompiledPedestrianSignalPhase>,
    cycle_s: f64,
}

impl CompiledPedestrianSignal {
    /// Phases in cycle order, each with its offset from the cycle start.
    pub fn phases(&self) -> &[CompiledPedestrianSignalPhase] {
        &self.phases
    }

    /// Total phase duration in seconds, the length of one cycle.
    pub fn cycle_s(&self) -> f64 {
        self.cycle_s
    }

    /// Whether pedestrians may cross during the phase active at `elapsed_s`.
    ///
    /// A phase covers the half-open interval `[start_s, start_s + duration_s)`,
    /// so a boundary tick belongs to the later phase. Returns `None` only for a
    /// signal with no phases, which validation rejects.
    pub fn walk_at(&self, elapsed_s: f64) -> Option<bool> {
        let offset = if self.cycle_s > 0.0 {
            elapsed_s.rem_euclid(self.cycle_s)
        } else {
            0.0
        };
        self.phases
            .iter()
            .rev()
            .find(|phase| phase.start_s <= offset)
            .map(|phase| phase.walk)
    }
}

/// One compiled fixed-time pedestrian signal phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompiledPedestrianSignalPhase {
    duration_s: f64,
    start_s: f64,
    walk: bool,
}

impl CompiledPedestrianSignalPhase {
    /// Phase duration in seconds.
    pub fn duration_s(&self) -> f64 {
        self.duration_s
    }

    /// Offset of this phase from the start of the cycle in seconds.
    pub fn start_s(&self) -> f64 {
        self.start_s
    }

    /// Whether pedestrians may cross during this phase.
    pub fn walk(&self) -> bool {
        self.walk
    }
}

/// A compiled waiting area where pedestrians stage between crossings.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledWaitingArea {
    id: WaitingAreaId,
    name: String,
    region: RegionId,
}

impl CompiledWaitingArea {
    /// Dense identifier of this waiting area.
    pub fn id(&self) -> WaitingAreaId {
        self.id
    }

    /// Authored name of this waiting area.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Region the waiting area occupies.
    pub fn region(&self) -> RegionId {
        self.region
    }
}

/// A compiled pedestrian route from one portal to another along a guide path.
///
/// `crossings` and `waiting_areas` are the zones the route passes through in
/// travel order. Each names a compiled object rather than copied geometry, so
/// the route stays a routing primitive over the authored layout.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPedestrianRoute {
    id: PedestrianRouteId,
    name: String,
    from: PortalId,
    to: PortalId,
    path: PathId,
    crossings: Vec<CrossingId>,
    waiting_areas: Vec<WaitingAreaId>,
    entry: DVec2,
    entry_heading: f64,
    exit: DVec2,
    exit_heading: f64,
}

impl CompiledPedestrianRoute {
    /// Dense identifier of this route.
    pub fn id(&self) -> PedestrianRouteId {
        self.id
    }

    /// Authored name of this route.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Portal where the route begins.
    pub fn from(&self) -> PortalId {
        self.from
    }

    /// Portal where the route ends.
    pub fn to(&self) -> PortalId {
        self.to
    }

    /// Guide path the route follows.
    pub fn path(&self) -> PathId {
        self.path
    }

    /// Crossings the route traverses, in travel order.
    pub fn crossings(&self) -> &[CrossingId] {
        &self.crossings
    }

    /// Waiting areas the route stages at, in travel order.
    pub fn waiting_areas(&self) -> &[WaitingAreaId] {
        &self.waiting_areas
    }

    /// World position of the entry endpoint in metres.
    pub fn entry(&self) -> DVec2 {
        self.entry
    }

    /// Inward heading at the entry endpoint in radians.
    pub fn entry_heading(&self) -> f64 {
        self.entry_heading
    }

    /// World position of the exit endpoint in metres.
    pub fn exit(&self) -> DVec2 {
        self.exit
    }

    /// Heading at the exit endpoint in radians.
    pub fn exit_heading(&self) -> f64 {
        self.exit_heading
    }
}

/// A compiled conflict region shared by exactly two movements.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledConflictRegion {
    id: ConflictRegionId,
    name: String,
    polygon: CompiledPolygon,
    movements: [MovementId; 2],
}

impl CompiledConflictRegion {
    /// Dense identifier of this conflict region.
    pub fn id(&self) -> ConflictRegionId {
        self.id
    }

    /// Authored name of this conflict region.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Derived ring geometry of this conflict region.
    pub fn polygon(&self) -> &CompiledPolygon {
        &self.polygon
    }

    /// The two movements whose envelopes conflict here.
    pub fn movements(&self) -> [MovementId; 2] {
        self.movements
    }
}

/// A compiled control rule attached to one movement.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRule {
    id: RuleId,
    name: String,
    movement: MovementId,
    kind: RuleKind,
    signal: Option<SignalId>,
}

impl CompiledRule {
    /// Dense identifier of this rule.
    pub fn id(&self) -> RuleId {
        self.id
    }

    /// Authored name of this rule.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Movement this rule governs.
    pub fn movement(&self) -> MovementId {
        self.movement
    }

    /// Kind of control the rule applies.
    pub fn kind(&self) -> RuleKind {
        self.kind
    }

    /// Signal controller, present exactly for a signal rule.
    pub fn signal(&self) -> Option<SignalId> {
        self.signal
    }
}

/// A compiled signal head controlling one movement.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSignalHead {
    name: String,
    movement: MovementId,
}

impl CompiledSignalHead {
    /// Authored head identifier, unique within its signal.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Movement this head controls.
    pub fn movement(&self) -> MovementId {
        self.movement
    }
}

/// One compiled signal phase.
///
/// `colors` is parallel to the owning signal's head list, so a phase always
/// carries exactly one color per head.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSignalPhase {
    duration_s: f64,
    start_s: f64,
    colors: Vec<SignalColor>,
}

impl CompiledSignalPhase {
    /// Phase duration in seconds.
    pub fn duration_s(&self) -> f64 {
        self.duration_s
    }

    /// Offset of this phase from the start of the cycle in seconds.
    pub fn start_s(&self) -> f64 {
        self.start_s
    }

    /// Color shown on each head, parallel to the signal's head list.
    pub fn colors(&self) -> &[SignalColor] {
        &self.colors
    }

    /// Color shown on the head at `head_index`, if it exists.
    pub fn color(&self, head_index: usize) -> Option<SignalColor> {
        self.colors.get(head_index).copied()
    }
}

/// A compiled fixed-time signal controller.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSignal {
    id: SignalId,
    name: String,
    heads: Vec<CompiledSignalHead>,
    phases: Vec<CompiledSignalPhase>,
    cycle_s: f64,
}

impl CompiledSignal {
    /// Dense identifier of this signal.
    pub fn id(&self) -> SignalId {
        self.id
    }

    /// Authored name of this signal.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Heads controlled together, in authored order.
    pub fn heads(&self) -> &[CompiledSignalHead] {
        &self.heads
    }

    /// Phases in cycle order, in authored order.
    pub fn phases(&self) -> &[CompiledSignalPhase] {
        &self.phases
    }

    /// Total cycle length in seconds: the sum of every phase duration.
    pub fn cycle_s(&self) -> f64 {
        self.cycle_s
    }
}

/// One movement's relative share of a demand source's arrivals.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRouteShare {
    movement: MovementId,
    weight: f64,
}

impl CompiledRouteShare {
    /// The movement a generated vehicle follows.
    pub fn movement(&self) -> MovementId {
        self.movement
    }

    /// Relative weight; larger values are chosen proportionally more often.
    pub fn weight(&self) -> f64 {
        self.weight
    }
}

/// A compiled portal demand generator.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledDemand {
    id: DemandId,
    name: String,
    portal: PortalId,
    rate_vph: f64,
    routes: Vec<CompiledRouteShare>,
}

impl CompiledDemand {
    /// Dense identifier of this demand source.
    pub fn id(&self) -> DemandId {
        self.id
    }

    /// Authored name of this demand source.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Entry portal where generated vehicles enter.
    pub fn portal(&self) -> PortalId {
        self.portal
    }

    /// Mean arrival rate in vehicles per hour.
    pub fn rate_vph(&self) -> f64 {
        self.rate_vph
    }

    /// Weighted routes a generated vehicle may follow.
    pub fn routes(&self) -> &[CompiledRouteShare] {
        &self.routes
    }
}

/// One pedestrian route's relative share of a pedestrian demand source's
/// arrivals.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPedestrianRouteShare {
    route: PedestrianRouteId,
    weight: f64,
}

impl CompiledPedestrianRouteShare {
    /// The route a generated pedestrian follows.
    pub fn route(&self) -> PedestrianRouteId {
        self.route
    }

    /// Relative weight; larger values are chosen proportionally more often.
    pub fn weight(&self) -> f64 {
        self.weight
    }
}

/// A compiled pedestrian demand generator.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPedestrianDemand {
    id: PedestrianDemandId,
    name: String,
    portal: PortalId,
    rate_pph: f64,
    routes: Vec<CompiledPedestrianRouteShare>,
}

impl CompiledPedestrianDemand {
    /// Dense identifier of this demand source.
    pub fn id(&self) -> PedestrianDemandId {
        self.id
    }

    /// Authored name of this demand source.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Entry portal where generated pedestrians enter.
    pub fn portal(&self) -> PortalId {
        self.portal
    }

    /// Mean arrival rate in pedestrians per hour.
    pub fn rate_pph(&self) -> f64 {
        self.rate_pph
    }

    /// Weighted routes a generated pedestrian may follow.
    pub fn routes(&self) -> &[CompiledPedestrianRouteShare] {
        &self.routes
    }
}

/// An inclusive uniform profile range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileRange {
    min: f64,
    max: f64,
}

impl ProfileRange {
    /// Construct an inclusive range from its bounds.
    ///
    /// A range with `min == max` is a constant, matching the authored
    /// [`crate::source::ProfileRangeSource`] contract.
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    fn from_source(range: crate::source::ProfileRangeSource) -> Self {
        Self {
            min: range.min,
            max: range.max,
        }
    }

    /// Lower bound of the distribution.
    pub fn min(self) -> f64 {
        self.min
    }

    /// Upper bound of the distribution.
    pub fn max(self) -> f64 {
        self.max
    }

    /// Interpolate the range at `u` in `[0, 1]`, clamped to the bounds.
    pub fn sample(self, u: f64) -> f64 {
        self.min + (self.max - self.min) * u.clamp(0.0, 1.0)
    }
}

/// Compiled passenger-car physical and behavior profile distributions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompiledProfile {
    speed_mps: ProfileRange,
    length_m: ProfileRange,
    width_m: ProfileRange,
    time_gap_s: ProfileRange,
    max_accel_mps2: ProfileRange,
    comfortable_brake_mps2: ProfileRange,
    compliance: ProfileRange,
}

impl CompiledProfile {
    fn from_source(source: crate::source::ProfileSource) -> Self {
        Self {
            speed_mps: ProfileRange::from_source(source.speed_mps),
            length_m: ProfileRange::from_source(source.length_m),
            width_m: ProfileRange::from_source(source.width_m),
            time_gap_s: ProfileRange::from_source(source.time_gap_s),
            max_accel_mps2: ProfileRange::from_source(source.max_accel_mps2),
            comfortable_brake_mps2: ProfileRange::from_source(source.comfortable_brake_mps2),
            compliance: ProfileRange::from_source(source.compliance),
        }
    }

    /// Desired free-flow speed distribution in metres per second.
    pub fn speed_mps(&self) -> ProfileRange {
        self.speed_mps
    }

    /// Body length distribution in metres.
    pub fn length_m(&self) -> ProfileRange {
        self.length_m
    }

    /// Body width distribution in metres.
    pub fn width_m(&self) -> ProfileRange {
        self.width_m
    }

    /// Desired following time-gap distribution in seconds.
    pub fn time_gap_s(&self) -> ProfileRange {
        self.time_gap_s
    }

    /// Maximum acceleration distribution in metres per second squared.
    pub fn max_accel_mps2(&self) -> ProfileRange {
        self.max_accel_mps2
    }

    /// Comfortable deceleration distribution in metres per second squared.
    pub fn comfortable_brake_mps2(&self) -> ProfileRange {
        self.comfortable_brake_mps2
    }

    /// Signal-compliance propensity distribution, a fraction in `[0, 1]`.
    pub fn compliance(&self) -> ProfileRange {
        self.compliance
    }
}

/// Compiled pedestrian physical and behavior profile distributions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompiledPedestrianProfile {
    radius_m: ProfileRange,
    speed_mps: ProfileRange,
    compliance: ProfileRange,
}

impl CompiledPedestrianProfile {
    fn from_source(source: crate::source::PedestrianProfileSource) -> Self {
        Self {
            radius_m: ProfileRange::from_source(source.radius_m),
            speed_mps: ProfileRange::from_source(source.speed_mps),
            compliance: ProfileRange::from_source(source.compliance),
        }
    }

    /// Body radius distribution in metres.
    pub fn radius_m(&self) -> ProfileRange {
        self.radius_m
    }

    /// Desired walking speed distribution in metres per second.
    pub fn speed_mps(&self) -> ProfileRange {
        self.speed_mps
    }

    /// Signal-compliance propensity distribution, a fraction in `[0, 1]`.
    pub fn compliance(&self) -> ProfileRange {
        self.compliance
    }
}

/// A validated, immutable scenario ready for the kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledScenario {
    id: String,
    schema_version: u32,
    paths: Vec<CompiledPath>,
    portals: Vec<CompiledPortal>,
    boundaries: Vec<CompiledBoundary>,
    regions: Vec<CompiledRegion>,
    movements: Vec<CompiledMovement>,
    crossings: Vec<CompiledCrossing>,
    waiting_areas: Vec<CompiledWaitingArea>,
    pedestrian_routes: Vec<CompiledPedestrianRoute>,
    conflict_regions: Vec<CompiledConflictRegion>,
    rules: Vec<CompiledRule>,
    signals: Vec<CompiledSignal>,
    demand: Vec<CompiledDemand>,
    pedestrian_demand: Vec<CompiledPedestrianDemand>,
    profiles: CompiledProfile,
    pedestrian_profiles: CompiledPedestrianProfile,
    population: PopulationSource,
    mode_templates: Vec<CompiledModeTemplate>,
    /// Mode template a version-2 demand source produces, parallel to
    /// [`Self::demand`] and indexed by [`DemandId`]. The version-1 view has no
    /// mode templates, so every entry is `None` there; `compile_v2` fills the
    /// authored template of each mode-tagged demand source.
    demand_modes: Vec<Option<ModeTemplateId>>,
    facilities: Vec<CompiledFacility>,
    facility_connectors: Vec<CompiledFacilityConnector>,
    id_map: IdMap,
}

impl CompiledScenario {
    /// Validate and compile a source scenario.
    ///
    /// Returns every [`Diagnostic`] when the source is invalid; only a fully
    /// valid scenario produces a [`CompiledScenario`].
    pub fn compile(source: ScenarioSource) -> Result<Self, Vec<Diagnostic>> {
        let diagnostics = validate(&source);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(Self::compile_validated(source))
    }

    /// Validate and compile a version-2 source scenario.
    ///
    /// Version 2 does not change the Increment 0 compiled fields: mode
    /// templates and mode-tagged demand are materialized into the version-1
    /// fields (profiles, pedestrian profiles, population, demand), so the
    /// kernel consumes the same compiled representation. Alongside that view,
    /// the Increment 1 authored shapes populate the compiled facilities, their
    /// connectors, and the compiled mode-template bundles; a version-1 source
    /// has none of these.
    pub fn compile_v2(source: ScenarioSourceV2) -> Result<Self, Vec<Diagnostic>> {
        let diagnostics = validate_v2(&source);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let mode_templates = compile_mode_templates(&source.mode_templates)?;
        let mode_template_index = index_by_id(
            source
                .mode_templates
                .iter()
                .map(|template| template.id.as_str()),
        );
        let facilities = compile_facilities(&source, &mode_template_index);
        let facility_connectors = compile_facility_connectors(&source);
        let facilities = attach_facility_adjacency(facilities, &facility_connectors);
        let facility_names = names(source.facilities.iter().map(|facility| &facility.id));
        let facility_connector_names = names(
            source
                .facility_connectors
                .iter()
                .map(|connector| &connector.id),
        );
        // Each mode-tagged demand source's template, in the same order the
        // shared version-1 view materializes its `demand` array. Validation has
        // already rejected an undeclared mode, so an entry is always found.
        let demand_modes: Vec<Option<ModeTemplateId>> = source
            .demand
            .iter()
            .map(|entry| {
                mode_template_index
                    .get(entry.mode.as_str())
                    .map(|&index| ModeTemplateId::from_index(index))
            })
            .collect();

        let mut scenario = Self::compile_validated(v2_to_v1_view(source));
        scenario.mode_templates = mode_templates;
        scenario.demand_modes = demand_modes;
        scenario.facilities = facilities;
        scenario.facility_connectors = facility_connectors;
        scenario.id_map.facilities = facility_names;
        scenario.id_map.facility_connectors = facility_connector_names;
        Ok(scenario)
    }

    /// Compile a source scenario that has already passed validation.
    fn compile_validated(source: ScenarioSource) -> Self {
        // Validation guarantees these lookups succeed, so a missing name is a
        // programming error rather than a user diagnostic.
        let path_index = index_by_id(source.paths.iter().map(|path| path.id.as_str()));
        let portal_index = index_by_id(source.portals.iter().map(|portal| portal.id.as_str()));
        let region_index = index_by_id(source.regions.iter().map(|region| region.id.as_str()));
        let movement_index =
            index_by_id(source.movements.iter().map(|movement| movement.id.as_str()));
        let crossing_index =
            index_by_id(source.crossings.iter().map(|crossing| crossing.id.as_str()));
        let waiting_area_index =
            index_by_id(source.waiting_areas.iter().map(|area| area.id.as_str()));
        let pedestrian_route_index = index_by_id(
            source
                .pedestrian_routes
                .iter()
                .map(|route| route.id.as_str()),
        );
        let signal_index = index_by_id(source.signals.iter().map(|signal| signal.id.as_str()));

        let mut paths = Vec::with_capacity(source.paths.len());
        for (index, path) in source.paths.iter().enumerate() {
            paths.push(compile_path(PathId::from_index(index), path));
        }

        let mut portals = Vec::with_capacity(source.portals.len());
        for (index, portal) in source.portals.iter().enumerate() {
            let path_id = PathId::from_index(path_index[portal.path.as_str()]);
            let path = &paths[path_id.index()];
            portals.push(compile_portal(PortalId::from_index(index), portal, path));
        }

        let boundaries: Vec<CompiledBoundary> = source
            .boundaries
            .iter()
            .enumerate()
            .map(|(index, boundary)| CompiledBoundary {
                id: BoundaryId::from_index(index),
                name: boundary.id.clone(),
                polygon: CompiledPolygon::new(points_of(&boundary.points)),
            })
            .collect();

        let regions: Vec<CompiledRegion> = source
            .regions
            .iter()
            .enumerate()
            .map(|(index, region)| CompiledRegion {
                id: RegionId::from_index(index),
                name: region.id.clone(),
                polygon: CompiledPolygon::new(points_of(&region.points)),
            })
            .collect();

        let movements: Vec<CompiledMovement> = source
            .movements
            .iter()
            .enumerate()
            .map(|(index, movement)| {
                let from = PortalId::from_index(portal_index[movement.from.as_str()]);
                let to = PortalId::from_index(portal_index[movement.to.as_str()]);
                let entry_portal = &portals[from.index()];
                let exit_portal = &portals[to.index()];
                CompiledMovement {
                    id: MovementId::from_index(index),
                    name: movement.id.clone(),
                    from,
                    to,
                    path: PathId::from_index(path_index[movement.path.as_str()]),
                    priority: movement.priority,
                    stop_line_m: movement.stop_line_m,
                    entry: entry_portal.position(),
                    entry_heading: entry_portal.heading(),
                    exit: exit_portal.position(),
                    exit_heading: exit_portal.heading(),
                }
            })
            .collect();

        let crossings: Vec<CompiledCrossing> = source
            .crossings
            .iter()
            .enumerate()
            .map(|(index, crossing)| CompiledCrossing {
                id: CrossingId::from_index(index),
                name: crossing.id.clone(),
                region: RegionId::from_index(region_index[crossing.region.as_str()]),
                movements: crossing
                    .movements
                    .iter()
                    .map(|movement| MovementId::from_index(movement_index[movement.as_str()]))
                    .collect(),
                pedestrian_signal: crossing
                    .pedestrian_signal
                    .as_ref()
                    .map(compile_pedestrian_signal),
            })
            .collect();

        let waiting_areas: Vec<CompiledWaitingArea> = source
            .waiting_areas
            .iter()
            .enumerate()
            .map(|(index, area)| CompiledWaitingArea {
                id: WaitingAreaId::from_index(index),
                name: area.id.clone(),
                region: RegionId::from_index(region_index[area.region.as_str()]),
            })
            .collect();

        let pedestrian_routes: Vec<CompiledPedestrianRoute> = source
            .pedestrian_routes
            .iter()
            .enumerate()
            .map(|(index, route)| {
                let from = PortalId::from_index(portal_index[route.from.as_str()]);
                let to = PortalId::from_index(portal_index[route.to.as_str()]);
                let entry_portal = &portals[from.index()];
                let exit_portal = &portals[to.index()];
                CompiledPedestrianRoute {
                    id: PedestrianRouteId::from_index(index),
                    name: route.id.clone(),
                    from,
                    to,
                    path: PathId::from_index(path_index[route.path.as_str()]),
                    crossings: route
                        .crossings
                        .iter()
                        .map(|crossing| CrossingId::from_index(crossing_index[crossing.as_str()]))
                        .collect(),
                    waiting_areas: route
                        .waiting_areas
                        .iter()
                        .map(|area| WaitingAreaId::from_index(waiting_area_index[area.as_str()]))
                        .collect(),
                    entry: entry_portal.position(),
                    entry_heading: entry_portal.heading(),
                    exit: exit_portal.position(),
                    exit_heading: exit_portal.heading(),
                }
            })
            .collect();

        let conflict_regions: Vec<CompiledConflictRegion> = source
            .conflict_regions
            .iter()
            .enumerate()
            .map(|(index, conflict)| CompiledConflictRegion {
                id: ConflictRegionId::from_index(index),
                name: conflict.id.clone(),
                polygon: CompiledPolygon::new(points_of(&conflict.points)),
                movements: [
                    MovementId::from_index(movement_index[conflict.movements[0].as_str()]),
                    MovementId::from_index(movement_index[conflict.movements[1].as_str()]),
                ],
            })
            .collect();

        let signals: Vec<CompiledSignal> = source
            .signals
            .iter()
            .enumerate()
            .map(|(index, signal)| {
                compile_signal(SignalId::from_index(index), signal, &movement_index)
            })
            .collect();

        let rules: Vec<CompiledRule> = source
            .rules
            .iter()
            .enumerate()
            .map(|(index, rule)| CompiledRule {
                id: RuleId::from_index(index),
                name: rule.id.clone(),
                movement: MovementId::from_index(movement_index[rule.movement.as_str()]),
                kind: rule.kind,
                signal: rule
                    .signal
                    .as_ref()
                    .map(|signal| SignalId::from_index(signal_index[signal.as_str()])),
            })
            .collect();

        let demand: Vec<CompiledDemand> = source
            .demand
            .iter()
            .enumerate()
            .map(|(index, demand)| CompiledDemand {
                id: DemandId::from_index(index),
                name: demand.id.clone(),
                portal: PortalId::from_index(portal_index[demand.portal.as_str()]),
                rate_vph: demand.rate_vph,
                routes: demand
                    .routes
                    .iter()
                    .map(|route| CompiledRouteShare {
                        movement: MovementId::from_index(movement_index[route.movement.as_str()]),
                        weight: route.weight,
                    })
                    .collect(),
            })
            .collect();

        let pedestrian_demand: Vec<CompiledPedestrianDemand> = source
            .pedestrian_demand
            .iter()
            .enumerate()
            .map(|(index, demand)| CompiledPedestrianDemand {
                id: PedestrianDemandId::from_index(index),
                name: demand.id.clone(),
                portal: PortalId::from_index(portal_index[demand.portal.as_str()]),
                rate_pph: demand.rate_pph,
                routes: demand
                    .routes
                    .iter()
                    .map(|share| CompiledPedestrianRouteShare {
                        route: PedestrianRouteId::from_index(
                            pedestrian_route_index[share.route.as_str()],
                        ),
                        weight: share.weight,
                    })
                    .collect(),
            })
            .collect();

        let id_map = IdMap {
            paths: names(source.paths.iter().map(|path| &path.id)),
            portals: names(source.portals.iter().map(|portal| &portal.id)),
            boundaries: names(source.boundaries.iter().map(|boundary| &boundary.id)),
            regions: names(source.regions.iter().map(|region| &region.id)),
            movements: names(source.movements.iter().map(|movement| &movement.id)),
            crossings: names(source.crossings.iter().map(|crossing| &crossing.id)),
            waiting_areas: names(source.waiting_areas.iter().map(|area| &area.id)),
            pedestrian_routes: names(source.pedestrian_routes.iter().map(|route| &route.id)),
            conflict_regions: names(source.conflict_regions.iter().map(|conflict| &conflict.id)),
            rules: names(source.rules.iter().map(|rule| &rule.id)),
            signals: names(source.signals.iter().map(|signal| &signal.id)),
            demand: names(source.demand.iter().map(|demand| &demand.id)),
            pedestrian_demand: names(source.pedestrian_demand.iter().map(|demand| &demand.id)),
            facilities: Vec::new(),
            facility_connectors: Vec::new(),
        };

        Self {
            id: source.id,
            schema_version: source.schema_version,
            paths,
            portals,
            boundaries,
            regions,
            movements,
            crossings,
            waiting_areas,
            pedestrian_routes,
            conflict_regions,
            rules,
            signals,
            demand,
            pedestrian_demand,
            profiles: CompiledProfile::from_source(source.profiles),
            pedestrian_profiles: CompiledPedestrianProfile::from_source(source.pedestrian_profiles),
            population: source.population,
            // The Increment 1 compiled shapes are version-2 only; `compile_v2`
            // fills them after this shared version-1 view.
            mode_templates: Vec::new(),
            demand_modes: Vec::new(),
            facilities: Vec::new(),
            facility_connectors: Vec::new(),
            id_map,
        }
    }

    /// Authored scenario identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Schema version the scenario was authored against.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Compiled guide paths in dense-index order.
    pub fn paths(&self) -> &[CompiledPath] {
        &self.paths
    }

    /// Compiled portals in dense-index order.
    pub fn portals(&self) -> &[CompiledPortal] {
        &self.portals
    }

    /// Compiled boundary polygons in dense-index order.
    pub fn boundaries(&self) -> &[CompiledBoundary] {
        &self.boundaries
    }

    /// Compiled traversable regions in dense-index order.
    pub fn regions(&self) -> &[CompiledRegion] {
        &self.regions
    }

    /// Compiled movements in dense-index order.
    pub fn movements(&self) -> &[CompiledMovement] {
        &self.movements
    }

    /// Compiled crossings in dense-index order.
    pub fn crossings(&self) -> &[CompiledCrossing] {
        &self.crossings
    }

    /// Compiled waiting areas in dense-index order.
    pub fn waiting_areas(&self) -> &[CompiledWaitingArea] {
        &self.waiting_areas
    }

    /// Compiled pedestrian routes in dense-index order.
    pub fn pedestrian_routes(&self) -> &[CompiledPedestrianRoute] {
        &self.pedestrian_routes
    }

    /// Compiled conflict regions in dense-index order.
    pub fn conflict_regions(&self) -> &[CompiledConflictRegion] {
        &self.conflict_regions
    }

    /// Compiled control rules in dense-index order.
    pub fn rules(&self) -> &[CompiledRule] {
        &self.rules
    }

    /// Compiled fixed-time signals in dense-index order.
    pub fn signals(&self) -> &[CompiledSignal] {
        &self.signals
    }

    /// Compiled portal demand generators in dense-index order.
    pub fn demand(&self) -> &[CompiledDemand] {
        &self.demand
    }

    /// Compiled pedestrian demand generators in dense-index order.
    pub fn pedestrian_demand(&self) -> &[CompiledPedestrianDemand] {
        &self.pedestrian_demand
    }

    /// Passenger-car profile distributions carried through compilation.
    pub fn profiles(&self) -> &CompiledProfile {
        &self.profiles
    }

    /// Pedestrian profile distributions carried through compilation.
    pub fn pedestrian_profiles(&self) -> &CompiledPedestrianProfile {
        &self.pedestrian_profiles
    }

    /// Population tuning carried through compilation.
    pub fn population(&self) -> &PopulationSource {
        &self.population
    }

    /// Compiled mode-template bundles in dense-index order.
    ///
    /// A version-1 source has none; `compile_v2` populates one bundle per
    /// authored `mode_templates[]` entry.
    pub fn mode_templates(&self) -> &[CompiledModeTemplate] {
        &self.mode_templates
    }

    /// The mode template a compiled vehicle demand source produces, or `None`
    /// when the scenario authored no mode tag (every version-1 demand, and the
    /// version-2 pedestrian demand materialized separately).
    ///
    /// This is the demand-to-mode link the kernel reads to select an agent's
    /// family and profile at spawn; it is parallel to [`Self::demand`] and
    /// indexed the same way, so it is `None` for a version-1 source.
    pub fn demand_mode(&self, id: DemandId) -> Option<ModeTemplateId> {
        self.demand_modes.get(id.index()).copied().flatten()
    }

    /// Compiled continuous-width facilities in dense-index order.
    pub fn facilities(&self) -> &[CompiledFacility] {
        &self.facilities
    }

    /// Compiled facility connectors in dense-index order.
    pub fn facility_connectors(&self) -> &[CompiledFacilityConnector] {
        &self.facility_connectors
    }

    /// Stable string-to-dense-integer mapping for run provenance.
    pub fn id_map(&self) -> &IdMap {
        &self.id_map
    }

    /// Look up a compiled path by dense identifier.
    pub fn path(&self, id: PathId) -> Option<&CompiledPath> {
        self.paths.get(id.index())
    }

    /// Look up a compiled portal by dense identifier.
    pub fn portal(&self, id: PortalId) -> Option<&CompiledPortal> {
        self.portals.get(id.index())
    }

    /// Look up a compiled boundary by dense identifier.
    pub fn boundary(&self, id: BoundaryId) -> Option<&CompiledBoundary> {
        self.boundaries.get(id.index())
    }

    /// Look up a compiled region by dense identifier.
    pub fn region(&self, id: RegionId) -> Option<&CompiledRegion> {
        self.regions.get(id.index())
    }

    /// Look up a compiled movement by dense identifier.
    pub fn movement(&self, id: MovementId) -> Option<&CompiledMovement> {
        self.movements.get(id.index())
    }

    /// Look up a compiled crossing by dense identifier.
    pub fn crossing(&self, id: CrossingId) -> Option<&CompiledCrossing> {
        self.crossings.get(id.index())
    }

    /// Look up a compiled waiting area by dense identifier.
    pub fn waiting_area(&self, id: WaitingAreaId) -> Option<&CompiledWaitingArea> {
        self.waiting_areas.get(id.index())
    }

    /// Look up a compiled pedestrian route by dense identifier.
    pub fn pedestrian_route(&self, id: PedestrianRouteId) -> Option<&CompiledPedestrianRoute> {
        self.pedestrian_routes.get(id.index())
    }

    /// Look up a compiled conflict region by dense identifier.
    pub fn conflict_region(&self, id: ConflictRegionId) -> Option<&CompiledConflictRegion> {
        self.conflict_regions.get(id.index())
    }

    /// Look up a compiled rule by dense identifier.
    pub fn rule(&self, id: RuleId) -> Option<&CompiledRule> {
        self.rules.get(id.index())
    }

    /// Look up a compiled signal by dense identifier.
    pub fn signal(&self, id: SignalId) -> Option<&CompiledSignal> {
        self.signals.get(id.index())
    }

    /// Look up a compiled demand source by dense identifier.
    pub fn demand_by_id(&self, id: DemandId) -> Option<&CompiledDemand> {
        self.demand.get(id.index())
    }

    /// Look up a compiled pedestrian demand source by dense identifier.
    pub fn pedestrian_demand_by_id(
        &self,
        id: PedestrianDemandId,
    ) -> Option<&CompiledPedestrianDemand> {
        self.pedestrian_demand.get(id.index())
    }

    /// Look up a compiled mode-template bundle by dense identifier.
    pub fn mode_template(&self, id: ModeTemplateId) -> Option<&CompiledModeTemplate> {
        self.mode_templates.get(id.index())
    }

    /// Look up a compiled facility by dense identifier.
    pub fn facility(&self, id: FacilityId) -> Option<&CompiledFacility> {
        self.facilities.get(id.index())
    }

    /// Look up a compiled facility connector by dense identifier.
    pub fn facility_connector(
        &self,
        id: FacilityConnectorId,
    ) -> Option<&CompiledFacilityConnector> {
        self.facility_connectors.get(id.index())
    }
}

/// Map each identifier to its dense array index.
fn index_by_id<'a>(ids: impl Iterator<Item = &'a str>) -> HashMap<&'a str, usize> {
    ids.enumerate().map(|(index, id)| (id, index)).collect()
}

/// Clone a sequence of authored identifiers.
fn names<'a>(ids: impl Iterator<Item = &'a String>) -> Vec<String> {
    ids.map(String::clone).collect()
}

/// Compile every authored version-2 mode template, or return every diagnostic.
fn compile_mode_templates(
    templates: &[ModeTemplateSource],
) -> Result<Vec<CompiledModeTemplate>, Vec<Diagnostic>> {
    let mut compiled = Vec::with_capacity(templates.len());
    let mut diagnostics = Vec::new();
    for template in templates {
        match compile_mode_template(template) {
            Ok(bundle) => compiled.push(bundle),
            Err(mut template_diagnostics) => diagnostics.append(&mut template_diagnostics),
        }
    }
    if diagnostics.is_empty() {
        Ok(compiled)
    } else {
        Err(diagnostics)
    }
}

/// Compile the version-2 facilities into their reference-path geometry.
///
/// A facility with a `reference_path` gets a compiled [`CompiledReferencePath`]
/// over that path's vertices; a facility without one exposes region geometry
/// only. Adjacency is attached afterwards by [`attach_facility_adjacency`].
fn compile_facilities(
    source: &ScenarioSourceV2,
    mode_template_index: &HashMap<&str, usize>,
) -> Vec<CompiledFacility> {
    let region_index = index_by_id(source.regions.iter().map(|region| region.id.as_str()));
    let path_index = index_by_id(source.paths.iter().map(|path| path.id.as_str()));
    source
        .facilities
        .iter()
        .enumerate()
        .map(|(index, facility)| {
            let reference = facility.reference_path.as_ref().map(|name| {
                let path = PathId::from_index(path_index[name.as_str()]);
                let points = points_of(&source.paths[path.index()].points);
                CompiledFacilityReference {
                    path,
                    geometry: CompiledReferencePath::from_polyline(&points),
                }
            });
            CompiledFacility {
                id: FacilityId::from_index(index),
                name: facility.id.clone(),
                region: RegionId::from_index(region_index[facility.region.as_str()]),
                reference,
                width_m: facility.width_m,
                nominal_direction: compiled_nominal_direction(facility.nominal_direction),
                access: facility
                    .access
                    .modes
                    .iter()
                    .map(|mode| ModeTemplateId::from_index(mode_template_index[mode.as_str()]))
                    .collect(),
                lateral_use: facility.lateral_use,
                speed_policy: compiled_speed_policy(facility.speed_policy),
                outgoing: Vec::new(),
                incoming: Vec::new(),
                physically_possible: Vec::new(),
            }
        })
        .collect()
}

/// Compile the version-2 facility connectors into dense traversal endpoints.
fn compile_facility_connectors(source: &ScenarioSourceV2) -> Vec<CompiledFacilityConnector> {
    let facility_index = index_by_id(
        source
            .facilities
            .iter()
            .map(|facility| facility.id.as_str()),
    );
    source
        .facility_connectors
        .iter()
        .enumerate()
        .map(|(index, connector)| CompiledFacilityConnector {
            id: FacilityConnectorId::from_index(index),
            name: connector.id.clone(),
            from: FacilityTraversal {
                facility: FacilityId::from_index(facility_index[connector.from.facility.as_str()]),
                direction: connector.from.direction,
            },
            to: FacilityTraversal {
                facility: FacilityId::from_index(facility_index[connector.to.facility.as_str()]),
                direction: connector.to.direction,
            },
        })
        .collect()
}

/// Attach each connector to the facilities it leaves and enters, recording the
/// traversal directions the connector graph makes physically possible.
fn attach_facility_adjacency(
    mut facilities: Vec<CompiledFacility>,
    connectors: &[CompiledFacilityConnector],
) -> Vec<CompiledFacility> {
    for (index, connector) in connectors.iter().enumerate() {
        let id = FacilityConnectorId::from_index(index);
        let from = connector.from();
        let to = connector.to();

        let outgoing = &mut facilities[from.facility().index()];
        outgoing.outgoing.push(id);
        push_direction(&mut outgoing.physically_possible, from.direction());

        let incoming = &mut facilities[to.facility().index()];
        incoming.incoming.push(id);
        push_direction(&mut incoming.physically_possible, to.direction());
    }
    facilities
}

/// Push `direction` unless it is already present, keeping authored order.
fn push_direction(directions: &mut Vec<MovementDirection>, direction: MovementDirection) {
    if !directions.contains(&direction) {
        directions.push(direction);
    }
}

/// Materialize the version-1 reader view the Increment 0 compiler consumes.
///
/// Version 2 does not change compiled behavior in this increment, so a
/// validated version-2 document is mapped back onto the version-1 source shape
/// and compiled by the same code path. Validation guarantees every template and
/// reference used here exists; the `Default` fallbacks only cover profile sets
/// no authored agent references.
fn v2_to_v1_view(source: ScenarioSourceV2) -> ScenarioSource {
    let car = source
        .mode_templates
        .iter()
        .find(|template| template.id == "passenger_car");
    let pedestrian = source
        .mode_templates
        .iter()
        .find(|template| template.id == "pedestrian");

    let profiles = car.map(car_profile_from_template).unwrap_or_default();
    let pedestrian_profiles = pedestrian
        .map(pedestrian_profile_from_template)
        .unwrap_or_default();

    let mut demand = Vec::new();
    let mut pedestrian_demand = Vec::new();
    let mut population = None;
    for entry in &source.demand {
        match &entry.spawn {
            DemandSpawnSource::Rate(rate) => match &rate.choice {
                DemandChoiceSource::Movements(shares) => demand.push(DemandSource {
                    id: entry.id.clone(),
                    portal: rate.portal.clone(),
                    rate_vph: rate.rate_per_hour,
                    routes: shares.clone(),
                }),
                DemandChoiceSource::Routes(shares) => {
                    pedestrian_demand.push(PedestrianDemandSource {
                        id: entry.id.clone(),
                        portal: rate.portal.clone(),
                        rate_pph: rate.rate_per_hour,
                        routes: shares.clone(),
                    });
                }
            },
            DemandSpawnSource::Population(spawn) => {
                let (length_m, width_m) = match car.map(|template| &template.body) {
                    Some(ModeBodySource::Box { length_m, width_m }) => (*length_m, *width_m),
                    _ => (zero_range(), zero_range()),
                };
                population = Some(PopulationSource {
                    vehicle_count: spawn.count,
                    vehicle_speed_mps: spawn.speed_mps,
                    vehicle_spacing_m: spawn.spacing_m,
                    vehicle_length_m: length_m.min,
                    vehicle_width_m: width_m.min,
                });
            }
        }
    }

    ScenarioSource {
        schema_version: source.schema_version,
        id: source.id,
        coordinate_system: source.coordinate_system,
        paths: source.paths,
        portals: source.portals,
        boundaries: source.boundaries,
        regions: source.regions,
        movements: source
            .movements
            .into_iter()
            .map(|movement| MovementSource {
                id: movement.id,
                from: movement.from,
                to: movement.to,
                path: movement.path,
                priority: movement.priority,
                stop_line_m: movement.stop_line_m,
            })
            .collect(),
        crossings: source.crossings,
        waiting_areas: source.waiting_areas,
        pedestrian_routes: source.pedestrian_routes,
        conflict_regions: source.conflict_regions,
        rules: source.rules,
        signals: source.signals,
        demand,
        pedestrian_demand,
        profiles,
        pedestrian_profiles,
        population: population.unwrap_or_default(),
    }
}

/// A zero-width placeholder range for an unreferenced profile slot.
fn zero_range() -> ProfileRangeSource {
    ProfileRangeSource { min: 0.0, max: 0.0 }
}

/// Materialize the passenger-car profile distributions from a version-2 template.
fn car_profile_from_template(template: &ModeTemplateSource) -> ProfileSource {
    let (length_m, width_m) = match &template.body {
        ModeBodySource::Box { length_m, width_m } => (*length_m, *width_m),
        ModeBodySource::Circle { .. } | ModeBodySource::Capsule { .. } => {
            (zero_range(), zero_range())
        }
    };
    ProfileSource {
        speed_mps: profile_param(template, "speed_mps"),
        length_m,
        width_m,
        time_gap_s: profile_param(template, "time_gap_s"),
        max_accel_mps2: profile_param(template, "max_accel_mps2"),
        comfortable_brake_mps2: profile_param(template, "comfortable_brake_mps2"),
        compliance: profile_param(template, "compliance"),
    }
}

/// Materialize the pedestrian profile distributions from a version-2 template.
fn pedestrian_profile_from_template(template: &ModeTemplateSource) -> PedestrianProfileSource {
    let radius_m = match &template.body {
        ModeBodySource::Circle { radius_m } => *radius_m,
        ModeBodySource::Box { .. } | ModeBodySource::Capsule { .. } => zero_range(),
    };
    PedestrianProfileSource {
        radius_m,
        speed_mps: profile_param(template, "speed_mps"),
        compliance: profile_param(template, "compliance"),
    }
}

/// One profile parameter of a mode template that validation has already checked.
fn profile_param(template: &ModeTemplateSource, name: &str) -> ProfileRangeSource {
    *template.profiles.get(name).unwrap_or_else(|| {
        panic!(
            "validated mode template '{}' must declare '{name}'",
            template.id
        )
    })
}

/// Convert authored points to world vectors.
fn points_of(points: &[crate::source::PointSource]) -> Vec<DVec2> {
    points
        .iter()
        .map(|point| DVec2::new(point.x, point.y))
        .collect()
}

/// Compile one fixed-time signal into dense heads and phases.
fn compile_signal(
    id: SignalId,
    signal: &SignalSource,
    movement_index: &HashMap<&str, usize>,
) -> CompiledSignal {
    let heads: Vec<CompiledSignalHead> = signal
        .heads
        .iter()
        .map(|head| CompiledSignalHead {
            name: head.id.clone(),
            movement: MovementId::from_index(movement_index[head.movement.as_str()]),
        })
        .collect();
    let head_index = index_by_id(signal.heads.iter().map(|head| head.id.as_str()));

    let mut start_s = 0.0;
    let mut phases = Vec::with_capacity(signal.phases.len());
    for phase in &signal.phases {
        // Validation guarantees every head appears exactly once per phase.
        let mut colors = vec![SignalColor::Red; heads.len()];
        for state in &phase.states {
            colors[head_index[state.head.as_str()]] = state.color;
        }
        phases.push(CompiledSignalPhase {
            duration_s: phase.duration_s,
            start_s,
            colors,
        });
        start_s += phase.duration_s;
    }

    CompiledSignal {
        id,
        name: signal.id.clone(),
        heads,
        phases,
        cycle_s: start_s,
    }
}

/// Compile one crossing's embedded pedestrian signal into contiguous phases.
fn compile_pedestrian_signal(
    signal: &crate::source::PedestrianSignalSource,
) -> CompiledPedestrianSignal {
    let mut start_s = 0.0;
    let mut phases = Vec::with_capacity(signal.phases.len());
    for phase in &signal.phases {
        phases.push(CompiledPedestrianSignalPhase {
            duration_s: phase.duration_s,
            start_s,
            walk: phase.walk,
        });
        start_s += phase.duration_s;
    }
    CompiledPedestrianSignal {
        phases,
        cycle_s: start_s,
    }
}

fn compile_path(id: PathId, path: &crate::source::PathSource) -> CompiledPath {
    let points: Vec<DVec2> = path
        .points
        .iter()
        .map(|point| DVec2::new(point.x, point.y))
        .collect();

    let mut cumulative = Vec::with_capacity(points.len());
    cumulative.push(0.0);
    for pair in points.windows(2) {
        let previous = *cumulative.last().expect("cumulative is seeded");
        cumulative.push(previous + pair[1].distance(pair[0]));
    }
    let length = *cumulative.last().expect("cumulative is seeded");

    CompiledPath {
        id,
        name: path.id.clone(),
        points,
        cumulative,
        length,
    }
}

fn compile_portal(
    id: PortalId,
    portal: &crate::source::PortalSource,
    path: &CompiledPath,
) -> CompiledPortal {
    let (position, heading) = match portal.end {
        PathEnd::Start => (path.position_at(0.0), path.heading_at(0.0)),
        // Portal headings point inward, along the direction of travel.
        PathEnd::End => (path.position_at(path.length), path.heading_at(path.length)),
    };
    CompiledPortal {
        id,
        name: portal.id.clone(),
        path: path.id,
        end: portal.end,
        position,
        heading,
        width_m: portal.width_m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{parse_scenario_source, parse_scenario_source_v2};

    const WALKING: &str = r#"
    {
      schema_version: 1,
      id: 'walking_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 60.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
      ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
    }
    "#;

    fn walking() -> CompiledScenario {
        let source = parse_scenario_source(WALKING).expect("parses");
        CompiledScenario::compile(source).expect("compiles")
    }

    #[test]
    fn assigns_dense_ids_in_source_order() {
        let scenario = walking();
        assert_eq!(
            scenario.path(PathId::from_index(0)).map(CompiledPath::name),
            Some("guide")
        );
        assert_eq!(
            scenario.id_map().path_name(PathId::from_index(0)),
            Some("guide")
        );
        assert_eq!(
            scenario.id_map().portal_name(PortalId::from_index(0)),
            Some("west_entry")
        );
        assert_eq!(
            scenario.id_map().portal_name(PortalId::from_index(1)),
            Some("east_exit")
        );
    }

    #[test]
    fn caches_arc_length_parameterization() {
        let scenario = walking();
        let path = scenario.path(PathId::from_index(0)).expect("path exists");
        assert!((path.length() - 120.0).abs() < 1e-9);
        assert!((path.position_at(30.0) - DVec2::new(30.0, 0.0)).length() < 1e-9);
        // Beyond the end clamps to the final vertex.
        assert!((path.position_at(1000.0) - DVec2::new(120.0, 0.0)).length() < 1e-9);
        // Before the start clamps to the first vertex.
        assert!((path.position_at(-5.0) - DVec2::new(0.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn creates_portals_at_path_ends_with_inward_headings() {
        let scenario = walking();
        let entry = scenario
            .portal(PortalId::from_index(0))
            .expect("entry exists");
        let exit = scenario
            .portal(PortalId::from_index(1))
            .expect("exit exists");
        assert!((entry.position() - DVec2::new(0.0, 0.0)).length() < 1e-9);
        assert!((exit.position() - DVec2::new(120.0, 0.0)).length() < 1e-9);
        assert!(entry.heading().abs() < 1e-9);
        assert!(exit.heading().abs() < 1e-9);
    }

    #[test]
    fn refuses_to_compile_invalid_source() {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'bad', coordinate_system: { x: 'a', y: 'b' }, \
             paths: [], portals: [ { id: 'a', path: 'missing', end: 'start', width_m: 3.0 } ] }",
        )
        .expect("parses");
        let diagnostics = CompiledScenario::compile(source).expect_err("must reject");
        assert!(!diagnostics.is_empty());
    }

    /// A general four-way layout exercising every Increment 1 primitive.
    const SIGNALIZED: &str = "
    {
      schema_version: 1,
      id: 'four_leg',
      coordinate_system: { x: 'east_m', y: 'north_m' },
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
        { id: 'ns_through', from: 'south', to: 'north', path: 'ns', priority: 1 },
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
    }
    ";

    fn signalized() -> CompiledScenario {
        let source = parse_scenario_source(SIGNALIZED).expect("parses");
        CompiledScenario::compile(source).expect("compiles")
    }

    #[test]
    fn compiles_polygons_with_area_and_centroid() {
        let scenario = signalized();
        assert_eq!(scenario.boundaries().len(), 1);
        let boundary = scenario
            .boundary(BoundaryId::from_index(0))
            .expect("boundary exists");
        assert_eq!(boundary.name(), "world");
        assert!((boundary.polygon().area() - 3600.0).abs() < 1e-9);
        assert!(boundary.polygon().centroid().length() < 1e-9);

        assert_eq!(scenario.regions().len(), 1);
        let region = scenario
            .region(RegionId::from_index(0))
            .expect("region exists");
        assert!((region.polygon().area() - 36.0).abs() < 1e-9);
        assert!(region.polygon().centroid().length() < 1e-9);
    }

    #[test]
    fn compiles_movement_endpoints_and_priority() {
        let scenario = signalized();
        let movement = scenario
            .movement(MovementId::from_index(0))
            .expect("movement exists");
        assert_eq!(movement.from(), PortalId::from_index(0));
        assert_eq!(movement.to(), PortalId::from_index(1));
        assert_eq!(movement.path(), PathId::from_index(0));
        assert_eq!(movement.priority(), 0);
        assert!((movement.entry() - DVec2::new(-20.0, 0.0)).length() < 1e-9);
        assert!((movement.exit() - DVec2::new(20.0, 0.0)).length() < 1e-9);
        assert!(movement.entry_heading().abs() < 1e-9);

        let crossing_movement = scenario
            .movement(MovementId::from_index(1))
            .expect("movement exists");
        assert_eq!(crossing_movement.priority(), 1);
        assert!((crossing_movement.entry_heading() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn compiles_crossings_and_conflict_regions() {
        let scenario = signalized();
        let crossing = scenario
            .crossing(CrossingId::from_index(0))
            .expect("crossing exists");
        assert_eq!(crossing.region(), RegionId::from_index(0));
        assert_eq!(
            crossing.movements(),
            [MovementId::from_index(0), MovementId::from_index(1)]
        );

        let conflict = scenario
            .conflict_region(ConflictRegionId::from_index(0))
            .expect("conflict region exists");
        assert_eq!(
            conflict.movements(),
            [MovementId::from_index(0), MovementId::from_index(1)]
        );
        assert!((conflict.polygon().area() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn compiles_a_pedestrian_signal_with_phase_offsets_and_walk_intervals() {
        let source = parse_scenario_source(
            r#"{
                schema_version: 1, id: 'crossing', coordinate_system: { x: 'east_m', y: 'north_m' },
                paths: [ { id: 'walk', points: [ { x: 0, y: -20 }, { x: 0, y: 20 } ] },
                         { id: 'road', points: [ { x: -20, y: 0 }, { x: 20, y: 0 } ] } ],
                portals: [ { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
                           { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
                           { id: 'west', path: 'road', end: 'start', width_m: 2.0 },
                           { id: 'east', path: 'road', end: 'end', width_m: 2.0 } ],
                regions: [ { id: 'area', points: [
                    { x: 0, y: 0 }, { x: 3, y: 0 }, { x: 3, y: 3 }, { x: 0, y: 3 } ] } ],
                movements: [ { id: 'm', from: 'west', to: 'east', path: 'road', priority: 0 } ],
                crossings: [ { id: 'cross', region: 'area', movements: [ 'm' ],
                    pedestrian_signal: { phases: [
                        { duration_s: 18.0, walk: true },
                        { duration_s: 6.0, walk: false },
                    ] } } ],
                pedestrian_profiles: {
                    radius_m: { min: 0.2, max: 0.2 },
                    speed_mps: { min: 1.2, max: 1.2 },
                    compliance: { min: 0.3, max: 0.7 },
                },
            }"#,
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let signal = scenario
            .crossing(CrossingId::from_index(0))
            .expect("crossing exists")
            .pedestrian_signal()
            .expect("signal exists");
        assert_eq!(signal.phases().len(), 2);
        assert!((signal.cycle_s() - 24.0).abs() < 1e-9);
        assert!((signal.phases()[1].start_s() - 18.0).abs() < 1e-9);
        assert!(signal.phases()[0].walk());
        assert!(!signal.phases()[1].walk());
        // Half-open phases: a boundary tick belongs to the later phase.
        assert_eq!(signal.walk_at(0.0), Some(true));
        assert_eq!(signal.walk_at(17.999), Some(true));
        assert_eq!(signal.walk_at(18.0), Some(false));
        assert_eq!(signal.walk_at(24.0), Some(true));

        let profiles = scenario.pedestrian_profiles();
        assert!((profiles.compliance().min() - 0.3).abs() < 1e-9);
        assert!((profiles.compliance().max() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn a_crossing_without_a_pedestrian_signal_is_uncontrolled() {
        let scenario = signalized();
        assert!(
            scenario
                .crossing(CrossingId::from_index(0))
                .expect("crossing exists")
                .pedestrian_signal()
                .is_none()
        );
    }

    #[test]
    fn compiles_signals_with_cycle_length_and_phase_colors() {
        let scenario = signalized();
        let signal = scenario
            .signal(SignalId::from_index(0))
            .expect("signal exists");
        assert_eq!(signal.heads().len(), 2);
        assert_eq!(signal.phases().len(), 4);
        assert!((signal.cycle_s() - 48.0).abs() < 1e-9);
        assert!((signal.phases()[2].start_s() - 24.0).abs() < 1e-9);
        assert_eq!(signal.phases()[0].color(0), Some(SignalColor::Green));
        assert_eq!(signal.phases()[0].color(1), Some(SignalColor::Red));
        assert_eq!(signal.heads()[0].movement(), MovementId::from_index(0));
        assert_eq!(signal.heads()[1].name(), "ns");
    }

    #[test]
    fn compiles_demand_routes_and_profiles() {
        let source = parse_scenario_source(
            r#"{
                schema_version: 1, id: 'flow', coordinate_system: { x: 'east_m', y: 'north_m' },
                paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] } ],
                portals: [ { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
                           { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 } ],
                movements: [ { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 } ],
                demand: [ { id: 'inflow', portal: 'entry', rate_vph: 720.0,
                    routes: [ { movement: 'through', weight: 3.0 } ] } ],
                profiles: {
                    speed_mps: { min: 10.0, max: 14.0 },
                    length_m: { min: 4.0, max: 5.0 },
                    width_m: { min: 1.8, max: 2.0 },
                    time_gap_s: { min: 1.2, max: 1.8 },
                    max_accel_mps2: { min: 1.5, max: 2.5 },
                    comfortable_brake_mps2: { min: 2.0, max: 3.0 },
                },
            }"#,
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");

        assert_eq!(scenario.demand().len(), 1);
        let demand = scenario
            .demand_by_id(DemandId::from_index(0))
            .expect("demand");
        assert_eq!(demand.name(), "inflow");
        assert_eq!(demand.portal(), PortalId::from_index(0));
        assert!((demand.rate_vph() - 720.0).abs() < 1e-9);
        assert_eq!(demand.routes().len(), 1);
        assert_eq!(demand.routes()[0].movement(), MovementId::from_index(0));
        assert!((demand.routes()[0].weight() - 3.0).abs() < 1e-9);
        assert_eq!(
            scenario.id_map().demand_name(DemandId::from_index(0)),
            Some("inflow")
        );

        let profiles = scenario.profiles();
        assert!((profiles.speed_mps().min() - 10.0).abs() < 1e-9);
        assert!((profiles.speed_mps().sample(0.5) - 12.0).abs() < 1e-9);
        assert!((profiles.time_gap_s().max() - 1.8).abs() < 1e-9);
    }

    #[test]
    fn profile_sampling_clamps_out_of_range_parameters() {
        let range = ProfileRange { min: 4.0, max: 6.0 };
        assert!((range.sample(-1.0) - 4.0).abs() < 1e-9);
        assert!((range.sample(2.0) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn compiles_rules_and_the_id_map() {
        let scenario = signalized();
        let rule = scenario.rule(RuleId::from_index(0)).expect("rule exists");
        assert_eq!(rule.movement(), MovementId::from_index(0));
        assert_eq!(rule.kind(), RuleKind::Signal);
        assert_eq!(rule.signal(), Some(SignalId::from_index(0)));

        let id_map = scenario.id_map();
        assert_eq!(
            id_map.movement_name(MovementId::from_index(1)),
            Some("ns_through")
        );
        assert_eq!(id_map.signal_name(SignalId::from_index(0)), Some("main"));
        assert_eq!(
            id_map.region_name(RegionId::from_index(0)),
            Some("crossing_area")
        );
        assert_eq!(
            id_map.conflict_region_name(ConflictRegionId::from_index(0)),
            Some("center")
        );
        assert_eq!(id_map.rule_name(RuleId::from_index(0)), Some("r_ew"));
        assert_eq!(
            id_map.boundary_name(BoundaryId::from_index(0)),
            Some("world")
        );
        assert_eq!(
            id_map.crossing_name(CrossingId::from_index(0)),
            Some("north_crossing")
        );
    }

    /// A pedestrian route across a road movement, with a waiting area and a
    /// pedestrian demand source.
    const PEDESTRIAN: &str = "
    {
      schema_version: 1,
      id: 'pedestrian_crossing',
      coordinate_system: { x: 'east_m', y: 'north_m' },
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
        routes: [ { route: 'north_crossing', weight: 3.0 } ] } ],
      pedestrian_profiles: { radius_m: { min: 0.2, max: 0.3 },
        speed_mps: { min: 1.0, max: 1.6 } },
    }
    ";

    #[test]
    fn compiles_pedestrian_routes_waiting_areas_and_demand() {
        let source = parse_scenario_source(PEDESTRIAN).expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");

        assert_eq!(scenario.waiting_areas().len(), 1);
        let area = scenario
            .waiting_area(WaitingAreaId::from_index(0))
            .expect("waiting area exists");
        assert_eq!(area.name(), "south_wait");
        assert_eq!(area.region(), RegionId::from_index(1));

        assert_eq!(scenario.pedestrian_routes().len(), 1);
        let route = scenario
            .pedestrian_route(PedestrianRouteId::from_index(0))
            .expect("route exists");
        assert_eq!(route.name(), "north_crossing");
        assert_eq!(route.from(), PortalId::from_index(2));
        assert_eq!(route.to(), PortalId::from_index(3));
        assert_eq!(route.path(), PathId::from_index(1));
        assert_eq!(route.crossings(), [CrossingId::from_index(0)]);
        assert_eq!(route.waiting_areas(), [WaitingAreaId::from_index(0)]);
        assert!((route.entry() - DVec2::new(0.0, -15.0)).length() < 1e-9);
        assert!((route.exit() - DVec2::new(0.0, 15.0)).length() < 1e-9);

        assert_eq!(scenario.pedestrian_demand().len(), 1);
        let demand = scenario
            .pedestrian_demand_by_id(PedestrianDemandId::from_index(0))
            .expect("pedestrian demand exists");
        assert_eq!(demand.name(), "footfall");
        assert_eq!(demand.portal(), PortalId::from_index(2));
        assert!((demand.rate_pph() - 240.0).abs() < 1e-9);
        assert_eq!(demand.routes()[0].route(), PedestrianRouteId::from_index(0));
        assert!((demand.routes()[0].weight() - 3.0).abs() < 1e-9);

        let profiles = scenario.pedestrian_profiles();
        assert!((profiles.radius_m().min() - 0.2).abs() < 1e-9);
        assert!((profiles.speed_mps().sample(0.5) - 1.3).abs() < 1e-9);

        let id_map = scenario.id_map();
        assert_eq!(
            id_map.waiting_area_name(WaitingAreaId::from_index(0)),
            Some("south_wait")
        );
        assert_eq!(
            id_map.pedestrian_route_name(PedestrianRouteId::from_index(0)),
            Some("north_crossing")
        );
        assert_eq!(
            id_map.pedestrian_demand_name(PedestrianDemandId::from_index(0)),
            Some("footfall")
        );
        assert_eq!(id_map.waiting_areas().len(), scenario.waiting_areas().len());
        assert_eq!(
            id_map.pedestrian_routes().len(),
            scenario.pedestrian_routes().len()
        );
        assert_eq!(
            id_map.pedestrian_demand().len(),
            scenario.pedestrian_demand().len()
        );
    }

    /// A version-2 document with two continuous-width facilities joined by a
    /// directed connector.
    const FACILITIES: &str = "
    {
      schema_version: 2,
      id: 'facility_geometry',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'west_centerline', points: [ { x: 0.0, y: 0.0 }, { x: 100.0, y: 0.0 } ] },
        { id: 'east_centerline', points: [ { x: 100.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] },
      ],
      portals: [],
      regions: [
        { id: 'west_band', points: [
          { x: 0.0, y: -1.5 }, { x: 100.0, y: -1.5 },
          { x: 100.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
        { id: 'east_band', points: [
          { x: 100.0, y: -1.5 }, { x: 200.0, y: -1.5 },
          { x: 200.0, y: 1.5 }, { x: 100.0, y: 1.5 } ] },
      ],
      mode_templates: [
        {
          id: 'cycle',
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
        },
      ],
      facilities: [
        { id: 'west_lane', region: 'west_band', reference_path: 'west_centerline',
          width_m: 3.0, nominal_direction: 'forward',
          access: { modes: [ 'cycle' ] }, lateral_use: 'shared',
          speed_policy: { limit_mps: 8.0 } },
        { id: 'east_lane', region: 'east_band', reference_path: 'east_centerline',
          width_m: 3.0, nominal_direction: 'forward',
          access: { modes: [ 'cycle' ] }, lateral_use: 'shared',
          speed_policy: { limit_mps: null } },
      ],
      facility_connectors: [
        { id: 'west_to_east', from: { facility: 'west_lane', direction: 'forward' },
          to: { facility: 'east_lane', direction: 'forward' } },
      ],
    }
    ";

    fn facilities() -> CompiledScenario {
        let source = parse_scenario_source_v2(FACILITIES).expect("facility document parses");
        CompiledScenario::compile_v2(source).expect("facility document compiles")
    }

    #[test]
    fn compiles_facilities_with_reference_coordinates_access_and_connectors() {
        let scenario = facilities();

        assert_eq!(scenario.mode_templates().len(), 1);
        assert_eq!(
            scenario
                .mode_template(ModeTemplateId::from_index(0))
                .map(CompiledModeTemplate::id),
            Some("cycle")
        );

        assert_eq!(scenario.facilities().len(), 2);
        let west = scenario
            .facility(FacilityId::from_index(0))
            .expect("west lane exists");
        assert_eq!(west.name(), "west_lane");
        assert_eq!(west.region(), RegionId::from_index(0));
        assert_eq!(west.reference_path(), Some(PathId::from_index(0)));
        assert_eq!(west.width_m(), 3.0);
        assert_eq!(west.nominal_direction(), NominalDirection::Forward);
        assert_eq!(west.access(), [ModeTemplateId::from_index(0)]);
        assert!(west.permits_mode(ModeTemplateId::from_index(0)));
        assert_eq!(west.lateral_use(), LateralUse::Shared);
        assert_eq!(west.speed_policy().limit_mps(), Some(8.0));

        assert_eq!(west.length(), Some(100.0));
        assert_eq!(west.position_at(25.0), Some(DVec2::new(25.0, 0.0)));
        assert_eq!(west.tangent_at(25.0), Some(DVec2::new(1.0, 0.0)));
        assert_eq!(west.normal_at(25.0), Some(DVec2::new(0.0, 1.0)));
        assert_eq!(west.curvature_at(25.0), Some(0.0));

        let east = scenario
            .facility(FacilityId::from_index(1))
            .expect("east lane exists");
        assert_eq!(east.reference_path(), Some(PathId::from_index(1)));
        assert_eq!(east.speed_policy().limit_mps(), None);

        // The connector is a directed edge in the facility graph.
        assert_eq!(scenario.facility_connectors().len(), 1);
        let connector = scenario
            .facility_connector(FacilityConnectorId::from_index(0))
            .expect("connector exists");
        assert_eq!(connector.name(), "west_to_east");
        assert_eq!(
            connector.from(),
            FacilityTraversal {
                facility: FacilityId::from_index(0),
                direction: MovementDirection::Forward,
            }
        );
        assert_eq!(connector.to().facility(), FacilityId::from_index(1));
        assert_eq!(
            west.outgoing_connectors(),
            [FacilityConnectorId::from_index(0)]
        );
        assert_eq!(
            east.incoming_connectors(),
            [FacilityConnectorId::from_index(0)]
        );

        // The physically possible direction is separate from the nominal one:
        // only the traversals an attached connector actually joins are possible.
        assert!(west.is_physically_possible(MovementDirection::Forward));
        assert!(!west.is_physically_possible(MovementDirection::Reverse));
        assert!(east.is_physically_possible(MovementDirection::Forward));
        assert!(!east.is_physically_possible(MovementDirection::Reverse));

        // The provenance id map names the Increment 1 objects.
        assert_eq!(
            scenario.id_map().facility_name(FacilityId::from_index(0)),
            Some("west_lane")
        );
        assert_eq!(
            scenario
                .id_map()
                .facility_connector_name(FacilityConnectorId::from_index(0)),
            Some("west_to_east")
        );
        assert_eq!(
            scenario.facilities().len(),
            scenario.id_map().facilities().len()
        );
        assert_eq!(
            scenario.facility_connectors().len(),
            scenario.id_map().facility_connectors().len()
        );
    }

    /// Largest `(s, d)` round-trip error in metres over the given route
    /// coordinates, including the reconstructed world distance.
    fn max_round_trip_error(
        reference: &CompiledReferencePath,
        distances: &[f64],
        offsets: &[f64],
    ) -> f64 {
        let mut max = 0.0_f64;
        for &s in distances {
            for &d in offsets {
                let world = reference.point_at(s, d);
                let route = reference.project(world);
                max = max.max((route.s() - s).abs()).max((route.d() - d).abs());
                let reconstructed = reference.point_at(route.s(), route.d());
                max = max.max((reconstructed - world).length());
            }
        }
        max
    }

    /// The checked-in `T-RT` evidence: `(s, d) -> world -> (s, d)` is exact
    /// within `1e-9 m` on a straight facility and on a constant-curvature
    /// facility, and the usable lateral interval subtracts the body envelope
    /// and clearance.
    #[test]
    fn facility_reference_round_trips_path_world_path_within_trt() {
        let scenario = facilities();
        let straight = scenario
            .facility(FacilityId::from_index(0))
            .expect("west lane exists")
            .reference()
            .expect("west lane has a reference path")
            .geometry();

        let straight_error = max_round_trip_error(
            straight,
            &[0.0, 1.0, 25.0, 50.0, 99.0, 100.0],
            &[-1.0, 0.0, 0.5, 1.0],
        );
        assert!(
            straight_error <= 1e-9,
            "straight T-RT error {straight_error} m exceeds 1e-9 m"
        );

        // A constant-curvature facility: a counter-clockwise quarter circle of
        // radius 50 m. A single authored polyline cannot be an exact arc, so
        // the analytic reference is built through the compiled geometry.
        let curved_facility = CompiledFacility {
            id: FacilityId::from_index(0),
            name: "curved".to_owned(),
            region: RegionId::from_index(0),
            reference: Some(CompiledFacilityReference {
                path: PathId::from_index(0),
                geometry: CompiledReferencePath::arc(
                    DVec2::new(0.0, 0.0),
                    50.0,
                    0.0,
                    std::f64::consts::FRAC_PI_2,
                ),
            }),
            width_m: 3.0,
            nominal_direction: NominalDirection::Forward,
            access: vec![ModeTemplateId::from_index(0)],
            lateral_use: LateralUse::Shared,
            speed_policy: SpeedPolicy::unlimited(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
            physically_possible: Vec::new(),
        };
        let geometry = curved_facility
            .reference()
            .expect("the curved facility has a reference path")
            .geometry();
        for s in [0.0, 10.0, 25.0, 50.0, 75.0] {
            assert!(
                (geometry.curvature_at(s) - 1.0 / 50.0).abs() < 1e-12,
                "curvature at s = {s} is the constant 1 / radius"
            );
        }
        let curved_error = max_round_trip_error(
            geometry,
            &[5.0, 20.0, 40.0, 60.0, 75.0],
            &[-2.0, -0.5, 0.0, 0.5, 2.0],
        );
        assert!(
            curved_error <= 1e-9,
            "constant-curvature T-RT error {curved_error} m exceeds 1e-9 m"
        );

        // A clockwise arc reports negative curvature and round-trips too.
        let clockwise = CompiledReferencePath::arc(
            DVec2::new(0.0, 0.0),
            50.0,
            0.0,
            -std::f64::consts::FRAC_PI_2,
        );
        assert!((clockwise.curvature_at(20.0) + 1.0 / 50.0).abs() < 1e-12);
        let clockwise_error = max_round_trip_error(
            &clockwise,
            &[5.0, 20.0, 40.0, 60.0, 75.0],
            &[-2.0, 0.0, 2.0],
        );
        assert!(
            clockwise_error <= 1e-9,
            "clockwise T-RT error {clockwise_error} m exceeds 1e-9 m"
        );

        // The usable lateral interval is the band minus the body envelope and
        // lateral clearance: W/2 - envelope/2 - clearance on each side.
        let interval = curved_facility.usable_lateral_interval(0.6, 0.2);
        assert!((interval.d_min() + 1.0).abs() < 1e-12);
        assert!((interval.d_max() - 1.0).abs() < 1e-12);
        assert!(!interval.is_empty());
        assert!(interval.contains(0.5));
        assert!(!interval.contains(1.5));

        // A band narrower than the envelope plus twice the clearance is empty.
        let too_narrow = curved_facility.usable_lateral_interval(1.2, 1.0);
        assert!(too_narrow.is_empty());
        assert!(!too_narrow.contains(0.0));
    }
}
