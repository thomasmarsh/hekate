//! Backend-agnostic scene projection.
//!
//! A [`SceneFrame`] is a pure projection of a kernel snapshot plus compiled
//! scenario geometry and presentation state. It references no Bevy, window, or
//! terminal type, so every backend consumes exactly the same data and no
//! backend can accidentally read or mutate simulation internals.

use std::sync::Arc;

use glam::DVec2;
use hekate_model::{
    BodyKind, BoundaryId, CompiledScenario, ConflictRegionId, CrossingId, FacilityId, MovementId,
    PathId, PortalId, RegionId, RuleId, RuleKind, SignalId,
};
use hekate_sim::{
    AgentMode, AgentSample, BodySegmentSample, ComplianceDecision, RegionKey, VehicleProfile,
};

use crate::clock::Speed;
use crate::safety::SafetyOverlay;

/// Declared shape version of the shared scene projection.
///
/// Version 1 is the Phase 1 projection: paths, portals, boundaries, regions,
/// movements, crossings, conflict regions, rules, signals, and the body kinds
/// and ordered segments of `[[TAS-072-body-kind-and-segment-presenters]]`.
/// Version 2 adds the compiled facility list, each facility carrying the
/// traversable region it occupies and its reference path, and the capsule body
/// shape of the narrow wheeled modes.
///
/// Nothing serializes a scene, so no artifact records this version; it is the
/// declared name of the projection's shape, and a change to that shape bumps it
/// and regenerates the scene golden under a declared explanation
/// (`docs/body-kind-segment-output.md`).
pub const SCENE_FORMAT_VERSION: u32 = 2;

/// Default rendered body length when a snapshot carries no motion detail.
pub const DEFAULT_BODY_LENGTH_M: f64 = 4.5;
/// Default rendered body width when a snapshot carries no motion detail.
pub const DEFAULT_BODY_WIDTH_M: f64 = 1.8;

/// Closest a click may land to a body and still select it, in screen pixels.
pub const SELECT_RADIUS_PIXELS: f64 = 12.0;

/// View over the world: a world-space centre and a world-units-per-screen-unit
/// scale.
///
/// `scale` matches an orthographic camera's scale: a larger value shows more
/// world and therefore zooms out. Panning moves [`Viewport::center`]; zooming
/// multiplies [`Viewport::scale`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    center: DVec2,
    scale: f64,
}

impl Viewport {
    /// Tightest permitted scale.
    pub const MIN_SCALE: f64 = 0.02;
    /// Loosest permitted scale.
    pub const MAX_SCALE: f64 = 64.0;

    /// A viewport centred on `center` at `scale`, clamped to the scale range.
    pub fn new(center: DVec2, scale: f64) -> Self {
        Self {
            center,
            scale: clamp_scale(scale),
        }
    }

    /// Fit `bounds` into a `screen` measured in pixels, leaving a `margin`
    /// factor of slack (for example `1.25` shows 25% extra world).
    pub fn fit(bounds: (DVec2, DVec2), screen: (f64, f64), margin: f64) -> Self {
        let (min, max) = bounds;
        let center = (min + max) * 0.5;
        let span = (max - min).max(DVec2::splat(1.0));
        let screen = DVec2::new(screen.0.max(1.0), screen.1.max(1.0));
        let scale = (span.x / screen.x).max(span.y / screen.y) * margin.max(f64::MIN_POSITIVE);
        Self::new(center, scale)
    }

    /// World-space centre currently in view.
    pub const fn center(&self) -> DVec2 {
        self.center
    }

    /// World units per screen unit.
    pub const fn scale(&self) -> f64 {
        self.scale
    }

    /// World-space radius corresponding to [`SELECT_RADIUS_PIXELS`].
    pub const fn select_radius(&self) -> f64 {
        SELECT_RADIUS_PIXELS * self.scale
    }

    /// Move the centre by a world-space delta.
    pub fn pan(&mut self, delta: DVec2) {
        self.center += delta;
    }

    /// Multiply the zoom by `factor`, clamped to the supported range.
    pub fn zoom(&mut self, factor: f64) {
        self.scale = clamp_scale(self.scale * factor);
    }
}

fn clamp_scale(scale: f64) -> f64 {
    if scale.is_finite() {
        scale.clamp(Viewport::MIN_SCALE, Viewport::MAX_SCALE)
    } else {
        Viewport::MAX_SCALE
    }
}

/// A guide path as drawn by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenePath {
    id: PathId,
    points: Vec<DVec2>,
}

impl ScenePath {
    /// Dense identifier of the source path.
    pub const fn id(&self) -> PathId {
        self.id
    }

    /// Polyline vertices in order.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }
}

/// A portal as drawn by a backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenePortal {
    id: PortalId,
    position: DVec2,
    heading: f64,
    width_m: f64,
}

impl ScenePortal {
    /// Dense identifier of the source portal.
    pub const fn id(&self) -> PortalId {
        self.id
    }

    /// World position in metres.
    pub const fn position(&self) -> DVec2 {
        self.position
    }

    /// Inward heading in radians, along the direction of travel.
    pub const fn heading(&self) -> f64 {
        self.heading
    }

    /// Traversable width in metres.
    pub const fn width_m(&self) -> f64 {
        self.width_m
    }

    /// The two ends of the gate, perpendicular to the inward heading.
    pub fn gate(&self) -> (DVec2, DVec2) {
        let half_width = self.width_m * 0.5;
        let (sin, cos) = self.heading.sin_cos();
        let offset = DVec2::new(sin, -cos) * half_width;
        (self.position + offset, self.position - offset)
    }

    /// A point `distance` metres into the path from the portal.
    pub fn inward_tip(&self, distance: f64) -> DVec2 {
        let (sin, cos) = self.heading.sin_cos();
        self.position + DVec2::new(cos, sin) * distance
    }
}

/// A boundary polygon as drawn by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneBoundary {
    id: BoundaryId,
    points: Vec<DVec2>,
}

impl SceneBoundary {
    /// Dense identifier of the source boundary.
    pub const fn id(&self) -> BoundaryId {
        self.id
    }

    /// Ring vertices in order; the last connects back to the first.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }
}

/// A traversable region as drawn by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRegion {
    id: RegionId,
    points: Vec<DVec2>,
}

impl SceneRegion {
    /// Dense identifier of the source region.
    pub const fn id(&self) -> RegionId {
        self.id
    }

    /// Ring vertices in order; the last connects back to the first.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }
}

/// The reference path of a facility as drawn by a backend: the authored path
/// the facility's `(s, d)` frame was compiled from, and its polyline.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneFacilityReference {
    path: PathId,
    points: Vec<DVec2>,
}

