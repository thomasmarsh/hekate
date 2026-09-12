//! Project a [`SceneFrame`] into a [`CellGrid`].
//!
//! The rasterizer is the character-cell analogue of the Bevy draw systems: it
//! consumes only the backend-agnostic scene projection and never touches kernel
//! state. World metres map to cells with an aspect correction, because a
//! terminal cell is roughly twice as tall as it is wide; a square region of
//! world therefore covers twice as many columns as rows.

use glam::DVec2;
use std::collections::BTreeMap;
use tangle_present::{BodyEmphasis, SceneFrame, Viewport};
use tangle_sim::EventKind;

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
    }
}

const PATH_GLYPH: char = '*';
const PORTAL_GLYPH: char = '=';
const PORTAL_TIP_GLYPH: char = '>';
const BOUNDARY_GLYPH: char = '.';
const REGION_GLYPH: char = ',';
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
        // One emphasis per emphasized body, so a body is styled once.
        let emphasis: BTreeMap<usize, BodyEmphasis> = frame.body_emphasis().into_iter().collect();
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
            let (sin, cos) = body.heading_rad.sin_cos();
            let forward = DVec2::new(cos, sin);
            let left = DVec2::new(-sin, cos);
            let half_length = body.length_m * 0.5;
            let half_width = body.width_m * 0.5;

            let corners = [
                body.position + forward * half_length + left * half_width,
                body.position + forward * half_length - left * half_width,
                body.position - forward * half_length + left * half_width,
                body.position - forward * half_length - left * half_width,
            ];
            let projected: Vec<(f64, f64)> = corners
                .iter()
                .map(|corner| self.project(frame.viewport, *corner))
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

            let (fg, bg) = if selected {
                (SELECTED_COLOR, SELECTED_BACKGROUND)
            } else if let Some(emphasis) = emphasis.get(&body.id) {
                (emphasis_color(*emphasis), BACKGROUND)
            } else {
                (BODY_COLOR, BACKGROUND)
            };
            let cell = Cell::new(BODY_GLYPH, fg, bg);
            for row in min_row..max_row {
                for col in min_col..max_col {
                    let world = self.world_at(frame.viewport, col as f64, row as f64);
                    let offset = world - body.position;
                    if offset.dot(forward).abs() <= half_length
                        && offset.dot(left).abs() <= half_width
                    {
                        grid.put(col, row, cell);
                    }
                }
            }

            // Always mark the center so a sub-cell body stays visible.
            let center = self.project(frame.viewport, body.position);
            grid.put(center.0.round() as i64, center.1.round() as i64, cell);
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

    use tangle_model::{CompiledScenario, parse_scenario_source};
    use tangle_present::{FrameStatus, Overlays, SafetyOverlay, SceneGeometry, Speed, Viewport};
    use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

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
                .map(|sample| tangle_present::SceneBody::project(&[], sample, 0.0))
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
}
