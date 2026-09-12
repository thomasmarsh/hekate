//! Immutable compiled scenario and dense identifiers.
//!
//! Compilation is the boundary between the authored document and the hot
//! kernel. Authored objects keep stable string identifiers everywhere the
//! document is edited, viewed, or reported; the kernel indexes dense integer
//! arrays instead. The string-to-integer mapping is preserved in
//! [`IdMap`] so run provenance can name any identifier the kernel carries.

use std::collections::HashMap;

use glam::DVec2;

use crate::source::{
    PathEnd, PopulationSource, RuleKind, ScenarioSource, SignalColor, SignalSource,
};
use crate::validate::{Diagnostic, validate};

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

/// Stable string identifiers in dense-index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMap {
    paths: Vec<String>,
    portals: Vec<String>,
    boundaries: Vec<String>,
    regions: Vec<String>,
    movements: Vec<String>,
    crossings: Vec<String>,
    conflict_regions: Vec<String>,
    rules: Vec<String>,
    signals: Vec<String>,
    demand: Vec<String>,
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

/// An inclusive uniform profile range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileRange {
    min: f64,
    max: f64,
}

impl ProfileRange {
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
    conflict_regions: Vec<CompiledConflictRegion>,
    rules: Vec<CompiledRule>,
    signals: Vec<CompiledSignal>,
    demand: Vec<CompiledDemand>,
    profiles: CompiledProfile,
    population: PopulationSource,
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

        // Validation guarantees these lookups succeed, so a missing name is a
        // programming error rather than a user diagnostic.
        let path_index = index_by_id(source.paths.iter().map(|path| path.id.as_str()));
        let portal_index = index_by_id(source.portals.iter().map(|portal| portal.id.as_str()));
        let region_index = index_by_id(source.regions.iter().map(|region| region.id.as_str()));
        let movement_index =
            index_by_id(source.movements.iter().map(|movement| movement.id.as_str()));
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

        let id_map = IdMap {
            paths: names(source.paths.iter().map(|path| &path.id)),
            portals: names(source.portals.iter().map(|portal| &portal.id)),
            boundaries: names(source.boundaries.iter().map(|boundary| &boundary.id)),
            regions: names(source.regions.iter().map(|region| &region.id)),
            movements: names(source.movements.iter().map(|movement| &movement.id)),
            crossings: names(source.crossings.iter().map(|crossing| &crossing.id)),
            conflict_regions: names(source.conflict_regions.iter().map(|conflict| &conflict.id)),
            rules: names(source.rules.iter().map(|rule| &rule.id)),
            signals: names(source.signals.iter().map(|signal| &signal.id)),
            demand: names(source.demand.iter().map(|demand| &demand.id)),
        };

        Ok(Self {
            id: source.id,
            schema_version: source.schema_version,
            paths,
            portals,
            boundaries,
            regions,
            movements,
            crossings,
            conflict_regions,
            rules,
            signals,
            demand,
            profiles: CompiledProfile::from_source(source.profiles),
            population: source.population,
            id_map,
        })
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

    /// Passenger-car profile distributions carried through compilation.
    pub fn profiles(&self) -> &CompiledProfile {
        &self.profiles
    }

    /// Population tuning carried through compilation.
    pub fn population(&self) -> &PopulationSource {
        &self.population
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
}

/// Map each identifier to its dense array index.
fn index_by_id<'a>(ids: impl Iterator<Item = &'a str>) -> HashMap<&'a str, usize> {
    ids.enumerate().map(|(index, id)| (id, index)).collect()
}

/// Clone a sequence of authored identifiers.
fn names<'a>(ids: impl Iterator<Item = &'a String>) -> Vec<String> {
    ids.map(String::clone).collect()
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
    use crate::source::parse_scenario_source;

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
}