impl SceneFacilityReference {
    /// Dense identifier of the authored reference path.
    pub const fn path(&self) -> PathId {
        self.path
    }

    /// Polyline vertices of the reference path in order.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }
}

/// A continuous-width facility as drawn by a backend: the traversable region it
/// occupies and, when it declares one, the reference path that gives it its
/// arc-length frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneFacility {
    id: FacilityId,
    region: RegionId,
    points: Vec<DVec2>,
    reference: Option<SceneFacilityReference>,
}

impl SceneFacility {
    /// Dense identifier of the source facility.
    pub const fn id(&self) -> FacilityId {
        self.id
    }

    /// Dense identifier of the region the facility occupies.
    pub const fn region(&self) -> RegionId {
        self.region
    }

    /// Ring vertices of the occupied region in order; the last connects back
    /// to the first.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }

    /// The compiled reference path, absent when the facility declares none.
    pub const fn reference(&self) -> Option<&SceneFacilityReference> {
        self.reference.as_ref()
    }
}

/// A movement connector as drawn by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneMovement {
    id: MovementId,
    points: Vec<DVec2>,
    entry: DVec2,
    exit: DVec2,
}

impl SceneMovement {
    /// Dense identifier of the source movement.
    pub const fn id(&self) -> MovementId {
        self.id
    }

    /// Polyline vertices of the movement's guide path.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }

    /// World position of the entry endpoint in metres.
    pub const fn entry(&self) -> DVec2 {
        self.entry
    }

    /// World position of the exit endpoint in metres.
    pub const fn exit(&self) -> DVec2 {
        self.exit
    }
}

/// A pedestrian crossing as drawn by a backend; its ring is the region it
/// occupies.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneCrossing {
    id: CrossingId,
    points: Vec<DVec2>,
}

impl SceneCrossing {
    /// Dense identifier of the source crossing.
    pub const fn id(&self) -> CrossingId {
        self.id
    }

    /// Ring vertices of the occupied region, in order.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }
}

/// A conflict region as drawn by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneConflictRegion {
    id: ConflictRegionId,
    points: Vec<DVec2>,
}

impl SceneConflictRegion {
    /// Dense identifier of the source conflict region.
    pub const fn id(&self) -> ConflictRegionId {
        self.id
    }

    /// Ring vertices in order; the last connects back to the first.
    pub fn points(&self) -> &[DVec2] {
        &self.points
    }
}

/// A control rule as drawn by a backend: a marker at the governed movement's
/// entry endpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRule {
    id: RuleId,
    kind: RuleKind,
    movement: MovementId,
    position: DVec2,
    heading: f64,
}

impl SceneRule {
    /// Dense identifier of the source rule.
    pub const fn id(&self) -> RuleId {
        self.id
    }

    /// Kind of control the rule applies.
    pub const fn kind(&self) -> RuleKind {
        self.kind
    }

    /// Movement the rule governs.
    pub const fn movement(&self) -> MovementId {
        self.movement
    }

    /// World position of the marker in metres.
    pub const fn position(&self) -> DVec2 {
        self.position
    }

    /// Heading of the marker in radians.
    pub const fn heading(&self) -> f64 {
        self.heading
    }
}

/// One signal head as drawn by a backend, placed at its movement's entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneSignalHead {
    movement: MovementId,
    position: DVec2,
    heading: f64,
}

impl SceneSignalHead {
    /// Movement this head controls.
    pub const fn movement(&self) -> MovementId {
        self.movement
    }

    /// World position of the head in metres.
    pub const fn position(&self) -> DVec2 {
        self.position
    }

    /// Heading of the head in radians.
    pub const fn heading(&self) -> f64 {
        self.heading
    }
}

/// A fixed-time signal controller as drawn by a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneSignal {
    id: SignalId,
    cycle_s: f64,
    heads: Vec<SceneSignalHead>,
}

impl SceneSignal {
    /// Dense identifier of the source signal.
    pub const fn id(&self) -> SignalId {
        self.id
    }

    /// Total cycle length in seconds.
    pub const fn cycle_s(&self) -> f64 {
        self.cycle_s
    }

    /// Heads controlled together, in dense-index order.
    pub fn heads(&self) -> &[SceneSignalHead] {
        &self.heads
    }
}

/// Static geometry a scenario contributes to every frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneGeometry {
    paths: Vec<ScenePath>,
    portals: Vec<ScenePortal>,
    boundaries: Vec<SceneBoundary>,
    regions: Vec<SceneRegion>,
    facilities: Vec<SceneFacility>,
    movements: Vec<SceneMovement>,
    crossings: Vec<SceneCrossing>,
    conflict_regions: Vec<SceneConflictRegion>,
    rules: Vec<SceneRule>,
    signals: Vec<SceneSignal>,
    bounds: Option<(DVec2, DVec2)>,
}

