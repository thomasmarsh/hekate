//! Project a [`SceneFrame`] into an RGBA pixel image.
//!
//! This is the pixel twin of [`crate::raster`]. It deliberately reuses the
//! character rasterizer's projection and its color constants, so the Kitty
//! graphics backend draws the same geometry in the same styles as the
//! character-cell backend; only the sampling grid differs. World metres map to
//! cells with a 1:2 aspect correction, and each cell maps to a fixed block of
//! pixels, so the picture stays square when the terminal scales the image to a
//! cell rectangle.

use glam::DVec2;
use tangle_present::{SceneFrame, Viewport};

use crate::palette::Rgb;
use crate::raster::{
    BACKGROUND, BODY_COLOR, CELL_ASPECT, PATH_COLOR, PORTAL_COLOR, Rasterizer, SELECTED_BACKGROUND,
    SELECTED_COLOR, VECTOR_COLOR,
};

/// Default pixels per terminal column.
pub const DEFAULT_PIXELS_PER_COLUMN: u32 = 6;
/// Smallest pixels per terminal column before the rasterizer gives up shrinking.
pub const MIN_PIXELS_PER_COLUMN: u32 = 2;
/// Upper bound on one frame, so a very large terminal cannot allocate an
/// unbounded image.
pub const MAX_FRAME_PIXELS: u64 = 1920 * 1080;
/// Half-width of a drawn guide line, in pixels.
const LINE_HALF_WIDTH: i64 = 1;

/// A tightly packed RGBA8 image, row-major, four bytes per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaImage {
    /// A transparent-black `width` by `height` image.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width as usize) * (height as usize) * 4],
        }
    }

    /// Image width in pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Image height in pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The raw RGBA bytes, row-major.
    pub fn rgba(&self) -> &[u8] {
        &self.pixels
    }

    /// Set every pixel to `color`.
    pub fn fill(&mut self, color: Rgb) {
        let rgba = opaque(color);
        for pixel in self.pixels.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&rgba);
        }
    }

    /// Set one pixel. Out-of-bounds writes are ignored.
    pub fn put(&mut self, x: i64, y: i64, color: Rgb) {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return;
        }
        let index = ((y as u32 * self.width + x as u32) * 4) as usize;
        self.pixels[index..index + 4].copy_from_slice(&opaque(color));
    }

    /// Fill an axis-aligned rectangle in pixel coordinates.
    fn rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: Rgb) {
        let x0 = x.floor() as i64;
        let y0 = y.floor() as i64;
        let x1 = (x + width).ceil() as i64;
        let y1 = (y + height).ceil() as i64;
        for py in y0..y1 {
            for px in x0..x1 {
                self.put(px, py, color);
            }
        }
    }

    /// Draw a thick segment between two pixel positions.
    fn segment(&mut self, from: (f64, f64), to: (f64, f64), color: Rgb, half_width: i64) {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let steps = dx.abs().max(dy.abs()).ceil().max(1.0) as i64;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let x = (from.0 + dx * t).round() as i64;
            let y = (from.1 + dy * t).round() as i64;
            for oy in -half_width..=half_width {
                for ox in -half_width..=half_width {
                    self.put(x + ox, y + oy, color);
                }
            }
        }
    }
}

/// RGBA for an opaque [`Rgb`].
const fn opaque(color: Rgb) -> [u8; 4] {
    [color.r, color.g, color.b, 255]
}

/// Draws a [`SceneFrame`] into an RGBA image of a fixed cell rectangle.
#[derive(Debug, Clone, Copy)]
pub struct PixelRasterizer {
    cells: Rasterizer,
    pixels_per_column: u32,
}

