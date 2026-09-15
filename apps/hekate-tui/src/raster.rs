//! Project a [`SceneFrame`] into a [`CellGrid`].
//!
//! The rasterizer is the character-cell analogue of the Bevy draw systems: it
//! consumes only the backend-agnostic scene projection and never touches kernel
//! state. World metres map to cells with an aspect correction, because a
//! terminal cell is roughly twice as tall as it is wide; a square region of
//! world therefore covers twice as many columns as rows.

use glam::DVec2;
use std::collections::BTreeMap;
use hekate_present::{BodyEmphasis, BodyShape, SceneFrame, Viewport};
use hekate_sim::EventKind;

use crate::grid::{Cell, CellGrid};
use crate::palette::Rgb;

/// Height of one terminal cell in units of its width.
///
/// Character cells are close to 1:2, so one row spans twice the world distance
/// of one column at the same viewport scale. The rasterizer and the host's
/// initial fit both use this constant so the picture stays square.
pub const CELL_ASPECT: f64 = 2.0;

/// Scene background, matching the viewer's dark clear color.
pub const BACKGROUND: Rgb = Rgb::new(10, 13, 18);
/// Guide-path color.
pub const PATH_COLOR: Rgb = Rgb::new(61, 209, 112);
/// Portal gate and arrow color.
pub const PORTAL_COLOR: Rgb = Rgb::new(245, 189, 56);
/// Boundary polygon color.
pub const BOUNDARY_COLOR: Rgb = Rgb::new(110, 118, 138);
/// Traversable region color.
pub const REGION_COLOR: Rgb = Rgb::new(64, 156, 176);
/// Facility band color.
pub const FACILITY_COLOR: Rgb = Rgb::new(120, 214, 214);
/// Facility reference-path color.
pub const FACILITY_REFERENCE_COLOR: Rgb = Rgb::new(196, 168, 240);
/// Movement connector color.
pub const MOVEMENT_COLOR: Rgb = Rgb::new(236, 118, 196);
/// Pedestrian crossing color.
pub const CROSSING_COLOR: Rgb = Rgb::new(190, 214, 96);
/// Conflict region color.
pub const CONFLICT_COLOR: Rgb = Rgb::new(230, 84, 84);
/// Control-rule marker color.
pub const RULE_COLOR: Rgb = Rgb::new(170, 168, 240);
/// Signal head color.
pub const SIGNAL_COLOR: Rgb = Rgb::new(240, 100, 60);
/// Agent body color.
pub const BODY_COLOR: Rgb = Rgb::new(217, 222, 235);
/// Selected agent color.
pub const SELECTED_COLOR: Rgb = Rgb::new(255, 140, 51);
/// Selected agent background highlight.
pub const SELECTED_BACKGROUND: Rgb = Rgb::new(64, 30, 8);
/// Velocity-vector color.
pub const VECTOR_COLOR: Rgb = Rgb::new(89, 179, 255);
/// Contact emphasis color.
pub const COLLISION_COLOR: Rgb = Rgb::new(242, 64, 64);
/// Near-miss emphasis color.
pub const NEAR_MISS_COLOR: Rgb = Rgb::new(250, 186, 38);
/// Violation emphasis color.
pub const VIOLATION_COLOR: Rgb = Rgb::new(217, 89, 242);
/// Standstill emphasis color.
pub const QUEUE_COLOR: Rgb = Rgb::new(89, 191, 242);
/// Controller-state emphasis color.
pub const CONTROL_COLOR: Rgb = Rgb::new(140, 217, 140);
/// Occupied-region overlay color.
pub const OCCUPIED_COLOR: Rgb = Rgb::new(250, 158, 46);

/// Color one body emphasis draws.
pub const fn emphasis_color(emphasis: BodyEmphasis) -> Rgb {
    match emphasis {
        BodyEmphasis::Collision => COLLISION_COLOR,
        BodyEmphasis::NearMiss => NEAR_MISS_COLOR,
        BodyEmphasis::Violation => VIOLATION_COLOR,
        BodyEmphasis::Queue => QUEUE_COLOR,
        BodyEmphasis::ControlTransition => CONTROL_COLOR,
    }
}

/// Glyph one safety-marker kind draws, or `None` for a record with no marker.
pub const fn marker_glyph(kind: EventKind) -> Option<char> {
    match kind {
        EventKind::Collision => Some('X'),
        EventKind::NearMiss => Some('~'),
        EventKind::Violation => Some('!'),
        EventKind::Entry | EventKind::Exit => Some('x'),
        EventKind::Queue => Some('q'),
        EventKind::Yielded | EventKind::ControlTransition => Some('c'),
        EventKind::Spawned | EventKind::Despawned => None,
        // The increment-2 maneuver and rule records are not markers
        // ([`is_safety_record`] does not carry them), so they draw none.
        EventKind::Maneuver
        | EventKind::FacilityTransition
        | EventKind::OpposingTraversal
        | EventKind::ClosePass => None,
    }
}