impl SceneGeometry {
    /// Project the drawable geometry out of a compiled scenario.
    pub fn from_scenario(scenario: &CompiledScenario) -> Self {
        let paths: Vec<ScenePath> = scenario
            .paths()
            .iter()
            .map(|path| ScenePath {
                id: path.id(),
                points: path.points().to_vec(),
            })
            .collect();
        let portals: Vec<ScenePortal> = scenario
            .portals()
            .iter()
            .map(|portal| ScenePortal {
                id: portal.id(),
                position: portal.position(),
                heading: portal.heading(),
                width_m: portal.width_m(),
            })
            .collect();
        let boundaries: Vec<SceneBoundary> = scenario
            .boundaries()
            .iter()
            .map(|boundary| SceneBoundary {
                id: boundary.id(),
                points: boundary.polygon().ring().to_vec(),
            })
            .collect();
        let regions: Vec<SceneRegion> = scenario
            .regions()
            .iter()
            .map(|region| SceneRegion {
                id: region.id(),
                points: region.polygon().ring().to_vec(),
            })
            .collect();
        let facilities: Vec<SceneFacility> = scenario
            .facilities()
            .iter()
            .map(|facility| SceneFacility {
                id: facility.id(),
                region: facility.region(),
                points: scenario
                    .region(facility.region())
                    .map_or_else(Vec::new, |region| region.polygon().ring().to_vec()),
                reference: facility
                    .reference_path()
                    .map(|path| SceneFacilityReference {
                        path,
                        points: scenario
                            .path(path)
                            .map_or_else(Vec::new, |path| path.points().to_vec()),
                    }),
            })
            .collect();
        let movements: Vec<SceneMovement> = scenario
            .movements()
            .iter()
            .map(|movement| SceneMovement {
                id: movement.id(),
                points: scenario
                    .path(movement.path())
                    .map_or_else(Vec::new, |path| path.points().to_vec()),
                entry: movement.entry(),
                exit: movement.exit(),
            })
            .collect();
        let crossings: Vec<SceneCrossing> = scenario
            .crossings()
            .iter()
            .map(|crossing| SceneCrossing {
                id: crossing.id(),
                points: scenario
                    .region(crossing.region())
                    .map_or_else(Vec::new, |region| region.polygon().ring().to_vec()),
            })
            .collect();
        let conflict_regions: Vec<SceneConflictRegion> = scenario
            .conflict_regions()
            .iter()
            .map(|conflict| SceneConflictRegion {
                id: conflict.id(),
                points: conflict.polygon().ring().to_vec(),
            })
            .collect();
        let rules: Vec<SceneRule> = scenario
            .rules()
            .iter()
            .map(|rule| {
                let movement = scenario.movement(rule.movement());
                SceneRule {
                    id: rule.id(),
                    kind: rule.kind(),
                    movement: rule.movement(),
                    position: movement.map_or(DVec2::ZERO, |movement| movement.entry()),
                    heading: movement.map_or(0.0, |movement| movement.entry_heading()),
                }
            })
            .collect();
        let signals: Vec<SceneSignal> = scenario
            .signals()
            .iter()
            .map(|signal| SceneSignal {
                id: signal.id(),
                cycle_s: signal.cycle_s(),
                heads: signal
                    .heads()
                    .iter()
                    .map(|head| {
                        let movement = scenario.movement(head.movement());
                        SceneSignalHead {
                            movement: head.movement(),
                            position: movement.map_or(DVec2::ZERO, |movement| movement.entry()),
                            heading: movement.map_or(0.0, |movement| movement.entry_heading()),
                        }
                    })
                    .collect(),
            })
            .collect();

        let mut min = DVec2::splat(f64::INFINITY);
        let mut max = DVec2::splat(f64::NEG_INFINITY);
        let mut any = false;
        let mut include = |point: DVec2, pad: f64| {
            min = min.min(point - DVec2::splat(pad));
            max = max.max(point + DVec2::splat(pad));
            any = true;
        };
        for path in &paths {
            for &point in &path.points {
                include(point, 0.0);
            }
        }
        for portal in &portals {
            include(portal.position, portal.width_m * 0.5);
        }
        for ring in boundaries
            .iter()
            .map(|shape| &shape.points)
            .chain(regions.iter().map(|shape| &shape.points))
            .chain(facilities.iter().map(|shape| &shape.points))
            .chain(crossings.iter().map(|shape| &shape.points))
            .chain(conflict_regions.iter().map(|shape| &shape.points))
        {
            for &point in ring {
                include(point, 0.0);
            }
        }
        for facility in &facilities {
            for point in facility
                .reference
                .iter()
                .flat_map(|reference| &reference.points)
            {
                include(*point, 0.0);
            }
        }
        for movement in &movements {
            for &point in &movement.points {
                include(point, 0.0);
            }
            include(movement.entry, 0.0);
            include(movement.exit, 0.0);
        }
        for rule in &rules {
            include(rule.position, 0.0);
        }
        for signal in &signals {
            for head in &signal.heads {
                include(head.position, 0.0);
            }
        }

        Self {
            paths,
            portals,
            boundaries,
            regions,
            facilities,
            movements,
            crossings,
            conflict_regions,
            rules,
            signals,
            bounds: any.then_some((min, max)),
        }
    }

    /// Guide paths in dense-index order.
    pub fn paths(&self) -> &[ScenePath] {
        &self.paths
    }

    /// Portals in dense-index order.
    pub fn portals(&self) -> &[ScenePortal] {
        &self.portals
    }

    /// Boundary polygons in dense-index order.
    pub fn boundaries(&self) -> &[SceneBoundary] {
        &self.boundaries
    }

    /// Traversable regions in dense-index order.
    pub fn regions(&self) -> &[SceneRegion] {
        &self.regions
    }

    /// Continuous-width facilities in dense-index order.
    pub fn facilities(&self) -> &[SceneFacility] {
        &self.facilities
    }

    /// Movement connectors in dense-index order.
    pub fn movements(&self) -> &[SceneMovement] {
        &self.movements
    }

    /// Pedestrian crossings in dense-index order.
    pub fn crossings(&self) -> &[SceneCrossing] {
        &self.crossings
    }

    /// Conflict regions in dense-index order.
    pub fn conflict_regions(&self) -> &[SceneConflictRegion] {
        &self.conflict_regions
    }

    /// Control rules in dense-index order.
    pub fn rules(&self) -> &[SceneRule] {
        &self.rules
    }

    /// Fixed-time signals in dense-index order.
    pub fn signals(&self) -> &[SceneSignal] {
        &self.signals
    }

    /// World-space bounds of every drawable point, padded by portal width, or
    /// `None` when the scenario has no drawable geometry.
    pub const fn bounds(&self) -> Option<(DVec2, DVec2)> {
        self.bounds
    }

    /// Ring vertices of the region `region` names, or `None` when the scenario
    /// has no such region.
    ///
    /// The two region id spaces are separate, so a crossing key and a conflict
    /// region key with the same dense index name different regions. A consumer
    /// uses this to draw an occupancy overlay over the same ring the safety
    /// layer crossed.
    pub fn region_points(&self, region: RegionKey) -> Option<&[DVec2]> {
        match region {
            RegionKey::Crossing(crossing) => self
                .crossings
                .iter()
                .find(|candidate| candidate.id == crossing)
                .map(|scene| scene.points.as_slice()),
            RegionKey::ConflictRegion(id) => self
                .conflict_regions
                .iter()
                .find(|candidate| candidate.id == id)
                .map(|scene| scene.points.as_slice()),
        }
    }

    /// Centre of the region `region` names: the mean of its ring vertices, or
    /// `None` when the scenario has no such region or the ring is empty.
    ///
    /// The vertex mean is the anchor a marker for a region record sits at. It
    /// is the area centroid for a convex ring and stays inside the ring for
    /// every authored polygon, which is what a marker needs.
    pub fn region_center(&self, region: RegionKey) -> Option<DVec2> {
        let points = self.region_points(region)?;
        if points.is_empty() {
            return None;
        }
        let count = points.len() as f64;
        Some(
            points
                .iter()
                .copied()
                .fold(DVec2::ZERO, |sum, point| sum + point)
                / count,
        )
    }
}

