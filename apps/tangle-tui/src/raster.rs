//! Project a [`SceneFrame`] into a [`CellGrid`].
//!
//! The rasterizer is the character-cell analogue of the Bevy draw systems: it
//! consumes only the backend-agnostic scene projection and never touches kernel
//! state. World metres map to cells with an aspect correction, because a
//! terminal cell is roughly twice as tall as it is wide; a square region of
//! world therefore covers twice as many columns as rows.

use glam::DVec2;
use tangle_present::{SceneFrame, Viewport};

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
/// Agent body color.
pub const BODY_COLOR: Rgb = Rgb::new(217, 222, 235);
/// Selected agent color.
pub const SELECTED_COLOR: Rgb = Rgb::new(255, 140, 51);
/// Selected agent background highlight.
pub const SELECTED_BACKGROUND: Rgb = Rgb::new(64, 30, 8);
/// Velocity-vector color.
pub const VECTOR_COLOR: Rgb = Rgb::new(89, 179, 255);

const PATH_GLYPH: char = '*';
const PORTAL_GLYPH: char = '=';
const PORTAL_TIP_GLYPH: char = '>';
const BODY_GLYPH: char = '#';
const VECTOR_GLYPH: char = '.';

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
        self.draw_bodies(&mut grid, frame);
        if frame.overlays.vectors {
            self.draw_vectors(&mut grid, frame);
        }
        grid
    }

    fn draw_geometry(&self, grid: &mut CellGrid, frame: &SceneFrame) {
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
    }

    fn draw_bodies(&self, grid: &mut CellGrid, frame: &SceneFrame) {
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
    use tangle_present::{FrameStatus, Overlays, SceneGeometry, Speed, Viewport};
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
}