/// Color one safety-marker kind draws.
pub const fn marker_color(kind: EventKind) -> Rgb {
    match kind {
        EventKind::Collision => COLLISION_COLOR,
        EventKind::NearMiss => NEAR_MISS_COLOR,
        EventKind::Violation => VIOLATION_COLOR,
        EventKind::Entry | EventKind::Exit => OCCUPIED_COLOR,
        EventKind::Queue => QUEUE_COLOR,
        EventKind::Yielded | EventKind::ControlTransition => CONTROL_COLOR,
        EventKind::Spawned | EventKind::Despawned => BODY_COLOR,
        EventKind::Maneuver
        | EventKind::FacilityTransition
        | EventKind::OpposingTraversal
        | EventKind::ClosePass => BODY_COLOR,
    }
}

const PATH_GLYPH: char = '*';
const PORTAL_GLYPH: char = '=';
const PORTAL_TIP_GLYPH: char = '>';
const BOUNDARY_GLYPH: char = '.';
const REGION_GLYPH: char = ',';
const FACILITY_GLYPH: char = ':';
const FACILITY_REFERENCE_GLYPH: char = '+';
const MOVEMENT_GLYPH: char = '%';
const CROSSING_GLYPH: char = 'x';
const CONFLICT_GLYPH: char = '!';
const RULE_GLYPH: char = 'o';
const SIGNAL_GLYPH: char = 'S';
const BODY_GLYPH: char = '#';
const VECTOR_GLYPH: char = '.';
/// Glyph traced along an occupied region's ring.
const OCCUPIED_GLYPH: char = 'x';
/// Half-width of a rendered signal-head gate in world metres.
pub const SIGNAL_GATE_HALF_WIDTH_M: f64 = 1.5;
/// Perpendicular offset of a rendered rule marker from its movement entry, in
/// world metres, so a rule marker and a signal gate at the same entry do not
/// draw over each other.
pub const RULE_MARKER_OFFSET_M: f64 = 2.5;

/// Draws a [`SceneFrame`] into a character grid of a fixed size.
#[derive(Debug, Clone, Copy)]
pub struct Rasterizer {
    width: u32,
    height: u32,
}

impl Rasterizer {
    /// A rasterizer producing a `width` by `height` grid.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Grid width in columns.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Grid height in rows.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Fractional grid position of a world point: `(col, row)` with row zero at
    /// the top, matching terminal coordinates.
    pub fn project(&self, viewport: Viewport, point: DVec2) -> (f64, f64) {
        let half_width = f64::from(self.width) / 2.0;
        let half_height = f64::from(self.height) / 2.0;
        let col = half_width + (point.x - viewport.center().x) / viewport.scale();
        let row = half_height - (point.y - viewport.center().y) / (viewport.scale() * CELL_ASPECT);
        (col, row)
    }

    /// World point at the center of grid cell `(col, row)`.
    pub fn world_at(&self, viewport: Viewport, col: f64, row: f64) -> DVec2 {
        let half_width = f64::from(self.width) / 2.0;
        let half_height = f64::from(self.height) / 2.0;
        DVec2::new(
            viewport.center().x + (col - half_width) * viewport.scale(),
            viewport.center().y - (row - half_height) * viewport.scale() * CELL_ASPECT,
        )
    }

    /// Rasterize one frame: geometry, bodies, then overlays.
    pub fn rasterize(&self, frame: &SceneFrame) -> CellGrid {
        let mut grid = CellGrid::new(self.width, self.height, Cell::blank(BACKGROUND));
        if frame.overlays.geometry {
            self.draw_geometry(&mut grid, frame);
        }
        // One emphasis per emphasized body, so a body is styled once. Emphasis
        // is part of the safety overlay, so the overlay switch governs it too.
        let emphasis: BTreeMap<usize, BodyEmphasis> = if frame.overlays.safety {
            frame.body_emphasis().into_iter().collect()
        } else {
            BTreeMap::new()
        };
        self.draw_bodies(&mut grid, frame, &emphasis);
        if frame.overlays.safety {
            self.draw_safety(&mut grid, frame);
        }
        if frame.overlays.vectors {
            self.draw_vectors(&mut grid, frame);
        }
        grid
    }