/// One convex piece of a rendered body envelope, in world space.
///
/// A backend draws these without knowing a body's mode or scenario: the shared
/// projection from a [`SceneBody`]'s reported `body_kind` and ordered segments
/// to this list is the only place that decides a body's drawn shape, so a
/// vehicle box, a pedestrian circle, and a narrow wheeled capsule are
/// distinguished by scene data rather than by a presenter branch. A `Box` is an
/// oriented rectangle and a `Circle` its radius, both in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyShape {
    /// A circle of `radius_m` metres centred on `center`.
    Circle {
        /// Circle centre in world metres.
        center: DVec2,
        /// Circle radius in metres.
        radius_m: f64,
    },
    /// An oriented rectangle: `length_m` along its heading, `width_m` across.
    Box {
        /// Rectangle centre in world metres.
        center: DVec2,
        /// Rectangle heading in world radians.
        heading_rad: f64,
        /// Rectangle length along its heading in metres.
        length_m: f64,
        /// Rectangle width across its heading in metres.
        width_m: f64,
    },
    /// A two-dimensional capsule: a straight `length_m` segment along its
    /// heading, capped at both ends by a semicircle of `radius_m`.
    ///
    /// The whole body is `length_m + 2 * radius_m` long and `2 * radius_m`
    /// wide, which is the narrow wheeled modes' reported extent. Its straight
    /// part and its cap radius are independent, so no single rectangle can
    /// draw it and no scaled unit mesh can either; a backend draws the capsule
    /// itself.
    Capsule {
        /// Centre of the straight segment in world metres.
        center: DVec2,
        /// Heading of the straight segment in world radians.
        heading_rad: f64,
        /// Length of the straight segment in metres.
        length_m: f64,
        /// Radius of the semicircular caps in metres.
        radius_m: f64,
    },
}

/// One agent projected into the scene, with interpolation already applied.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneBody {
    /// Stable kernel identifier.
    pub id: usize,
    /// Interpolated world position in metres.
    pub position: DVec2,
    /// Interpolated world heading in radians.
    pub heading_rad: f64,
    /// Rendered body length in metres.
    pub length_m: f64,
    /// Rendered body width in metres.
    pub width_m: f64,
    /// Which mode the body belongs to, so a backend styles a vehicle and a
    /// pedestrian distinctly. A snapshot taken at `SnapshotDetail::Position`
    /// carries no mode, and such a body reads as a vehicle, the same fallback
    /// its body dimensions use.
    pub mode: AgentMode,
    /// Envelope kind of the body: a vehicle is an oriented box, a pedestrian a
    /// circle, and a narrow wheeled agent a capsule. A snapshot without motion
    /// detail reads as a box, matching the vehicle fallback [`Self::mode`] and
    /// the body dimensions use.
    pub body_kind: BodyKind,
    /// Ordered body segments front to back, each with its own interpolated
    /// world pose. Empty for a Phase 1 single-envelope body.
    pub segments: Vec<BodySegmentSample>,
    /// Longitudinal speed in metres per second, when known.
    pub speed_mps: Option<f64>,
    /// The guide path the body follows, when known.
    pub path: Option<PathId>,
    /// Arc-length position along the path in metres, when known.
    pub path_distance_m: Option<f64>,
    /// Assigned route through the network, present for demand-generated
    /// vehicles.
    pub route: Option<MovementId>,
    /// Sampled physical and behavior profile, present for demand-generated
    /// vehicles.
    pub profile: Option<VehicleProfile>,
    /// Most recent signal-compliance decision, when the body is
    /// signal-controlled. Carries only the small recorded reason, not the
    /// controller's internal state.
    pub decision: Option<ComplianceDecision>,
}

impl SceneBody {
    /// Project a kernel sample, interpolating from its previous position.
    pub fn project(previous: &[AgentSample], sample: &AgentSample, alpha: f64) -> Self {
        let (position, heading_rad) = interpolate(previous, sample, alpha);
        let motion = sample.motion.as_ref();
        let (length_m, width_m) = motion
            .map_or((DEFAULT_BODY_LENGTH_M, DEFAULT_BODY_WIDTH_M), |motion| {
                (motion.body_length_m, motion.body_width_m)
            });
        Self {
            id: sample.id.get() as usize,
            position,
            heading_rad,
            length_m,
            width_m,
            mode: motion.map_or(AgentMode::Vehicle, |motion| motion.mode),
            body_kind: motion.map_or(BodyKind::Box, |motion| motion.body_kind),
            segments: project_segments(previous, sample, alpha),
            speed_mps: motion.map(|motion| motion.speed_mps),
            path: motion.map(|motion| motion.path),
            path_distance_m: motion.map(|motion| motion.path_distance_m),
            route: motion.and_then(|motion| motion.route),
            profile: motion.and_then(|motion| motion.profile),
            decision: motion.and_then(|motion| motion.decision),
        }
    }

    /// The ordered convex shapes this body draws, in world space.
    ///
    /// The shape decision is shared by every backend and reads only scene
    /// data. A body with no ordered segments draws one envelope determined by
    /// its reported body kind; a body carrying ordered segments draws one box
    /// per segment at the segment's own pose, so an articulated chain renders
    /// without a mode branch. A scene segment sample carries only a pose, so a
    /// segmented body's authored length is divided evenly across its chain and
    /// its segments draw boxes; a capsule body is a single unsegmented envelope
    /// and keeps its capsule shape.
    pub fn shapes(&self) -> Vec<BodyShape> {
        let segment_count = self.segments.len();
        if segment_count == 0 {
            return vec![self.envelope_shape()];
        }
        let segment_length_m = self.length_m / segment_count as f64;
        self.segments
            .iter()
            .map(|segment| BodyShape::Box {
                center: segment.position,
                heading_rad: segment.heading_rad,
                length_m: segment_length_m,
                width_m: self.width_m,
            })
            .collect()
    }

    /// The single-envelope shape a body without ordered segments draws.
    fn envelope_shape(&self) -> BodyShape {
        match self.body_kind {
            BodyKind::Circle => BodyShape::Circle {
                center: self.position,
                radius_m: self.length_m * 0.5,
            },
            // A capsule's reported width is its diameter, so half of it is the
            // cap radius and the reported length is its straight segment.
            BodyKind::Capsule => BodyShape::Capsule {
                center: self.position,
                heading_rad: self.heading_rad,
                length_m: self.length_m,
                radius_m: self.width_m * 0.5,
            },
            // A box and a segment-less articulated chain draw their reported
            // extent as an oriented rectangle; a body that reports a chain
            // carries its finer shape in its ordered segments, and a scene
            // segment carries only a pose.
            BodyKind::Box | BodyKind::ArticulatedChain => BodyShape::Box {
                center: self.position,
                heading_rad: self.heading_rad,
                length_m: self.length_m,
                width_m: self.width_m,
            },
        }
    }
}

