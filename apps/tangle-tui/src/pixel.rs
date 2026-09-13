//! Project a [`SceneFrame`] into an RGBA pixel image.
//!
//! This is the pixel twin of [`crate::raster`]. It deliberately reuses the
//! character rasterizer's projection and its color constants, so the Kitty
//! graphics backend draws the same geometry in the same styles as the
//! character-cell backend; only the sampling grid differs. World metres map to
//! cells with a 1:2 aspect correction, and each cell maps to a fixed block of
//! pixels, so the picture stays square when the terminal scales the image to a
//! cell rectangle.

use std::collections::BTreeMap;

use glam::DVec2;
use tangle_present::{BodyEmphasis, BodyShape, SceneFrame, Viewport};

use crate::palette::Rgb;
use crate::raster::{
    BACKGROUND, BODY_COLOR, BOUNDARY_COLOR, CELL_ASPECT, CONFLICT_COLOR, CROSSING_COLOR,
    FACILITY_COLOR, FACILITY_REFERENCE_COLOR, MOVEMENT_COLOR, OCCUPIED_COLOR, PATH_COLOR,
    PORTAL_COLOR, REGION_COLOR, RULE_COLOR, RULE_MARKER_OFFSET_M, Rasterizer, SELECTED_BACKGROUND,
    SELECTED_COLOR, SIGNAL_COLOR, SIGNAL_GATE_HALF_WIDTH_M, VECTOR_COLOR, emphasis_color,
    marker_color, marker_glyph,
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
/// Chords approximating one capsule cap's semicircle in the pixel backend.
const CAPSULE_CAP_SEGMENTS: usize = 16;

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
        // One emphasis per emphasized body, so a body is styled once. Emphasis
        // is part of the safety overlay, so the overlay switch governs it too.
        let emphasis: BTreeMap<usize, BodyEmphasis> = if frame.overlays.safety {
            frame.body_emphasis().into_iter().collect()
        } else {
            BTreeMap::new()
        };
        self.draw_bodies(&mut image, frame, &emphasis);
        if frame.overlays.safety {
            self.draw_safety(&mut image, frame);
        }
        if frame.overlays.vectors {
            self.draw_vectors(&mut image, frame);
        }
        image
    }

    fn draw_geometry(&self, image: &mut RgbaImage, frame: &SceneFrame) {
        // Boundaries and traversable regions form the static backdrop.
        for boundary in frame.geometry.boundaries() {
            self.draw_ring(image, frame.viewport, boundary.points(), BOUNDARY_COLOR);
        }
        for region in frame.geometry.regions() {
            self.draw_ring(image, frame.viewport, region.points(), REGION_COLOR);
        }

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

        // A movement shares its guide path's polyline but is the routed
        // connector, drawn over the path with its own color and endpoints.
        for movement in frame.geometry.movements() {
            let points: Vec<(f64, f64)> = movement
                .points()
                .iter()
                .map(|point| self.project(frame.viewport, *point))
                .collect();
            for pair in points.windows(2) {
                image.segment(pair[0], pair[1], MOVEMENT_COLOR, LINE_HALF_WIDTH);
            }
            for endpoint in [movement.entry(), movement.exit()] {
                let point = self.project(frame.viewport, endpoint);
                image.rect(point.0 - 1.0, point.1 - 1.0, 2.0, 2.0, MOVEMENT_COLOR);
            }
        }

        // A facility is drawn over the region it occupies and over the guide
        // path and movement it references, so a band and its reference path
        // read as a facility rather than as authored geometry.
        for facility in frame.geometry.facilities() {
            self.draw_ring(image, frame.viewport, facility.points(), FACILITY_COLOR);
            if let Some(reference) = facility.reference() {
                let points: Vec<(f64, f64)> = reference
                    .points()
                    .iter()
                    .map(|point| self.project(frame.viewport, *point))
                    .collect();
                for pair in points.windows(2) {
                    image.segment(pair[0], pair[1], FACILITY_REFERENCE_COLOR, LINE_HALF_WIDTH);
                }
            }
        }

        for conflict in frame.geometry.conflict_regions() {
            self.draw_ring(image, frame.viewport, conflict.points(), CONFLICT_COLOR);
        }
        for crossing in frame.geometry.crossings() {
            self.draw_ring(image, frame.viewport, crossing.points(), CROSSING_COLOR);
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

        for rule in frame.geometry.rules() {
            let (sin, cos) = rule.heading().sin_cos();
            let offset = DVec2::new(-sin, cos) * RULE_MARKER_OFFSET_M;
            let point = self.project(frame.viewport, rule.position() + offset);
            image.rect(point.0 - 1.0, point.1 - 1.0, 2.0, 2.0, RULE_COLOR);
        }

        for signal in frame.geometry.signals() {
            for head in signal.heads() {
                let (sin, cos) = head.heading().sin_cos();
                let offset = DVec2::new(sin, -cos) * SIGNAL_GATE_HALF_WIDTH_M;
                image.segment(
                    self.project(frame.viewport, head.position() + offset),
                    self.project(frame.viewport, head.position() - offset),
                    SIGNAL_COLOR,
                    LINE_HALF_WIDTH,
                );
                let point = self.project(frame.viewport, head.position());
                image.rect(point.0 - 1.0, point.1 - 1.0, 2.0, 2.0, SIGNAL_COLOR);
            }
        }
    }

    /// Draw a closed polygon outline, connecting the last vertex to the first.
    fn draw_ring(&self, image: &mut RgbaImage, viewport: Viewport, points: &[DVec2], color: Rgb) {
        if points.len() < 2 {
            return;
        }
        for index in 0..points.len() {
            image.segment(
                self.project(viewport, points[index]),
                self.project(viewport, points[(index + 1) % points.len()]),
                color,
                LINE_HALF_WIDTH,
            );
        }
    }

    fn draw_bodies(
        &self,
        image: &mut RgbaImage,
        frame: &SceneFrame,
        emphasis: &BTreeMap<usize, BodyEmphasis>,
    ) {
        for body in &frame.bodies {
            let selected = frame.status.selection == Some(body.id);
            let (fill, outline) = if selected {
                (SELECTED_COLOR, SELECTED_BACKGROUND)
            } else if let Some(emphasis) = emphasis.get(&body.id) {
                let color = emphasis_color(*emphasis);
                (color, color)
            } else {
                (BODY_COLOR, BODY_COLOR)
            };
            // A body draws the shapes the shared scene projection chooses for
            // it: one box, circle, or capsule, or one box per ordered segment.
            // The choice reads scene data, so no presenter branch names a mode
            // or scenario.
            for shape in body.shapes() {
                match shape {
                    BodyShape::Circle { center, radius_m } => {
                        self.draw_circle(image, frame.viewport, center, radius_m, fill);
                        // A mark at the centre keeps a sub-pixel circle visible.
                        let point = self.project(frame.viewport, center);
                        image.put(point.0.round() as i64, point.1.round() as i64, fill);
                    }
                    BodyShape::Box {
                        center,
                        heading_rad,
                        length_m,
                        width_m,
                    } => self.draw_box(
                        image,
                        frame.viewport,
                        center,
                        heading_rad,
                        length_m,
                        width_m,
                        fill,
                        outline,
                    ),
                    BodyShape::Capsule {
                        center,
                        heading_rad,
                        length_m,
                        radius_m,
                    } => self.draw_capsule(
                        image,
                        frame.viewport,
                        center,
                        heading_rad,
                        length_m,
                        radius_m,
                        fill,
                        outline,
                    ),
                }
            }
        }
    }

    /// Fill a capsule and outline its two sides and its two caps.
    ///
    /// A pixel is inside when it lies within `radius_m` of the straight
    /// segment, which is exactly the capsule; no rectangle covers a capsule,
    /// and neither would one drawn at the body's bounding extent. The outline
    /// traces the two sides the segment spans and each cap's semicircle, the
    /// last approximated by [`CAPSULE_CAP_SEGMENTS`] chords.
    #[allow(clippy::too_many_arguments)]
    fn draw_capsule(
        &self,
        image: &mut RgbaImage,
        viewport: Viewport,
        center: DVec2,
        heading_rad: f64,
        length_m: f64,
        radius_m: f64,
        fill: Rgb,
        outline: Rgb,
    ) {
        let (sin, cos) = heading_rad.sin_cos();
        let forward = DVec2::new(cos, sin);
        let left = DVec2::new(-sin, cos);
        let half_length = length_m * 0.5;

        let corner = DVec2::splat(half_length + radius_m);
        let low = self.project(viewport, center - corner);
        let high = self.project(viewport, center + corner);
        let min_col = low.0.min(high.0).floor().max(0.0) as i64;
        let max_col = low.0.max(high.0).ceil().min(f64::from(image.width())) as i64;
        let min_row = low.1.min(high.1).floor().max(0.0) as i64;
        let max_row = low.1.max(high.1).ceil().min(f64::from(image.height())) as i64;

        for row in min_row..max_row {
            for col in min_col..max_col {
                let world = self.cells.world_at(
                    viewport,
                    col as f64 / f64::from(self.pixels_per_column),
                    row as f64 / f64::from(self.pixels_per_row()),
                );
                let along = (world - center)
                    .dot(forward)
                    .clamp(-half_length, half_length);
                if (world - (center + forward * along)).length() <= radius_m {
                    image.put(col, row, fill);
                }
            }
        }

        // The two straight sides, each `radius_m` out along the normal.
        let near = center - forward * half_length;
        let far = center + forward * half_length;
        for side in [1.0, -1.0] {
            image.segment(
                self.project(viewport, near + left * (radius_m * side)),
                self.project(viewport, far + left * (radius_m * side)),
                outline,
                0,
            );
        }
        // Each cap sweeps the half-turn around its end of the segment.
        let start = heading_rad + std::f64::consts::FRAC_PI_2;
        let step = std::f64::consts::PI / CAPSULE_CAP_SEGMENTS as f64;
        for pivot in [far, near] {
            for chord in 0..CAPSULE_CAP_SEGMENTS {
                let at = |angle: f64| {
                    self.project(
                        viewport,
                        pivot + DVec2::new(angle.cos(), angle.sin()) * radius_m,
                    )
                };
                image.segment(
                    at(start + step * chord as f64),
                    at(start + step * (chord + 1) as f64),
                    outline,
                    0,
                );
            }
        }
    }

    /// Fill an oriented box and outline its four edges.
    #[allow(clippy::too_many_arguments)]
    fn draw_box(
        &self,
        image: &mut RgbaImage,
        viewport: Viewport,
        center: DVec2,
        heading_rad: f64,
        length_m: f64,
        width_m: f64,
        fill: Rgb,
        outline: Rgb,
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

        for row in min_row..max_row {
            for col in min_col..max_col {
                let world = self.cells.world_at(
                    viewport,
                    col as f64 / f64::from(self.pixels_per_column),
                    row as f64 / f64::from(self.pixels_per_row()),
                );
                let offset = world - center;
                if offset.dot(forward).abs() <= half_length && offset.dot(left).abs() <= half_width
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
                self.project(viewport, pair.0),
                self.project(viewport, pair.1),
                outline,
                0,
            );
        }
    }

    /// Fill the pixels inside a circle of `radius_m` metres.
    fn draw_circle(
        &self,
        image: &mut RgbaImage,
        viewport: Viewport,
        center: DVec2,
        radius_m: f64,
        color: Rgb,
    ) {
        let corner = DVec2::splat(radius_m);
        let low = self.project(viewport, center - corner);
        let high = self.project(viewport, center + corner);
        let min_col = low.0.min(high.0).floor().max(0.0) as i64;
        let max_col = low.0.max(high.0).ceil().min(f64::from(image.width())) as i64;
        let min_row = low.1.min(high.1).floor().max(0.0) as i64;
        let max_row = low.1.max(high.1).ceil().min(f64::from(image.height())) as i64;

        for row in min_row..max_row {
            for col in min_col..max_col {
                let world = self.cells.world_at(
                    viewport,
                    col as f64 / f64::from(self.pixels_per_column),
                    row as f64 / f64::from(self.pixels_per_row()),
                );
                if (world - center).length() <= radius_m {
                    image.put(col, row, color);
                }
            }
        }
    }

    /// Draw the frame's safety overlays: occupied-region outlines and event
    /// markers, both derived from the frame's projected safety data.
    fn draw_safety(&self, image: &mut RgbaImage, frame: &SceneFrame) {
        for region in frame.occupied_regions() {
            let Some(points) = frame.region_points(region.region()) else {
                continue;
            };
            self.draw_ring(image, frame.viewport, points, OCCUPIED_COLOR);
        }

        for marker in frame.safety_markers() {
            let Some(_) = marker_glyph(marker.kind()) else {
                continue;
            };
            let color = marker_color(marker.kind());
            let (x, y) = self.project(frame.viewport, marker.position());
            // A cross, so a marker stays visible over a body's outline.
            image.segment((x - 2.0, y), (x + 2.0, y), color, 0);
            image.segment((x, y - 2.0), (x, y + 2.0), color, 0);
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

    use tangle_model::{BodyKind, CompiledScenario, CrossingId, parse_scenario_source};
    use tangle_present::{
        FrameStatus, Overlays, SafetyOverlay, SceneGeometry, Speed, load_scenario,
    };
    use tangle_sim::{
        AgentId, AgentMode, BodySegmentSample, Event, RegionKey, RunConfig, Simulation,
        SnapshotDetail,
    };

    use crate::raster::{COLLISION_COLOR, QUEUE_COLOR};

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

    /// True when any pixel in `image` equals `color`.
    fn contains_color(image: &RgbaImage, color: Rgb) -> bool {
        let target = opaque(color);
        image.rgba().as_chunks::<4>().0.contains(&target)
    }

    /// A geometry-only frame for a scenario carrying every general primitive.
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

    /// One still vehicle body, for a frame the simulation does not need to
    /// produce.
    fn vehicle(id: usize, position: DVec2) -> tangle_present::SceneBody {
        tangle_present::SceneBody {
            id,
            position,
            heading_rad: 0.0,
            length_m: 4.5,
            width_m: 1.8,
            mode: AgentMode::Vehicle,
            body_kind: AgentMode::Vehicle.body_kind(),
            segments: Vec::new(),
            speed_mps: Some(0.0),
            path: None,
            path_distance_m: None,
            route: None,
            profile: None,
            decision: None,
        }
    }

    /// A frame over the general-primitive scenario, carrying a region entry, a
    /// contact, and a standstill: every family the safety overlay draws.
    fn safety_frame() -> SceneFrame {
        let mut frame = general_frame(Viewport::new(DVec2::ZERO, 0.4));
        frame.bodies = vec![vehicle(0, DVec2::ZERO), vehicle(1, DVec2::new(6.0, 0.0))];
        frame.status.agents = frame.bodies.len();
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
        frame.safety = safety;
        frame
    }

    #[test]
    fn safety_occupancy_emphasis_and_markers_are_drawn() {
        let raster = PixelRasterizer::new(160, 80);
        let image = raster.rasterize(&safety_frame());
        assert!(contains_color(&image, OCCUPIED_COLOR));
        assert!(contains_color(&image, COLLISION_COLOR));
        assert!(contains_color(&image, QUEUE_COLOR));
    }

    #[test]
    fn the_safety_overlay_can_be_disabled() {
        let raster = PixelRasterizer::new(160, 80);
        let mut frame = safety_frame();
        frame.overlays.safety = false;
        let image = raster.rasterize(&frame);
        assert!(!contains_color(&image, OCCUPIED_COLOR));
        assert!(!contains_color(&image, COLLISION_COLOR));
        assert!(!contains_color(&image, QUEUE_COLOR));
        assert!(contains_color(&image, BODY_COLOR));
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

    /// A facility's occupied band and its reference path draw from scene data
    /// in the pixel backend's own colors, so the Kitty graphics backend shows
    /// the same facility the character-cell backend does.
    #[test]
    fn a_facility_band_and_reference_path_are_drawn_from_scene_data() {
        let raster = PixelRasterizer::new(120, 40);
        let image = raster.rasterize(&facility_frame(Viewport::new(DVec2::new(110.0, 0.0), 0.5)));
        assert!(
            contains_color(&image, FACILITY_COLOR),
            "the facility band was not drawn"
        );
        assert!(
            contains_color(&image, FACILITY_REFERENCE_COLOR),
            "the facility reference path was not drawn"
        );
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

    #[test]
    fn every_general_primitive_is_drawn_in_its_own_color() {
        let raster = PixelRasterizer::new(160, 80);
        let frame = general_frame(Viewport::new(DVec2::ZERO, 0.4));
        let image = raster.rasterize(&frame);
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
                contains_color(&image, color),
                "{name} primitive was not drawn in its color"
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
    ) -> tangle_present::SceneBody {
        tangle_present::SceneBody {
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

    /// A frame over an empty geometry carrying only `bodies`.
    fn synthetic_frame(bodies: Vec<tangle_present::SceneBody>) -> SceneFrame {
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

    /// The RGBA bytes at an integer pixel coordinate.
    fn pixel_at(image: &RgbaImage, x: i64, y: i64) -> [u8; 4] {
        let index = ((y as u32 * image.width() + x as u32) * 4) as usize;
        image.rgba()[index..index + 4]
            .try_into()
            .expect("four bytes")
    }

    /// The pixel the world point `point` projects to.
    fn pixel_of(
        raster: &PixelRasterizer,
        viewport: Viewport,
        image: &RgbaImage,
        point: DVec2,
    ) -> [u8; 4] {
        let (x, y) = raster.project(viewport, point);
        pixel_at(image, x.round() as i64, y.round() as i64)
    }

    /// A box body and a circle body draw visibly different pixels, and a body
    /// carrying ordered segments draws each segment at its own pose: a box
    /// fills its bounding square's corner that a circle of the same extent
    /// leaves empty, and both segment centres carry the body color.
    #[test]
    fn each_body_kind_and_ordered_segment_is_drawn_from_scene_data() {
        let raster = PixelRasterizer::new(80, 40);
        let viewport = Viewport::new(DVec2::ZERO, 1.0);
        // 80x40 cells at scale 1: one cell is one metre wide, two metres tall.
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
        let image = raster.rasterize(&synthetic_frame(vec![box_body, circle, articulated]));
        let body_color = opaque(BODY_COLOR);

        assert_eq!(
            pixel_of(&raster, viewport, &image, DVec2::new(-13.0, 2.0)),
            body_color,
            "the box must fill its corner"
        );
        assert_ne!(
            pixel_of(&raster, viewport, &image, DVec2::new(13.0, 2.0)),
            body_color,
            "the circle must not fill its bounding square's corner"
        );
        assert_eq!(
            pixel_of(&raster, viewport, &image, DVec2::new(-10.0, 0.0)),
            body_color,
            "the box centre must draw"
        );
        assert_eq!(
            pixel_of(&raster, viewport, &image, DVec2::new(10.0, 0.0)),
            body_color,
            "the circle centre must draw"
        );
        assert_eq!(
            pixel_of(&raster, viewport, &image, DVec2::new(-2.0, 8.0)),
            body_color,
            "segment 0 must draw at its pose"
        );
        assert_eq!(
            pixel_of(&raster, viewport, &image, DVec2::new(2.0, 8.0)),
            body_color,
            "segment 1 must draw at its pose"
        );
    }

    /// A capsule body draws its straight part and both caps: a pixel inside a
    /// cap beyond the straight part carries the body, and the rectangle that
    /// bounds the capsule fills the corner the cap curves away from.
    #[test]
    fn a_capsule_body_is_drawn_as_a_capsule() {
        let raster = PixelRasterizer::new(80, 40);
        // One pixel is 1/24 m across, so a 1 m cap is resolvable.
        let viewport = Viewport::new(DVec2::ZERO, 0.25);
        let mut capsule_frame =
            synthetic_frame(vec![body(0, BodyKind::Capsule, DVec2::ZERO, 6.0, 2.0)]);
        capsule_frame.viewport = viewport;
        let capsule = raster.rasterize(&capsule_frame);
        let body_color = opaque(BODY_COLOR);

        // The straight part: world (2, 0.5) is inside the 6 x 2 rectangle.
        assert_eq!(
            pixel_of(&raster, viewport, &capsule, DVec2::new(2.0, 0.5)),
            body_color,
            "the straight part must draw"
        );
        // Each cap: world (+-3.75, 0) is 0.75 m beyond the straight part's end
        // and within the 1 m cap radius, which no rectangle of the reported
        // length covers.
        assert_eq!(
            pixel_of(&raster, viewport, &capsule, DVec2::new(3.75, 0.0)),
            body_color,
            "the front cap must draw"
        );
        assert_eq!(
            pixel_of(&raster, viewport, &capsule, DVec2::new(-3.75, 0.0)),
            body_color,
            "the rear cap must draw"
        );

        // The rectangle bounding the capsule, drawn as a box body of the same
        // extent, fills the corner world (3.75, 1); the capsule leaves it
        // empty because the cap curves away from it.
        let mut bounding_frame =
            synthetic_frame(vec![body(0, BodyKind::Box, DVec2::ZERO, 8.0, 2.0)]);
        bounding_frame.viewport = viewport;
        let bounding = raster.rasterize(&bounding_frame);
        assert_eq!(
            pixel_of(&raster, viewport, &bounding, DVec2::new(3.75, 1.0)),
            body_color,
            "the bounding box fills its corner"
        );
        assert_ne!(
            pixel_of(&raster, viewport, &capsule, DVec2::new(3.75, 1.0)),
            body_color,
            "the capsule must not fill the bounding rectangle's corner"
        );
    }
}