    fn draw_geometry(&self, grid: &mut CellGrid, frame: &SceneFrame) {
        // Boundaries and traversable regions form the static backdrop; guide
        // paths and movements are drawn over them.
        let boundary_cell = Cell::new(BOUNDARY_GLYPH, BOUNDARY_COLOR, BACKGROUND);
        for boundary in frame.geometry.boundaries() {
            self.draw_ring(grid, frame.viewport, boundary.points(), boundary_cell);
        }
        let region_cell = Cell::new(REGION_GLYPH, REGION_COLOR, BACKGROUND);
        for region in frame.geometry.regions() {
            self.draw_ring(grid, frame.viewport, region.points(), region_cell);
        }

        let path_cell = Cell::new(PATH_GLYPH, PATH_COLOR, BACKGROUND);
        for path in frame.geometry.paths() {
            let points: Vec<(f64, f64)> = path
                .points()
                .iter()
                .map(|point| self.project(frame.viewport, *point))
                .collect();
            for pair in points.windows(2) {
                draw_line(grid, pair[0], pair[1], path_cell);
            }
        }

        // A movement follows the same polyline as its guide path but is the
        // routed connector, so it gets its own color, glyph, and endpoints.
        let movement_cell = Cell::new(MOVEMENT_GLYPH, MOVEMENT_COLOR, BACKGROUND);
        for movement in frame.geometry.movements() {
            let points: Vec<(f64, f64)> = movement
                .points()
                .iter()
                .map(|point| self.project(frame.viewport, *point))
                .collect();
            for pair in points.windows(2) {
                draw_line(grid, pair[0], pair[1], movement_cell);
            }
            for endpoint in [movement.entry(), movement.exit()] {
                let point = self.project(frame.viewport, endpoint);
                grid.put(
                    point.0.round() as i64,
                    point.1.round() as i64,
                    movement_cell,
                );
            }
        }

        // A facility is drawn over the region it occupies and over the guide
        // path and movement it references, so a band and its reference path
        // read as a facility rather than as authored geometry.
        let facility_cell = Cell::new(FACILITY_GLYPH, FACILITY_COLOR, BACKGROUND);
        let reference_cell = Cell::new(
            FACILITY_REFERENCE_GLYPH,
            FACILITY_REFERENCE_COLOR,
            BACKGROUND,
        );
        for facility in frame.geometry.facilities() {
            self.draw_ring(grid, frame.viewport, facility.points(), facility_cell);
            if let Some(reference) = facility.reference() {
                let points: Vec<(f64, f64)> = reference
                    .points()
                    .iter()
                    .map(|point| self.project(frame.viewport, *point))
                    .collect();
                for pair in points.windows(2) {
                    draw_line(grid, pair[0], pair[1], reference_cell);
                }
            }
        }

        let conflict_cell = Cell::new(CONFLICT_GLYPH, CONFLICT_COLOR, BACKGROUND);
        for conflict in frame.geometry.conflict_regions() {
            self.draw_ring(grid, frame.viewport, conflict.points(), conflict_cell);
        }
        let crossing_cell = Cell::new(CROSSING_GLYPH, CROSSING_COLOR, BACKGROUND);
        for crossing in frame.geometry.crossings() {
            self.draw_ring(grid, frame.viewport, crossing.points(), crossing_cell);
        }

        let portal_cell = Cell::new(PORTAL_GLYPH, PORTAL_COLOR, BACKGROUND);
        let tip_cell = Cell::new(PORTAL_TIP_GLYPH, PORTAL_COLOR, BACKGROUND);
        for portal in frame.geometry.portals() {
            let (start, end) = portal.gate();
            draw_line(
                grid,
                self.project(frame.viewport, start),
                self.project(frame.viewport, end),
                portal_cell,
            );
            let position = self.project(frame.viewport, portal.position());
            draw_line(
                grid,
                position,
                self.project(frame.viewport, portal.inward_tip(3.0)),
                tip_cell,
            );
            grid.put(
                position.0.round() as i64,
                position.1.round() as i64,
                portal_cell,
            );
        }

        let rule_cell = Cell::new(RULE_GLYPH, RULE_COLOR, BACKGROUND);
        for rule in frame.geometry.rules() {
            let (sin, cos) = rule.heading().sin_cos();
            let offset = DVec2::new(-sin, cos) * RULE_MARKER_OFFSET_M;
            let point = self.project(frame.viewport, rule.position() + offset);
            grid.put(point.0.round() as i64, point.1.round() as i64, rule_cell);
        }

        let signal_cell = Cell::new(SIGNAL_GLYPH, SIGNAL_COLOR, BACKGROUND);
        for signal in frame.geometry.signals() {
            for head in signal.heads() {
                let (sin, cos) = head.heading().sin_cos();
                let offset = DVec2::new(sin, -cos) * SIGNAL_GATE_HALF_WIDTH_M;
                draw_line(
                    grid,
                    self.project(frame.viewport, head.position() + offset),
                    self.project(frame.viewport, head.position() - offset),
                    signal_cell,
                );
                let point = self.project(frame.viewport, head.position());
                grid.put(point.0.round() as i64, point.1.round() as i64, signal_cell);
            }
        }
    }

    /// Draw a closed polygon outline, connecting the last vertex to the first.
    fn draw_ring(&self, grid: &mut CellGrid, viewport: Viewport, points: &[DVec2], cell: Cell) {
        if points.len() < 2 {
            return;
        }
        for index in 0..points.len() {
            let from = self.project(viewport, points[index]);
            let to = self.project(viewport, points[(index + 1) % points.len()]);
            draw_line(grid, from, to, cell);
        }
    }

