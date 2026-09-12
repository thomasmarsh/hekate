//! Backend-agnostic scene projection.
//!
//! A [`SceneFrame`] is a pure projection of a kernel snapshot plus compiled
//! scenario geometry and presentation state. It references no Bevy, window, or
//! terminal type, so every backend consumes exactly the same data and no
//! backend can accidentally read or mutate simulation internals.

use std::sync::Arc;

use glam::DVec2;
use tangle_model::{CompiledScenario, PathId, PortalId};
use tangle_sim::AgentSample;

use crate::clock::Speed;

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

/// Static geometry a scenario contributes to every frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneGeometry {
    paths: Vec<ScenePath>,
    portals: Vec<ScenePortal>,
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

        Self {
            paths,
            portals,
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

    /// World-space bounds of every path vertex and portal, padded by portal
    /// width, or `None` when the scenario has no drawable geometry.
    pub const fn bounds(&self) -> Option<(DVec2, DVec2)> {
        self.bounds
    }
}

/// What kind of body a backend is drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyKind {
    /// A motor vehicle.
    #[default]
    Vehicle,
    /// A pedestrian.
    Pedestrian,
}

/// One agent projected into the scene, with interpolation already applied.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Body kind, for backend styling.
    pub kind: BodyKind,
    /// Longitudinal speed in metres per second, when known.
    pub speed_mps: Option<f64>,
    /// The guide path the body follows, when known.
    pub path: Option<PathId>,
    /// Arc-length position along the path in metres, when known.
    pub path_distance_m: Option<f64>,
}

impl SceneBody {
    /// Project a kernel sample, interpolating from its previous position.
    pub fn project(previous: &[AgentSample], sample: &AgentSample, alpha: f64) -> Self {
        let (position, heading_rad) = interpolate(previous, sample, alpha);
        let (length_m, width_m) = sample
            .motion
            .map_or((DEFAULT_BODY_LENGTH_M, DEFAULT_BODY_WIDTH_M), |motion| {
                (motion.body_length_m, motion.body_width_m)
            });
        Self {
            id: sample.id.get() as usize,
            position,
            heading_rad,
            length_m,
            width_m,
            kind: BodyKind::Vehicle,
            speed_mps: sample.motion.map(|motion| motion.speed_mps),
            path: sample.motion.map(|motion| motion.path),
            path_distance_m: sample.motion.map(|motion| motion.path_distance_m),
        }
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

/// A debug overlay a backend may draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Scenario geometry: paths and portals.
    Geometry,
    /// Per-agent velocity vectors.
    Vectors,
}

/// Which optional debug overlays are enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlays {
    /// Draw scenario geometry.
    pub geometry: bool,
    /// Draw per-agent velocity vectors.
    pub vectors: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            geometry: true,
            vectors: false,
        }
    }
}

impl Overlays {
    /// Flip one overlay.
    pub fn toggle(&mut self, overlay: Overlay) {
        match overlay {
            Overlay::Geometry => self.geometry = !self.geometry,
            Overlay::Vectors => self.vectors = !self.vectors,
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
    use tangle_model::parse_scenario_source;

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
    fn overlays_toggle_independently() {
        let mut overlays = Overlays::default();
        assert!(overlays.geometry);
        assert!(!overlays.vectors);
        overlays.toggle(Overlay::Geometry);
        overlays.toggle(Overlay::Vectors);
        assert!(!overlays.geometry);
        assert!(overlays.vectors);
    }

    #[test]
    fn interpolation_uses_shortest_arc() {
        let previous = AgentSample {
            id: tangle_sim::AgentId::from_index(0),
            position: DVec2::ZERO,
            heading_rad: 3.0,
            motion: None,
        };
        let sample = AgentSample {
            id: tangle_sim::AgentId::from_index(0),
            position: DVec2::new(10.0, 0.0),
            heading_rad: -3.0,
            motion: None,
        };
        let body = SceneBody::project(std::slice::from_ref(&previous), &sample, 0.5);
        assert_eq!(body.position, DVec2::new(5.0, 0.0));
        // Wrapping across pi must not rotate the long way around.
        assert!(body.heading_rad.abs() > 3.0);
    }
}