impl PixelRasterizer {
    /// A rasterizer for a `columns` by `rows` scene area, choosing the pixel
    /// scale so the image stays within [`MAX_FRAME_PIXELS`].
    pub fn new(columns: u32, rows: u32) -> Self {
        let columns = columns.max(1);
        let rows = rows.max(1);
        let mut pixels_per_column = DEFAULT_PIXELS_PER_COLUMN;
        while pixels_per_column > MIN_PIXELS_PER_COLUMN
            && u64::from(columns)
                * u64::from(pixels_per_column)
                * u64::from(rows)
                * u64::from(pixels_per_column)
                * (CELL_ASPECT as u64)
                > MAX_FRAME_PIXELS
        {
            pixels_per_column -= 1;
        }
        Self {
            cells: Rasterizer::new(columns, rows),
            pixels_per_column,
        }
    }

    /// Pixels per terminal column the rasterizer settled on.
    pub const fn pixels_per_column(&self) -> u32 {
        self.pixels_per_column
    }

    /// Pixels per terminal row: a cell is twice as tall as it is wide.
    const fn pixels_per_row(&self) -> u32 {
        self.pixels_per_column * (CELL_ASPECT as u32)
    }

    /// The pixel dimensions of a frame this rasterizer produces.
    pub fn image_size(&self) -> (u32, u32) {
        (
            self.cells.width() * self.pixels_per_column,
            self.cells.height() * self.pixels_per_row(),
        )
    }

    /// Pixel position of a world point, row zero at the top.
    pub fn project(&self, viewport: Viewport, point: DVec2) -> (f64, f64) {
        let (col, row) = self.cells.project(viewport, point);
        (
            col * f64::from(self.pixels_per_column),
            row * f64::from(self.pixels_per_row()),
        )
    }

    /// Rasterize one frame: background, geometry, bodies, then overlays.
    pub fn rasterize(&self, frame: &SceneFrame) -> RgbaImage {
        let (width, height) = self.image_size();
        let mut image = RgbaImage::new(width, height);
        image.fill(BACKGROUND);
        if frame.overlays.geometry {
            self.draw_geometry(&mut image, frame);
        }
        self.draw_bodies(&mut image, frame);
        if frame.overlays.vectors {
            self.draw_vectors(&mut image, frame);
        }
        image
    }

    fn draw_geometry(&self, image: &mut RgbaImage, frame: &SceneFrame) {
        for path in frame.geometry.paths() {
            let points: Vec<(f64, f64)> = path
                .points()
                .iter()
                .map(|point| self.project(frame.viewport, *point))
                .collect();
            for pair in points.windows(2) {
                image.segment(pair[0], pair[1], PATH_COLOR, LINE_HALF_WIDTH);
            }
        }

        for portal in frame.geometry.portals() {
            let (start, end) = portal.gate();
            image.segment(
                self.project(frame.viewport, start),
                self.project(frame.viewport, end),
                PORTAL_COLOR,
                LINE_HALF_WIDTH,
            );
            let position = self.project(frame.viewport, portal.position());
            image.segment(
                position,
                self.project(frame.viewport, portal.inward_tip(3.0)),
                PORTAL_COLOR,
                LINE_HALF_WIDTH,
            );
            // A solid cap so the gate reads as a portal, not a plain line.
            image.rect(position.0 - 2.0, position.1 - 2.0, 4.0, 4.0, PORTAL_COLOR);
        }
    }