    fn draw_bodies(
        &self,
        grid: &mut CellGrid,
        frame: &SceneFrame,
        emphasis: &BTreeMap<usize, BodyEmphasis>,
    ) {
        for body in &frame.bodies {
            let selected = frame.status.selection == Some(body.id);
            let (fg, bg) = if selected {
                (SELECTED_COLOR, SELECTED_BACKGROUND)
            } else if let Some(emphasis) = emphasis.get(&body.id) {
                (emphasis_color(*emphasis), BACKGROUND)
            } else {
                (BODY_COLOR, BACKGROUND)
            };
            let cell = Cell::new(BODY_GLYPH, fg, bg);
            // A body draws the shapes the shared scene projection chooses for
            // it: one box, circle, or capsule, or one box per ordered segment.
            // The choice reads scene data, so no presenter branch names a mode
            // or scenario.
            for shape in body.shapes() {
                let center = match shape {
                    BodyShape::Circle { center, radius_m } => {
                        self.draw_circle(grid, frame.viewport, center, radius_m, cell);
                        center
                    }
                    BodyShape::Box {
                        center,
                        heading_rad,
                        length_m,
                        width_m,
                    } => {
                        self.draw_box(
                            grid,
                            frame.viewport,
                            center,
                            heading_rad,
                            length_m,
                            width_m,
                            cell,
                        );
                        center
                    }
                    BodyShape::Capsule {
                        center,
                        heading_rad,
                        length_m,
                        radius_m,
                    } => {
                        self.draw_capsule(
                            grid,
                            frame.viewport,
                            center,
                            heading_rad,
                            length_m,
                            radius_m,
                            cell,
                        );
                        center
                    }
                };
                // Always mark the centre so a sub-cell shape stays visible.
                let point = self.project(frame.viewport, center);
                grid.put(point.0.round() as i64, point.1.round() as i64, cell);
            }
        }
    }

    /// Fill the cells inside a capsule: a straight segment of `length_m` and
    /// two semicircular caps of `radius_m` on its ends.
    ///
    /// The straight part is the rectangle the segment spans, and each cap is a
    /// circle at the segment's end, so the union is exactly the capsule. No
    /// rectangle covers a capsule, and neither would one drawn at the body's
    /// bounding extent.
    #[allow(clippy::too_many_arguments)]
    fn draw_capsule(
        &self,
        grid: &mut CellGrid,
        viewport: Viewport,
        center: DVec2,
        heading_rad: f64,
        length_m: f64,
        radius_m: f64,
        cell: Cell,
    ) {
        let (sin, cos) = heading_rad.sin_cos();
        let forward = DVec2::new(cos, sin);
        let half_length = length_m * 0.5;
        self.draw_box(
            grid,
            viewport,
            center,
            heading_rad,
            length_m,
            radius_m * 2.0,
            cell,
        );
        for cap in [1.0, -1.0] {
            self.draw_circle(
                grid,
                viewport,
                center + forward * (half_length * cap),
                radius_m,
                cell,
            );
        }
    }

