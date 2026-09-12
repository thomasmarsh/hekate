//! Synthetic raster scenes for the spike.
//!
//! The walking scene mirrors the scale of `scenarios/walking/walking_guide_v1`
//! (a straight guide with six constant-speed cars) so the measured cost is
//! representative of the Increment 0 walking skeleton. The dense scene adds
//! many small moving bodies and per-frame color noise so compression has a
//! realistic, hard case. Neither scene touches `tangle-sim`: the spike is
//! deliberately independent so it can fail cheaply.

/// Deterministic scene definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneSpec {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// World width represented by the frame, in metres.
    pub world_width_m: u32,
    pub dense: bool,
}

/// Look up a named scene spec.
pub fn spec(name: &str) -> SceneSpec {
    match name {
        "dense" => SceneSpec {
            name: "dense".to_string(),
            width: 960,
            height: 540,
            world_width_m: 120,
            dense: true,
        },
        _ => SceneSpec {
            name: "walking".to_string(),
            width: 480,
            height: 270,
            world_width_m: 130,
            dense: false,
        },
    }
}

/// Render frame `frame` as tightly packed RGBA rows.
pub fn render(spec: &SceneSpec, frame: u32) -> Vec<u8> {
    let mut fb = Framebuffer::new(spec.width, spec.height);
    let background = [12, 14, 20, 255];
    fb.fill(background);

    let scale = spec.width as f64 / f64::from(spec.world_width_m);
    let road_y = (spec.height as f64 * 0.5).round();
    let road_height = (7.0 * scale).max(8.0);

    // Road, then dashed centre line, then a border that flips colour each
    // second so animation is unmistakable.
    fb.rect(
        0.0,
        road_y - road_height / 2.0,
        spec.width as f64,
        road_height,
        [48, 50, 58, 255],
    );
    let dash = 6.0 * scale;
    let mut x = 0.0;
    while x < spec.width as f64 {
        fb.rect(x, road_y - 1.0, dash, 2.0, [200, 200, 170, 255]);
        x += dash * 2.0;
    }
    let border = if (frame / 30).is_multiple_of(2) {
        [80, 220, 140, 255]
    } else {
        [220, 140, 80, 255]
    };
    fb.frame(3.0, border);

    let t = f64::from(frame) / 60.0;
    if spec.dense {
        render_dense(&mut fb, spec, scale, t, road_y);
        add_sensor_noise(&mut fb, frame);
    } else {
        render_walking(&mut fb, spec, scale, t, road_y);
    }

    // Progress bar across the bottom: its width tracks `frame`, so a stalled
    // or dropped frame is visible as a frozen bar.
    let progress = (f64::from(frame % 120) / 120.0 * spec.width as f64) as u32;
    fb.rect(
        0.0,
        spec.height as f64 - 4.0,
        progress as f64,
        4.0,
        [230, 230, 240, 255],
    );
    fb.rgba
}

/// Six cars on a straight guide, matching the walking-skeleton scenario scale.
fn render_walking(fb: &mut Framebuffer, spec: &SceneSpec, scale: f64, t: f64, road_y: f64) {
    let lane_y = road_y - 1.6 * scale;
    for index in 0..6 {
        let start = f64::from(index) * 20.0;
        let distance = (start + 12.0 * t) % 130.0;
        let x = distance * scale;
        let y = if index % 2 == 0 {
            lane_y
        } else {
            lane_y + 3.2 * scale
        };
        let color = if index % 2 == 0 {
            [90, 170, 255, 255]
        } else {
            [255, 210, 90, 255]
        };
        fb.rect(x, y, 4.5 * scale, 1.8 * scale, color);
        // A heading tick so sub-cell motion is visible on every frame.
        fb.rect(
            x + 4.5 * scale,
            y + 0.6 * scale,
            2.0,
            2.0,
            [255, 255, 255, 255],
        );
    }
    // One pedestrian crossing the road on a diagonal.
    let px = (20.0 + 8.0 * t).rem_euclid(110.0) * scale;
    let py = road_y - 6.0 * scale + (t * 2.0).rem_euclid(6.0) * scale;
    fb.circle(px, py, 0.9 * scale, [255, 120, 160, 255]);
    let _ = spec;
}