/// Interpolate each body segment's world pose from its previous state.
///
/// A segment is matched to the prior state by its position in the ordered
/// chain. When the chain changed length — a body just spawned or reconfigured —
/// there is no counterpart to blend, so the current poses are used unchanged.
fn project_segments(
    previous: &[AgentSample],
    sample: &AgentSample,
    alpha: f64,
) -> Vec<BodySegmentSample> {
    let Some(motion) = sample.motion.as_ref() else {
        return Vec::new();
    };
    let prior = previous
        .iter()
        .find(|prior| prior.id == sample.id)
        .and_then(|prior| prior.motion.as_ref())
        .filter(|prior| prior.segments.len() == motion.segments.len());
    match prior {
        Some(prior) if alpha > 0.0 => motion
            .segments
            .iter()
            .zip(&prior.segments)
            .map(|(current, prior)| BodySegmentSample {
                position: prior.position.lerp(current.position, alpha),
                heading_rad: lerp_angle(prior.heading_rad, current.heading_rad, alpha),
            })
            .collect(),
        _ => motion.segments.clone(),
    }
}

/// Position and heading to render for `sample`, blended from its previous
/// state by `alpha`.
fn interpolate(previous: &[AgentSample], sample: &AgentSample, alpha: f64) -> (DVec2, f64) {
    if alpha <= 0.0 {
        return (sample.position, sample.heading_rad);
    }
    match previous.iter().find(|prior| prior.id == sample.id) {
        Some(prior) => (
            prior.position.lerp(sample.position, alpha),
            lerp_angle(prior.heading_rad, sample.heading_rad, alpha),
        ),
        None => (sample.position, sample.heading_rad),
    }
}

/// Interpolate headings along the shortest arc so wraparound does not spin.
fn lerp_angle(from: f64, to: f64, alpha: f64) -> f64 {
    let mut delta = (to - from) % std::f64::consts::TAU;
    if delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    } else if delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    from + delta * alpha
}

/// One-line inspector text for what an agent is currently trying to do.
///
/// A body with a sampled profile is driven by the IDM controller in
/// `hekate-sim::control`; a body without one is the static walking-skeleton
/// population at its configured constant speed. The presentation layer owns
/// this wording so every backend describes the same intent.
pub fn intent_summary(profile: Option<VehicleProfile>) -> &'static str {
    if profile.is_some() {
        "follow IDM under sampled profile bounds"
    } else {
        "hold constant speed along the guide path"
    }
}

/// One-line inspector text for an agent's sampled physical and behavior
/// profile; `None` means the body has no sampled profile.
pub fn profile_summary(profile: Option<VehicleProfile>) -> String {
    let Some(profile) = profile else {
        return "none (static population)".to_owned();
    };
    format!(
        "v0 {speed:.2} m/s   T {gap:.2} s   a_max {accel:.2} m/s²   \
         b {brake:.2} m/s²   size {length:.2} x {width:.2} m   compliance {compliance:.2}",
        speed = profile.desired_speed_mps,
        gap = profile.time_gap_s,
        accel = profile.max_accel_mps2,
        brake = profile.comfortable_brake_mps2,
        length = profile.length_m,
        width = profile.width_m,
        compliance = profile.compliance,
    )
}

/// One-line inspector text for an agent's most recent signal-compliance
/// decision; `None` means the movement is not signal-controlled.
///
/// The presentation layer owns this wording so every backend shows the same
/// decision reason for the same record.
pub fn decision_summary(decision: Option<ComplianceDecision>) -> String {
    let Some(decision) = decision else {
        return "none (movement is not signal-controlled)".to_owned();
    };
    let action = match decision.action {
        hekate_sim::SignalAction::Stop => "stop",
        hekate_sim::SignalAction::Proceed => "proceed",
    };
    format!(
        "{action} ({reason})   head {color}   required {required:.2} m/s²   gap {gap:.2} m",
        reason = decision.reason.label(),
        color = decision.color.label(),
        required = decision.required_decel_mps2,
        gap = decision.stop_line_gap_m,
    )
}

/// A debug overlay a backend may draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Scenario geometry: paths and portals.
    Geometry,
    /// Per-agent velocity vectors.
    Vectors,
    /// Safety markers, body emphasis, and region occupancy.
    Safety,
}

/// Which optional debug overlays are enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlays {
    /// Draw scenario geometry.
    pub geometry: bool,
    /// Draw per-agent velocity vectors.
    pub vectors: bool,
    /// Draw the frame's safety overlays: event markers, emphasized bodies, and
    /// occupied regions.
    pub safety: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            geometry: true,
            vectors: false,
            // Safety markers last a couple of simulated seconds, so a run has
            // them on by default: seeing a conflict is the point of watching.
            safety: true,
        }
    }
}

impl Overlays {
    /// Flip one overlay.
    pub fn toggle(&mut self, overlay: Overlay) {
        match overlay {
            Overlay::Geometry => self.geometry = !self.geometry,
            Overlay::Vectors => self.vectors = !self.vectors,
            Overlay::Safety => self.safety = !self.safety,
        }
    }
}

/// Frame-level summary a backend shows on a status line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameStatus {
    /// Number of live agents in the frame.
    pub agents: usize,
    /// Current playback speed.
    pub speed: Speed,
    /// Whether playback is paused.
    pub paused: bool,
    /// Currently selected agent, if any.
    pub selection: Option<usize>,
}

/// One backend-agnostic frame of the presentation layer.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneFrame {
    /// Authored scenario identifier, for provenance.
    pub scenario_id: String,
    /// Simulated seconds at the projected tick.
    pub time_seconds: f64,
    /// Completed kernel ticks.
    pub tick: u64,
    /// Frame-level summary.
    pub status: FrameStatus,
    /// Current viewport.
    pub viewport: Viewport,
    /// Static scenario geometry, shared cheaply across frames.
    pub geometry: Arc<SceneGeometry>,
    /// Live bodies with interpolation applied.
    pub bodies: Vec<SceneBody>,
    /// Which overlays are enabled.
    pub overlays: Overlays,
    /// Safety records and folded states projected for this frame; every safety
    /// overlay is derived from this frame alone.
    pub safety: SafetyOverlay,
}

impl SceneFrame {
    /// The projected body for `id`, if it is alive in this frame.
    pub fn body(&self, id: usize) -> Option<&SceneBody> {
        self.bodies.iter().find(|body| body.id == id)
    }