    /// Fill the cells inside an oriented box of `length_m` by `width_m`.
    #[allow(clippy::too_many_arguments)]
    fn draw_box(
        &self,
        grid: &mut CellGrid,
        viewport: Viewport,
        center: DVec2,
        heading_rad: f64,
        length_m: f64,
        width_m: f64,
        cell: Cell,
    ) {
        let (sin, cos) = heading_rad.sin_cos();
        let forward = DVec2::new(cos, sin);
        let left = DVec2::new(-sin, cos);
        let half_length = length_m * 0.5;
        let half_width = width_m * 0.5;

        let corners = [
            center + forward * half_length + left * half_width,
            center + forward * half_length - left * half_width,
            center - forward * half_length + left * half_width,
            center - forward * half_length - left * half_width,
        ];
        let projected: Vec<(f64, f64)> = corners
            .iter()
            .map(|corner| self.project(viewport, *corner))
            .collect();
        let min_col = projected
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0) as i64;
        let max_col = projected
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min(f64::from(self.width)) as i64;
        let min_row = projected
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0) as i64;
        let max_row = projected
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min(f64::from(self.height)) as i64;

        for row in min_row..max_row {
            for col in min_col..max_col {
                let world = self.world_at(viewport, col as f64, row as f64);
                let offset = world - center;
                if offset.dot(forward).abs() <= half_length && offset.dot(left).abs() <= half_width
                {
                    grid.put(col, row, cell);
                }
            }
        }
    }

    /// Fill the cells inside a circle of `radius_m` metres.
    fn draw_circle(
        &self,
        grid: &mut CellGrid,
        viewport: Viewport,
        center: DVec2,
        radius_m: f64,
        cell: Cell,
    ) {
        let corner = DVec2::splat(radius_m);
        let low = self.project(viewport, center - corner);
        let high = self.project(viewport, center + corner);
        let min_col = low.0.min(high.0).floor().max(0.0) as i64;
        let max_col = low.0.max(high.0).ceil().min(f64::from(self.width)) as i64;
        let min_row = low.1.min(high.1).floor().max(0.0) as i64;
        let max_row = low.1.max(high.1).ceil().min(f64::from(self.height)) as i64;

        for row in min_row..max_row {
            for col in min_col..max_col {
                let world = self.world_at(viewport, col as f64, row as f64);
                if (world - center).length() <= radius_m {
                    grid.put(col, row, cell);
                }
            }
        }
    }

    /// Draw the frame's safety overlays: occupied-region rings and event
    /// markers, both derived from the frame's projected safety data.
    fn draw_safety(&self, grid: &mut CellGrid, frame: &SceneFrame) {
        let occupied = Cell::new(OCCUPIED_GLYPH, OCCUPIED_COLOR, BACKGROUND);
        for region in frame.occupied_regions() {
            let Some(points) = frame.region_points(region.region()) else {
                continue;
            };
            self.draw_ring(grid, frame.viewport, points, occupied);
        }

        for marker in frame.safety_markers() {
            let Some(glyph) = marker_glyph(marker.kind()) else {
                continue;
            };
            let point = self.project(frame.viewport, marker.position());
            grid.put(
                point.0.round() as i64,
                point.1.round() as i64,
                Cell::new(glyph, marker_color(marker.kind()), BACKGROUND),
            );
        }
    }

    fn draw_vectors(&self, grid: &mut CellGrid, frame: &SceneFrame) {
        let cell = Cell::new(VECTOR_GLYPH, VECTOR_COLOR, BACKGROUND);
        for body in &frame.bodies {
            let Some(speed) = body.speed_mps else {
                continue;
            };
            let (sin, cos) = body.heading_rad.sin_cos();
            let tip = body.position + DVec2::new(cos, sin) * speed;
            draw_line(
                grid,
                self.project(frame.viewport, body.position),
                self.project(frame.viewport, tip),
                cell,
            );
        }
    }
}