    fn draw_bodies(&self, image: &mut RgbaImage, frame: &SceneFrame) {
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
                .min(f64::from(image.width())) as i64;
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
                .min(f64::from(image.height())) as i64;

            let (fill, outline) = if selected {
                (SELECTED_COLOR, SELECTED_BACKGROUND)
            } else {
                (BODY_COLOR, BODY_COLOR)
            };
            for row in min_row..max_row {
                for col in min_col..max_col {
                    let world = self.cells.world_at(
                        frame.viewport,
                        col as f64 / f64::from(self.pixels_per_column),
                        row as f64 / f64::from(self.pixels_per_row()),
                    );
                    let offset = world - body.position;
                    if offset.dot(forward).abs() <= half_length
                        && offset.dot(left).abs() <= half_width
                    {
                        image.put(col, row, fill);
                    }
                }
            }
            // Always mark the outline so a sub-pixel body stays visible.
            for pair in [
                (corners[0], corners[1]),
                (corners[1], corners[3]),
                (corners[3], corners[2]),
                (corners[2], corners[0]),
            ] {
                image.segment(
                    self.project(frame.viewport, pair.0),
                    self.project(frame.viewport, pair.1),
                    outline,
                    0,
                );
            }
        }
    }

    fn draw_vectors(&self, image: &mut RgbaImage, frame: &SceneFrame) {
        for body in &frame.bodies {
            let Some(speed) = body.speed_mps else {
                continue;
            };
            let (sin, cos) = body.heading_rad.sin_cos();
            let tip = body.position + DVec2::new(cos, sin) * speed;
            image.segment(
                self.project(frame.viewport, body.position),
                self.project(frame.viewport, tip),
                VECTOR_COLOR,
                0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use tangle_model::{CompiledScenario, parse_scenario_source};
    use tangle_present::{FrameStatus, Overlays, SceneGeometry, Speed};
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

    /// True when any pixel in `image` equals `color`.
    fn contains_color(image: &RgbaImage, color: Rgb) -> bool {
        let target = opaque(color);
        image.rgba().as_chunks::<4>().0.contains(&target)
    }

    #[test]
    fn a_frame_scales_by_the_cell_aspect() {
        let raster = PixelRasterizer::new(40, 20);
        let (width, height) = raster.image_size();
        assert_eq!(raster.pixels_per_column(), DEFAULT_PIXELS_PER_COLUMN);
        assert_eq!(width, 40 * DEFAULT_PIXELS_PER_COLUMN);
        // One row is twice as tall as one column is wide.
        assert_eq!(
            height,
            20 * DEFAULT_PIXELS_PER_COLUMN * (CELL_ASPECT as u32)
        );
    }

    #[test]
    fn a_huge_terminal_shrinks_the_scale_to_the_pixel_budget() {
        let raster = PixelRasterizer::new(500, 200);
        assert!(raster.pixels_per_column() < DEFAULT_PIXELS_PER_COLUMN);
        let (width, height) = raster.image_size();
        assert!(u64::from(width) * u64::from(height) <= MAX_FRAME_PIXELS);
    }

    #[test]
    fn the_same_frame_rasterizes_deterministically() {
        let raster = PixelRasterizer::new(120, 40);
        let frame = frame(Viewport::new(DVec2::ZERO, 0.5));
        assert_eq!(raster.rasterize(&frame), raster.rasterize(&frame));
    }

    #[test]
    fn geometry_and_bodies_are_drawn_in_shared_colors() {
        let raster = PixelRasterizer::new(120, 40);
        let frame = frame(Viewport::new(DVec2::ZERO, 0.5));
        let image = raster.rasterize(&frame);
        assert!(contains_color(&image, PATH_COLOR));
        assert!(contains_color(&image, PORTAL_COLOR));
        assert!(contains_color(&image, BODY_COLOR));
        assert!(contains_color(&image, BACKGROUND));
    }

    #[test]
    fn the_geometry_overlay_can_be_disabled() {
        let raster = PixelRasterizer::new(120, 40);
        let mut frame = frame(Viewport::new(DVec2::ZERO, 0.5));
        frame.overlays.geometry = false;
        let image = raster.rasterize(&frame);
        assert!(!contains_color(&image, PATH_COLOR));
        assert!(!contains_color(&image, PORTAL_COLOR));
    }

    #[test]
    fn a_selected_body_uses_the_selection_color() {
        let raster = PixelRasterizer::new(120, 40);
        let mut frame = frame(Viewport::new(DVec2::ZERO, 0.25));
        frame.status.selection = frame.bodies.first().map(|body| body.id);
        let image = raster.rasterize(&frame);
        assert!(contains_color(&image, SELECTED_COLOR));
    }

    #[test]
    fn out_of_bounds_pixels_are_clipped() {
        let mut image = RgbaImage::new(2, 2);
        image.put(-1, 0, Rgb::WHITE);
        image.put(0, -1, Rgb::WHITE);
        image.put(2, 0, Rgb::WHITE);
        image.put(0, 2, Rgb::WHITE);
        assert!(image.rgba().iter().all(|&byte| byte == 0));
    }
}