    /// The currently selected body, if it is alive in this frame.
    pub fn selected_body(&self) -> Option<&SceneBody> {
        self.status.selection.and_then(|id| self.body(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hekate_model::{parse_scenario_source, parse_scenario_source_v2};

    fn walking() -> CompiledScenario {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 60, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
             population: { vehicle_count: 1, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    #[test]
    fn geometry_bounds_cover_paths_and_portals() {
        let geometry = SceneGeometry::from_scenario(&walking());
        let (min, max) = geometry.bounds().expect("geometry has bounds");
        assert_eq!(min, DVec2::new(-2.0, -2.0));
        assert_eq!(max, DVec2::new(122.0, 2.0));
        assert_eq!(geometry.paths().len(), 1);
        assert_eq!(geometry.portals().len(), 2);
    }

    #[test]
    fn portal_gate_is_perpendicular_to_heading() {
        let geometry = SceneGeometry::from_scenario(&walking());
        let (start, end) = geometry.portals()[0].gate();
        // The path runs east, so the gate runs north-south and spans its width.
        assert!((start.x - end.x).abs() < 1e-12);
        assert!((start.y - end.y).abs() - 4.0 < 1e-12);
    }

    #[test]
    fn viewport_fit_centres_bounds_and_leaves_margin() {
        let viewport = Viewport::fit(
            (DVec2::new(0.0, 0.0), DVec2::new(120.0, 40.0)),
            (1200.0, 400.0),
            1.25,
        );
        assert_eq!(viewport.center(), DVec2::new(60.0, 20.0));
        // 40 world metres over 400 screen pixels is 0.1; 25% margin => 0.125.
        assert!((viewport.scale() - 0.125).abs() < 1e-12);
    }

    #[test]
    fn viewport_pan_and_zoom_are_clamped() {
        let mut viewport = Viewport::new(DVec2::ZERO, 1.0);
        viewport.pan(DVec2::new(3.0, -4.0));
        assert_eq!(viewport.center(), DVec2::new(3.0, -4.0));
        viewport.zoom(0.5);
        assert!((viewport.scale() - 0.5).abs() < 1e-12);
        viewport.zoom(1.0e12);
        assert!((viewport.scale() - Viewport::MAX_SCALE).abs() < 1e-12);
        viewport.zoom(0.0);
        assert!((viewport.scale() - Viewport::MIN_SCALE).abs() < 1e-12);
    }

    #[test]
    fn a_body_projects_the_mode_it_was_sampled_with() {
        let vehicle = sample(0, AgentMode::Vehicle);
        let pedestrian = sample(1, AgentMode::Pedestrian);
        assert_eq!(
            SceneBody::project(&[], &vehicle, 0.0).mode,
            AgentMode::Vehicle
        );
        assert_eq!(
            SceneBody::project(&[], &pedestrian, 0.0).mode,
            AgentMode::Pedestrian
        );
        assert_eq!(AgentMode::Vehicle.label(), "vehicle");
        assert_eq!(AgentMode::Pedestrian.label(), "pedestrian");

        // A snapshot without motion detail carries no mode, so the body keeps
        // the vehicle fallback its dimensions also use.
        let coarse = AgentSample {
            motion: None,
            ..vehicle
        };
        let body = SceneBody::project(&[], &coarse, 0.0);
        assert_eq!(body.mode, AgentMode::Vehicle);
        assert_eq!(body.length_m, DEFAULT_BODY_LENGTH_M);
    }

    /// Every projected body carries its envelope kind: a Phase 1 vehicle is a
    /// box and a pedestrian a circle, and a Phase 1 single envelope carries no
    /// ordered segments.
    #[test]
    fn a_body_projects_the_body_kind_and_no_phase1_segments() {
        let vehicle = SceneBody::project(&[], &sample(0, AgentMode::Vehicle), 0.0);
        assert_eq!(vehicle.body_kind, BodyKind::Box);
        assert!(vehicle.segments.is_empty());

        let pedestrian = SceneBody::project(&[], &sample(1, AgentMode::Pedestrian), 0.0);
        assert_eq!(pedestrian.body_kind, BodyKind::Circle);
        assert!(pedestrian.segments.is_empty());

        // A snapshot without motion detail falls back to the vehicle box, the
        // same fallback its mode and dimensions use.
        let coarse = AgentSample {
            motion: None,
            ..sample(2, AgentMode::Pedestrian)
        };
        assert_eq!(
            SceneBody::project(&[], &coarse, 0.0).body_kind,
            BodyKind::Box
        );
    }

    /// The shared shape decision maps a body kind and its ordered segments to
    /// drawable shapes: a box body to a box, a circle body to its inscribed
    /// circle, and a segmented body to one box per segment at its own pose.
    #[test]
    fn body_shapes_follow_kind_and_ordered_segments() {
        let vehicle = SceneBody::project(&[], &sample(0, AgentMode::Vehicle), 0.0);
        assert_eq!(
            vehicle.shapes(),
            vec![BodyShape::Box {
                center: DVec2::ZERO,
                heading_rad: 0.0,
                length_m: 0.5,
                width_m: 0.5,
            }]
        );

        let pedestrian = SceneBody::project(&[], &sample(1, AgentMode::Pedestrian), 0.0);
        assert_eq!(
            pedestrian.shapes(),
            vec![BodyShape::Circle {
                center: DVec2::ZERO,
                radius_m: 0.25,
            }]
        );

        // A segmented body draws one box per segment at the segment's own pose,
        // its authored length split evenly across the ordered chain.
        let mut segmented = sample(2, AgentMode::Vehicle);
        {
            let motion = segmented.motion.as_mut().expect("motion");
            motion.body_kind = BodyKind::ArticulatedChain;
            motion.body_length_m = 6.0;
            motion.body_width_m = 2.0;
            motion.segments = vec![
                BodySegmentSample {
                    position: DVec2::new(0.0, 0.0),
                    heading_rad: 0.0,
                },
                BodySegmentSample {
                    position: DVec2::new(4.0, 1.0),
                    heading_rad: 0.5,
                },
            ];
        }
        assert_eq!(
            SceneBody::project(&[], &segmented, 0.0).shapes(),
            vec![
                BodyShape::Box {
                    center: DVec2::new(0.0, 0.0),
                    heading_rad: 0.0,
                    length_m: 3.0,
                    width_m: 2.0,
                },
                BodyShape::Box {
                    center: DVec2::new(4.0, 1.0),
                    heading_rad: 0.5,
                    length_m: 3.0,
                    width_m: 2.0,
                },
            ]
        );
    }

    /// A segment's world pose is blended from the prior frame at the same
    /// position in the ordered chain.
    #[test]
    fn segment_poses_interpolate_from_the_previous_frame() {
        let mut before = sample(0, AgentMode::Vehicle);
        let mut after = sample(0, AgentMode::Vehicle);
        before.motion.as_mut().expect("motion").segments = vec![BodySegmentSample {
            position: DVec2::ZERO,
            heading_rad: 0.0,
        }];
        after.motion.as_mut().expect("motion").segments = vec![BodySegmentSample {
            position: DVec2::new(10.0, 0.0),
            heading_rad: 1.0,
        }];

        let body = SceneBody::project(std::slice::from_ref(&before), &after, 0.5);
        assert_eq!(
            body.segments,
            vec![BodySegmentSample {
                position: DVec2::new(5.0, 0.0),
                heading_rad: 0.5,
            }]
        );
    }

    /// The shared envelope decision draws a capsule for a capsule body: its
    /// reported length is the straight segment and half its reported width the
    /// cap radius, so a narrow wheeled agent renders as a capsule rather than
    /// the rectangle that bounds it.
    #[test]
    fn a_capsule_body_draws_its_capsule() {
        let mut capsule = sample(0, AgentMode::Vehicle);
        {
            let motion = capsule.motion.as_mut().expect("motion");
            motion.body_kind = BodyKind::Capsule;
            motion.body_length_m = 1.8;
            motion.body_width_m = 0.7;
        }
        assert_eq!(
            SceneBody::project(&[], &capsule, 0.0).shapes(),
            vec![BodyShape::Capsule {
                center: DVec2::ZERO,
                heading_rad: 0.0,
                length_m: 1.8,
                radius_m: 0.35,
            }]
        );
    }

    /// One observed sample with motion detail, so a body projects a mode.
    fn sample(id: usize, mode: AgentMode) -> AgentSample {
        use hekate_sim::MotionSample;
        AgentSample {
            id: hekate_sim::AgentId::from_index(id),
            position: DVec2::ZERO,
            heading_rad: 0.0,
            motion: Some(MotionSample {
                body_kind: mode.body_kind(),
                segments: Vec::new(),
                mode,
                speed_mps: 1.0,
                path: PathId::from_index(0),
                path_distance_m: 0.0,
                body_length_m: 0.5,
                body_width_m: 0.5,
                route: None,
                profile: None,
                pedestrian_route: None,
                pedestrian_profile: None,
                decision: None,
                pedestrian_decision: None,
                yield_crossing: None,
                route_state: None,
            }),
        }
    }

    #[test]
    fn a_region_key_resolves_in_its_own_id_space() {
        use hekate_sim::RegionKey;

        let geometry = SceneGeometry::from_scenario(&signalized());
        let crossing = RegionKey::Crossing(CrossingId::from_index(0));
        let conflict = RegionKey::ConflictRegion(ConflictRegionId::from_index(0));
        // The crossing sits on the authored 6x6 region, the conflict region on
        // the authored 2x2 one: one dense index, two different rings.
        assert_eq!(
            geometry.region_points(crossing),
            Some(&geometry.regions()[0].points[..])
        );
        assert_eq!(
            geometry.region_points(conflict),
            Some(&geometry.conflict_regions()[0].points[..])
        );
        assert_ne!(
            geometry.region_points(crossing),
            geometry.region_points(conflict)
        );
        assert_eq!(geometry.region_center(crossing), Some(DVec2::ZERO));
        assert_eq!(geometry.region_center(conflict), Some(DVec2::ZERO));
        assert_eq!(
            geometry.region_points(RegionKey::Crossing(CrossingId::from_index(9))),
            None
        );
        assert_eq!(
            geometry.region_center(RegionKey::ConflictRegion(ConflictRegionId::from_index(9))),
            None
        );
    }

    #[test]
    fn overlays_toggle_independently() {
        let mut overlays = Overlays::default();
        assert!(overlays.geometry);
        assert!(!overlays.vectors);
        assert!(overlays.safety);
        overlays.toggle(Overlay::Geometry);
        overlays.toggle(Overlay::Vectors);
        overlays.toggle(Overlay::Safety);
        assert!(!overlays.geometry);
        assert!(overlays.vectors);
        assert!(!overlays.safety);
    }

    #[test]
    fn interpolation_uses_shortest_arc() {
        let previous = AgentSample {
            id: hekate_sim::AgentId::from_index(0),
            position: DVec2::ZERO,
            heading_rad: 3.0,
            motion: None,
        };
        let sample = AgentSample {
            id: hekate_sim::AgentId::from_index(0),
            position: DVec2::new(10.0, 0.0),
            heading_rad: -3.0,
            motion: None,
        };
        let body = SceneBody::project(std::slice::from_ref(&previous), &sample, 0.5);
        assert_eq!(body.position, DVec2::new(5.0, 0.0));
        // Wrapping across pi must not rotate the long way around.
        assert!(body.heading_rad.abs() > 3.0);
    }

    #[test]
    fn intent_and_profile_summaries_follow_the_sampled_profile() {
        // The static population carries no sampled profile, so it keeps the
        // constant-speed wording; a demand vehicle reports IDM and its values.
        assert_eq!(
            intent_summary(None),
            "hold constant speed along the guide path"
        );
        assert_eq!(profile_summary(None), "none (static population)");

        let profile = VehicleProfile {
            desired_speed_mps: 12.5,
            length_m: 4.25,
            width_m: 1.9,
            time_gap_s: 1.4,
            max_accel_mps2: 2.2,
            comfortable_brake_mps2: 3.1,
            compliance: 0.4,
        };
        assert_eq!(
            intent_summary(Some(profile)),
            "follow IDM under sampled profile bounds"
        );
        let summary = profile_summary(Some(profile));
        assert!(summary.contains("v0 12.50 m/s"), "{summary}");
        assert!(summary.contains("T 1.40 s"), "{summary}");
        assert!(summary.contains("a_max 2.20 m/s²"), "{summary}");
        assert!(summary.contains("b 3.10 m/s²"), "{summary}");
        assert!(summary.contains("size 4.25 x 1.90 m"), "{summary}");
        assert!(summary.contains("compliance 0.40"), "{summary}");
    }

    #[test]
    fn decision_summary_names_the_action_reason_and_head() {
        use hekate_model::SignalColor;
        use hekate_sim::{ComplianceReason, SignalAction};

        assert_eq!(
            decision_summary(None),
            "none (movement is not signal-controlled)"
        );
        let summary = decision_summary(Some(hekate_sim::ComplianceDecision {
            action: SignalAction::Proceed,
            reason: ComplianceReason::NonCompliantRun,
            color: SignalColor::Red,
            stop_line_gap_m: 12.5,
            required_decel_mps2: 1.25,
        }));
        assert!(summary.contains("proceed (noncompliant run)"), "{summary}");
        assert!(summary.contains("head red"), "{summary}");
        assert!(summary.contains("required 1.25 m/s²"), "{summary}");
        assert!(summary.contains("gap 12.50 m"), "{summary}");
    }

    fn signalized() -> CompiledScenario {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'four_leg', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'ew', points: [ { x: -20, y: 0 }, { x: 20, y: 0 } ] }, \
             { id: 'ns', points: [ { x: 0, y: -20 }, { x: 0, y: 20 } ] } ], \
             portals: [ { id: 'west', path: 'ew', end: 'start', width_m: 3.5 }, \
             { id: 'east', path: 'ew', end: 'end', width_m: 3.5 }, \
             { id: 'south', path: 'ns', end: 'start', width_m: 3.5 }, \
             { id: 'north', path: 'ns', end: 'end', width_m: 3.5 } ], \
             boundaries: [ { id: 'world', points: [ { x: -30, y: -30 }, { x: 30, y: -30 }, \
             { x: 30, y: 30 }, { x: -30, y: 30 } ] } ], \
             regions: [ { id: 'area', points: [ { x: -3, y: -3 }, { x: 3, y: -3 }, \
             { x: 3, y: 3 }, { x: -3, y: 3 } ] } ], \
             movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'ew', priority: 0 }, \
             { id: 'ns_through', from: 'south', to: 'north', path: 'ns', priority: 1 } ], \
             crossings: [ { id: 'cross', region: 'area', movements: [ 'ew_through' ] } ], \
             conflict_regions: [ { id: 'center', points: [ { x: -1, y: -1 }, { x: 1, y: -1 }, \
             { x: 1, y: 1 }, { x: -1, y: 1 } ], movements: [ 'ew_through', 'ns_through' ] } ], \
             rules: [ { id: 'r_ew', movement: 'ew_through', kind: 'signal', signal: 'main' } ], \
             signals: [ { id: 'main', heads: [ { id: 'ew', movement: 'ew_through' } ], \
             phases: [ { duration_s: 20.0, states: [ { head: 'ew', color: 'green' } ] } ] } ] }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    /// A phase-1 scenario contributes no facilities, so the scene still
    /// projects the Phase 1 geometry it always did.
    #[test]
    fn a_phase_1_scenario_projects_no_facilities() {
        assert_eq!(SceneGeometry::from_scenario(&walking()).facilities(), []);
        assert_eq!(walking().schema_version(), 1);
    }

    #[test]
    fn geometry_projects_every_general_primitive() {
        let geometry = SceneGeometry::from_scenario(&signalized());
        assert_eq!(geometry.boundaries().len(), 1);
        assert_eq!(geometry.boundaries()[0].points().len(), 4);
        assert_eq!(geometry.regions().len(), 1);
        assert_eq!(geometry.movements().len(), 2);
        assert_eq!(geometry.movements()[0].points().len(), 2);
        assert_eq!(geometry.crossings().len(), 1);
        assert_eq!(geometry.crossings()[0].points().len(), 4);
        assert_eq!(geometry.conflict_regions().len(), 1);
        assert_eq!(geometry.rules().len(), 1);
        assert_eq!(geometry.rules()[0].kind(), RuleKind::Signal);
        assert_eq!(geometry.signals().len(), 1);
        assert_eq!(geometry.signals()[0].heads().len(), 1);
        assert!((geometry.signals()[0].cycle_s() - 20.0).abs() < 1e-9);
        // World bounds cover the authored boundary ring.
        let (min, max) = geometry.bounds().expect("bounds");
        assert_eq!(min, DVec2::new(-30.0, -30.0));
        assert_eq!(max, DVec2::new(30.0, 30.0));
    }

    /// The declared scene format version names the shape this projection
    /// produces; a change to the shape bumps it under a declared explanation
    /// (`docs/body-kind-segment-output.md`).
    #[test]
    fn the_declared_scene_format_version_is_the_facility_extension() {
        assert_eq!(SCENE_FORMAT_VERSION, 2);
    }

    /// A version-2 facility projects as the traversable region it occupies plus
    /// its authored reference path, so a backend draws the band and its
    /// centreline from scene data alone.
    #[test]
    fn a_facility_projects_its_region_and_reference_path() {
        let geometry = SceneGeometry::from_scenario(&facility());
        assert_eq!(geometry.facilities().len(), 1);
        let projected = &geometry.facilities()[0];
        assert_eq!(projected.id(), FacilityId::from_index(0));
        assert_eq!(projected.region(), RegionId::from_index(0));
        assert_eq!(projected.points(), geometry.regions()[0].points());
        let reference = projected
            .reference()
            .expect("the facility declares a reference path");
        assert_eq!(reference.path(), PathId::from_index(0));
        assert_eq!(reference.points(), geometry.paths()[0].points());
        // The reference path runs the length of the band, so its vertices fall
        // inside the projected bounds.
        let (min, max) = geometry.bounds().expect("bounds");
        assert!(min.x <= 0.0 && max.x >= 100.0);
    }

    /// A version-2 document with one continuous-width facility: a 3 m band over
    /// its own reference path.
    fn facility() -> CompiledScenario {
        const DOCUMENT: &str = "
        {
          schema_version: 2,
          id: 'facility_geometry',
          coordinate_system: { x: 'east_m', y: 'north_m' },
          paths: [
            { id: 'centerline', points: [ { x: 0.0, y: 0.0 }, { x: 100.0, y: 0.0 } ] },
          ],
          portals: [],
          regions: [
            { id: 'band', points: [ { x: 0.0, y: -1.5 }, { x: 100.0, y: -1.5 },
              { x: 100.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
          ],
          mode_templates: [
            {
              id: 'cycle',
              body: { kind: 'capsule', length_m: { min: 1.1, max: 1.1 },
                radius_m: { min: 0.3, max: 0.3 } },
              motion: 'single_body_wheeled',
              tactics: [ 'follow', 'stop', 'yield' ],
              access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
                speed_policy: { limit_mps: null } },
              occupancy: 'operator_only',
              profiles: {
                speed_mps: { min: 5.0, max: 5.0 },
                max_accel_mps2: { min: 1.2, max: 1.2 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                lateral_clearance_m: { min: 0.3, max: 0.3 },
                compliance: { min: 1.0, max: 1.0 },
              },
            },
          ],
          facilities: [
            { id: 'lane', region: 'band', reference_path: 'centerline',
              width_m: 3.0, nominal_direction: 'forward',
              access: { modes: [ 'cycle' ] }, lateral_use: 'shared',
              speed_policy: { limit_mps: null } },
          ],
        }
        ";
        let source = parse_scenario_source_v2(DOCUMENT).expect("facility document parses");
        CompiledScenario::compile_v2(source).expect("facility document compiles")
    }
}