/// Draw an integer Bresenham line between two fractional cell positions,
/// rounding each endpoint. Out-of-bounds cells are clipped by [`CellGrid::put`].
fn draw_line(grid: &mut CellGrid, from: (f64, f64), to: (f64, f64), cell: Cell) {
    let mut x0 = from.0.round() as i64;
    let mut y0 = from.1.round() as i64;
    let x1 = to.0.round() as i64;
    let y1 = to.1.round() as i64;

    let dx = (x1 - x0).abs();
    let step_x = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let step_y = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        grid.put(x0, y0, cell);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x0 += step_x;
        }
        if twice <= dx {
            error += dx;
            y0 += step_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use hekate_model::{BodyKind, CompiledScenario, CrossingId, parse_scenario_source};
    use hekate_present::{
        FrameStatus, Overlays, SafetyOverlay, SceneBody, SceneGeometry, Speed, Viewport,
        load_scenario,
    };
    use hekate_sim::{
        AgentId, AgentMode, BodySegmentSample, Event, RegionKey, RunConfig, Simulation,
        SnapshotDetail,
    };

    fn scenario() -> CompiledScenario {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
             population: { vehicle_count: 1, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    fn frame(viewport: Viewport) -> SceneFrame {
        let compiled = scenario();
        let sim = Simulation::new(compiled.clone(), RunConfig::new(0)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport,
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: snapshot
                .agents()
                .iter()
                .map(|sample| hekate_present::SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
        }
    }

    /// A geometry-only frame for a scenario that carries every general
    /// primitive, so no simulation population is needed to project it.
    fn general_frame(viewport: Viewport) -> SceneFrame {
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
             regions: [ { id: 'area', points: [ { x: -8, y: -3 }, { x: -3, y: -3 }, \
             { x: -3, y: 3 }, { x: -8, y: 3 } ] }, \
             { id: 'plaza', points: [ { x: -28, y: -28 }, { x: -22, y: -28 }, \
             { x: -22, y: -22 }, { x: -28, y: -22 } ] } ], \
             movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'ew', priority: 0 }, \
             { id: 'ns_through', from: 'south', to: 'north', path: 'ns', priority: 1 } ], \
             crossings: [ { id: 'cross', region: 'area', movements: [ 'ew_through' ] } ], \
             conflict_regions: [ { id: 'center', points: [ { x: -2, y: -2 }, { x: 2, y: -2 }, \
             { x: 2, y: 2 }, { x: -2, y: 2 } ], movements: [ 'ew_through', 'ns_through' ] } ], \
             rules: [ { id: 'r_ew', movement: 'ew_through', kind: 'signal', signal: 'main' } ], \
             signals: [ { id: 'main', heads: [ { id: 'ew', movement: 'ew_through' } ], \
             phases: [ { duration_s: 20.0, states: [ { head: 'ew', color: 'green' } ] } ] } ] }",
        )
        .expect("scenario parses");
        let compiled = CompiledScenario::compile(source).expect("scenario compiles");
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: 0,
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport,
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: Vec::new(),
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
        }
    }

    #[test]
    fn aspect_correction_makes_one_row_span_two_columns_of_world() {
        let raster = Rasterizer::new(40, 20);
        let viewport = Viewport::new(DVec2::ZERO, 1.0);
        // 10 m east is 10 columns; 10 m north is 5 rows at a 1:2 cell aspect.
        assert_eq!(
            raster.project(viewport, DVec2::new(10.0, 0.0)),
            (30.0, 10.0)
        );
        assert_eq!(raster.project(viewport, DVec2::new(0.0, 10.0)), (20.0, 5.0));
    }

    #[test]
    fn project_and_world_at_round_trip() {
        let raster = Rasterizer::new(80, 24);
        let viewport = Viewport::new(DVec2::new(12.0, -3.0), 0.5);
        let (col, row) = raster.project(viewport, DVec2::new(15.0, 1.0));
        let world = raster.world_at(viewport, col, row);
        assert!((world.x - 15.0).abs() < 1e-9);
        assert!((world.y - 1.0).abs() < 1e-9);
    }

    /// A frame over two vehicles and one crossing region, carrying a contact,
    /// a standstill, and a region entry at tick 0: every family the safety
    /// overlay draws.
    fn safety_frame() -> SceneFrame {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'cross', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'ew', points: [ { x: -20, y: 0 }, { x: 20, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'ew', end: 'start', width_m: 3.5 }, \
             { id: 'east', path: 'ew', end: 'end', width_m: 3.5 } ], \
             regions: [ { id: 'area', points: [ { x: -2, y: -4 }, { x: 2, y: -4 }, \
             { x: 2, y: 4 }, { x: -2, y: 4 } ] } ], \
             movements: [ { id: 'ew_through', from: 'west', to: 'east', path: 'ew', \
             priority: 0 } ], \
             crossings: [ { id: 'cross', region: 'area', movements: [ 'ew_through' ] } ], \
             population: { vehicle_count: 2, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        let compiled = CompiledScenario::compile(source).expect("scenario compiles");
        let sim = Simulation::new(compiled.clone(), RunConfig::new(0)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let mut safety = SafetyOverlay::new(40);
        safety.observe(
            0,
            &[
                Event::Entry {
                    agent: AgentId::from_index(0),
                    region: RegionKey::Crossing(CrossingId::from_index(0)),
                },
                Event::Collision {
                    agent: AgentId::from_index(0),
                    other: AgentId::from_index(1),
                    clearance_m: -0.4,
                    contacting: true,
                },
                Event::Queue {
                    agent: AgentId::from_index(1),
                    joined: true,
                },
            ],
        );
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport: Viewport::new(DVec2::ZERO, 0.5),
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: snapshot
                .agents()
                .iter()
                .map(|sample| hekate_present::SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
            safety,
        }
    }

    /// Every cell the rasterizer drew, as `(glyph, foreground color)`.
    fn raster_cells(frame: &SceneFrame) -> Vec<(char, Rgb)> {
        let raster = Rasterizer::new(120, 40);
        let grid = raster.rasterize(frame);
        (0..grid.height())
            .flat_map(|row| (0..grid.width()).map(move |col| (col, row)))
            .filter_map(|(col, row)| grid.get(i64::from(col), i64::from(row)))
            .map(|cell| (cell.ch, cell.fg))
            .collect()
    }

    #[test]
    fn safety_occupancy_emphasis_and_markers_are_rasterized() {
        let cells = raster_cells(&safety_frame());
        for (name, cell) in [
            ("occupied region ring", (OCCUPIED_GLYPH, OCCUPIED_COLOR)),
            ("contact marker", ('X', COLLISION_COLOR)),
            ("standstill marker", ('q', QUEUE_COLOR)),
            ("emphasized body", (BODY_GLYPH, COLLISION_COLOR)),
        ] {
            assert!(cells.contains(&cell), "the {name} was not rasterized");
        }
    }

    #[test]
    fn the_safety_overlay_can_be_disabled() {
        let mut frame = safety_frame();
        frame.overlays.safety = false;
        let cells = raster_cells(&frame);
        assert!(!cells.contains(&(OCCUPIED_GLYPH, OCCUPIED_COLOR)));
        assert!(!cells.contains(&('X', COLLISION_COLOR)));
        assert!(!cells.contains(&('q', QUEUE_COLOR)));
        assert!(
            cells.contains(&(BODY_GLYPH, BODY_COLOR)),
            "a body must still draw unemphasized"
        );
    }

    #[test]
    fn rasterize_marks_geometry_and_bodies() {
        let raster = Rasterizer::new(120, 40);
        let frame = frame(Viewport::new(DVec2::ZERO, 0.5));
        let grid = raster.rasterize(&frame);
        let cells: Vec<char> = (0..grid.height())
            .flat_map(|row| (0..grid.width()).map(move |col| (col, row)))
            .filter_map(|(col, row)| grid.get(i64::from(col), i64::from(row)).map(|c| c.ch))
            .collect();
        assert!(
            cells
                .iter()
                .any(|&ch| ch == PATH_GLYPH || ch == PORTAL_GLYPH)
        );
        assert!(cells.contains(&BODY_GLYPH));
    }

    /// A geometry-only frame over a checked-in version-2 fixture, so a
    /// facility's band and reference path come from the same source a viewer
    /// opens.
    fn facility_frame(viewport: Viewport) -> SceneFrame {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scenarios/phase2/inc1/narrow_isolated_straight_v2.json5");
        let compiled = load_scenario(&path).expect("fixture loads");
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: 0,
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport,
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: Vec::new(),
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
        }
    }

    /// A facility's occupied band and its reference path draw from scene data:
    /// the band's ring carries the facility glyph and color, and the reference
    /// polyline the reference glyph and color over the guide-path pass.
    #[test]
    fn a_facility_band_and_reference_path_are_rasterized_from_scene_data() {
        let raster = Rasterizer::new(120, 40);
        let grid = raster.rasterize(&facility_frame(Viewport::new(DVec2::new(110.0, 0.0), 0.5)));
        let cells: Vec<(char, Rgb)> = (0..grid.height())
            .flat_map(|row| (0..grid.width()).map(move |col| (col, row)))
            .filter_map(|(col, row)| grid.get(i64::from(col), i64::from(row)))
            .map(|cell| (cell.ch, cell.fg))
            .collect();
        assert!(
            cells.contains(&(FACILITY_GLYPH, FACILITY_COLOR)),
            "the facility band was not rasterized"
        );
        assert!(
            cells.contains(&(FACILITY_REFERENCE_GLYPH, FACILITY_REFERENCE_COLOR)),
            "the facility reference path was not rasterized"
        );
    }

    #[test]
    fn geometry_overlay_can_be_disabled() {
        let raster = Rasterizer::new(120, 40);
        let mut frame = frame(Viewport::new(DVec2::ZERO, 0.5));
        frame.overlays.geometry = false;
        let grid = raster.rasterize(&frame);
        let visible = (0..grid.height())
            .flat_map(|row| (0..grid.width()).map(move |col| (col, row)))
            .filter_map(|(col, row)| grid.get(i64::from(col), i64::from(row)).map(|c| c.ch))
            .any(|ch| ch == PATH_GLYPH || ch == PORTAL_GLYPH);
        assert!(!visible);
    }

    #[test]
    fn a_selected_body_is_drawn_in_the_selection_color() {
        let raster = Rasterizer::new(120, 40);
        let mut frame = frame(Viewport::new(DVec2::new(0.0, 0.0), 0.25));
        frame.status.selection = frame.bodies.first().map(|body| body.id);
        let grid = raster.rasterize(&frame);
        let selected = (0..grid.height())
            .flat_map(|row| (0..grid.width()).map(move |col| (col, row)))
            .filter_map(|(col, row)| grid.get(i64::from(col), i64::from(row)))
            .any(|cell| cell.fg == SELECTED_COLOR);
        assert!(selected);
    }

    #[test]
    fn every_general_primitive_is_rasterized_in_its_own_color() {
        let raster = Rasterizer::new(160, 80);
        let frame = general_frame(Viewport::new(DVec2::ZERO, 0.4));
        let grid = raster.rasterize(&frame);
        let colors: Vec<Rgb> = (0..grid.height())
            .flat_map(|row| (0..grid.width()).map(move |col| (col, row)))
            .filter_map(|(col, row)| grid.get(i64::from(col), i64::from(row)).map(|cell| cell.fg))
            .collect();
        for (name, color) in [
            ("boundary", BOUNDARY_COLOR),
            ("region", REGION_COLOR),
            ("movement", MOVEMENT_COLOR),
            ("crossing", CROSSING_COLOR),
            ("conflict region", CONFLICT_COLOR),
            ("rule", RULE_COLOR),
            ("signal", SIGNAL_COLOR),
        ] {
            assert!(
                colors.contains(&color),
                "{name} primitive was not rasterized in its color"
            );
        }
    }

    /// One scene body with no ordered segments, so a test draws a single
    /// envelope the shared shape decision chooses by body kind.
    fn body(
        id: usize,
        body_kind: BodyKind,
        position: DVec2,
        length_m: f64,
        width_m: f64,
    ) -> SceneBody {
        SceneBody {
            id,
            position,
            heading_rad: 0.0,
            length_m,
            width_m,
            mode: AgentMode::Vehicle,
            body_kind,
            segments: Vec::new(),
            speed_mps: Some(0.0),
            path: None,
            path_distance_m: None,
            route: None,
            profile: None,
            decision: None,
        }
    }

    /// A frame over an empty geometry, carrying only `bodies`, so a body-shape
    /// test sees no scene geometry.
    fn synthetic_frame(bodies: Vec<SceneBody>) -> SceneFrame {
        SceneFrame {
            scenario_id: "presenter-shapes".to_owned(),
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: bodies.len(),
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport: Viewport::new(DVec2::ZERO, 1.0),
            geometry: Arc::new(SceneGeometry::default()),
            bodies,
            overlays: Overlays {
                geometry: false,
                vectors: false,
                safety: false,
            },
            safety: SafetyOverlay::default(),
        }
    }

    /// True when the cell at `(col, row)` carries the body glyph and color.
    fn is_body(grid: &CellGrid, col: i64, row: i64) -> bool {
        grid.get(col, row)
            .is_some_and(|cell| cell.ch == BODY_GLYPH && cell.fg == BODY_COLOR)
    }

    /// A box body and a circle body draw visibly different shapes, and a body
    /// carrying ordered segments draws each segment at its own pose: the cell
    /// filling a circle's bounding square corner stays empty for the circle but
    /// is filled for the box, and both segment centres carry the body glyph.
    #[test]
    fn each_body_kind_and_ordered_segment_is_rasterized_from_scene_data() {
        let raster = Rasterizer::new(80, 40);
        // 80x40 cells at scale 1: one column is one metre, one row two metres.
        // A 6x6 box at (-10, 0) and a 6 m diameter circle at (10, 0).
        let box_body = body(0, BodyKind::Box, DVec2::new(-10.0, 0.0), 6.0, 6.0);
        let circle = body(1, BodyKind::Circle, DVec2::new(10.0, 0.0), 6.0, 6.0);
        let mut articulated = body(
            2,
            BodyKind::ArticulatedChain,
            DVec2::new(0.0, 8.0),
            8.0,
            2.0,
        );
        articulated.segments = vec![
            BodySegmentSample {
                position: DVec2::new(-2.0, 8.0),
                heading_rad: 0.0,
            },
            BodySegmentSample {
                position: DVec2::new(2.0, 8.0),
                heading_rad: 0.0,
            },
        ];
        let grid = raster.rasterize(&synthetic_frame(vec![box_body, circle, articulated]));

        // A box corner cell (world (-13, 2)) is inside the box; the mirrored
        // circle corner cell (world (13, 2)) is outside the circle.
        assert!(is_body(&grid, 27, 19), "the box must fill its corner");
        assert!(
            !is_body(&grid, 53, 19),
            "the circle must not fill its bounding square's corner"
        );
        // Both body centres draw, so neither kind is skipped.
        assert!(is_body(&grid, 30, 20), "the box centre must draw");
        assert!(is_body(&grid, 50, 20), "the circle centre must draw");

        // Each segment centre draws its own box at the segment's pose.
        assert!(is_body(&grid, 38, 16), "segment 0 must draw at its pose");
        assert!(is_body(&grid, 42, 16), "segment 1 must draw at its pose");
    }

    /// A capsule body draws its straight part and both caps: a cell inside a
    /// cap beyond the straight part carries the body, and the rectangle that
    /// bounds the capsule fills the box body's corner the cap curves away
    /// from.
    #[test]
    fn a_capsule_body_is_rasterized_as_a_capsule() {
        let raster = Rasterizer::new(80, 40);
        // One column is 0.25 m and one row 0.5 m, so a 1 m cap is resolvable.
        let viewport = Viewport::new(DVec2::ZERO, 0.25);
        let mut capsule_frame =
            synthetic_frame(vec![body(0, BodyKind::Capsule, DVec2::ZERO, 6.0, 2.0)]);
        capsule_frame.viewport = viewport;
        let capsule = raster.rasterize(&capsule_frame);

        // The straight part: world (2, 0.5) is inside the 6 x 2 rectangle.
        assert!(is_body(&capsule, 48, 19), "the straight part must draw");
        // Each cap: world (+-3.75, 0) is 0.75 m beyond the straight part's end
        // and within the 1 m cap radius, which no rectangle of the reported
        // length covers.
        assert!(is_body(&capsule, 55, 20), "the front cap must draw");
        assert!(is_body(&capsule, 25, 20), "the rear cap must draw");

        // The rectangle bounding the capsule, drawn as a box body of the same
        // extent, fills the corner world (3.75, 1); the capsule leaves it
        // empty because the cap curves away from it.
        let mut bounding_frame =
            synthetic_frame(vec![body(0, BodyKind::Box, DVec2::ZERO, 8.0, 2.0)]);
        bounding_frame.viewport = viewport;
        let bounding = raster.rasterize(&bounding_frame);
        assert!(
            is_body(&bounding, 55, 18),
            "the bounding box fills its corner"
        );
        assert!(
            !is_body(&capsule, 55, 18),
            "the capsule must not fill the bounding rectangle's corner"
        );
    }
}