/// Many small bodies plus per-frame noise: the hard compression case.
fn render_dense(fb: &mut Framebuffer, spec: &SceneSpec, scale: f64, t: f64, road_y: f64) {
    let mut rng = SplitMix64::new(0x7a11_7a11);
    for index in 0..90 {
        let speed = 4.0 + (index % 7) as f64 * 2.5;
        let lane = (index % 12) as f64;
        let x = ((index as f64 * 3.1 + speed * t) % 120.0) * scale;
        let y = road_y - 6.0 * scale + lane * 1.0 * scale;
        let color = [
            (rng.next() % 200 + 55) as u8,
            (rng.next() % 200 + 55) as u8,
            (rng.next() % 200 + 55) as u8,
            255,
        ];
        fb.rect(x, y, 2.6 * scale, 1.2 * scale, color);
    }
    for index in 0..40 {
        let px = (index as f64 * 2.7 + 2.0 * t).rem_euclid(118.0) * scale;
        let py = 6.0 * scale + (index as f64 * 1.9).rem_euclid(50.0) * scale;
        fb.circle(px, py, 0.5 * scale, [120, 230, 180, 255]);
    }
    // A moving highlight so adjacent frames differ even at zero speed.
    let hx = (t * 30.0).rem_euclid(110.0) * scale;
    fb.circle(hx, road_y, 1.4 * scale, [255, 255, 255, 255]);
    let _ = spec;
}

/// A trivial software framebuffer: RGBA8, no dependencies, no floating blur.
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn fill(&mut self, color: [u8; 4]) {
        for pixel in self.rgba.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&color);
        }
    }

    pub fn put(&mut self, x: i64, y: i64, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let index = ((y as u32 * self.width + x as u32) * 4) as usize;
        self.rgba[index..index + 4].copy_from_slice(&color);
    }

    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: [u8; 4]) {
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

    pub fn circle(&mut self, cx: f64, cy: f64, radius: f64, color: [u8; 4]) {
        let r = radius.max(0.5);
        let x0 = (cx - r).floor() as i64;
        let y0 = (cy - r).floor() as i64;
        let x1 = (cx + r).ceil() as i64;
        let y1 = (cy + r).ceil() as i64;
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f64 + 0.5 - cx;
                let dy = py as f64 + 0.5 - cy;
                if dx * dx + dy * dy <= r * r {
                    self.put(px, py, color);
                }
            }
        }
    }

    pub fn frame(&mut self, inset: f64, color: [u8; 4]) {
        self.rect(inset, inset, self.width as f64 - inset, 1.0, color);
        self.rect(
            inset,
            self.height as f64 - inset,
            self.width as f64 - inset,
            1.0,
            color,
        );
        self.rect(inset, inset, 1.0, self.height as f64 - inset, color);
        self.rect(
            self.width as f64 - inset,
            inset,
            1.0,
            self.height as f64 - inset,
            color,
        );
    }
}

/// Add deterministic per-pixel sensor noise so the dense scene is a genuine
/// worst case for compression: the walking scene is flat debug geometry that
/// zlib crushes, while a noisy raster should approach raw size. Without this,
/// the "dense" scene still compresses ~300x and would overstate the protocol.
fn add_sensor_noise(fb: &mut Framebuffer, frame: u32) {
    const AMPLITUDE: i32 = 22;
    for (index, pixel) in fb.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = index as u32 % fb.width;
        let y = index as u32 / fb.width;
        let mut hash = x
            .wrapping_mul(0x9e37_79b1)
            .wrapping_add(y.wrapping_mul(0x85eb_ca6b))
            .wrapping_add(frame.wrapping_mul(0xc2b2_ae35));
        hash ^= hash >> 15;
        let noise = (hash & 0xff) as i32 - 128;
        let delta = noise * AMPLITUDE / 128;
        for channel in &mut pixel[..3] {
            *channel = (i32::from(*channel) + delta).clamp(0, 255) as u8;
        }
    }
}

/// SplitMix64: deterministic, seedable, and dependency-free.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_deterministic_and_sized() {
        let spec = spec("walking");
        let first = render(&spec, 7);
        let second = render(&spec, 7);
        assert_eq!(first, second);
        assert_eq!(first.len(), (spec.width * spec.height * 4) as usize);
    }

    #[test]
    fn adjacent_frames_differ() {
        let walking = spec("walking");
        assert_ne!(render(&walking, 10), render(&walking, 11));
        let dense = spec("dense");
        assert_ne!(render(&dense, 10), render(&dense, 11));
    }

    #[test]
    fn dense_scene_compresses_worse_than_walking() {
        use crate::kitty::zlib;
        let walking = render(&spec("walking"), 3);
        let dense = render(&spec("dense"), 3);
        let walking_ratio = zlib(&walking).len() as f64 / walking.len() as f64;
        let dense_ratio = zlib(&dense).len() as f64 / dense.len() as f64;
        assert!(walking_ratio < 1.0);
        assert!(dense_ratio < 1.0);
        // The dense scene carries noise, so it must not compress better than
        // the flat walking scene.
        assert!(dense_ratio > walking_ratio);
    }
}
