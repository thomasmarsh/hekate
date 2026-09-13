//! Measure the Kitty graphics backend's frame cost and transmit size.
//!
//! Run with `cargo run -p tangle-tui --release --example kitty-bench`.
//!
//! By default the backend writes to a discarding sink, which measures raster
//! and base64 encode cost only. Set `TANGLE_BENCH_PTY=1` and run the binary
//! under a pty (for example `script -q /dev/null ...`) to include the terminal
//! write cost. `TANGLE_BENCH_FRAMES` overrides the frame count.
//!
//! Two cases are reported:
//!
//! - `walking`: the checked-in walking scenario through a real [`TuiSession`],
//!   so the number covers the whole project-draw-present path.
//! - `dense`: a synthetic frame with many small bodies at a larger viewport, to
//!   show raster and transmit cost when the picture is not flat debug geometry.
//!
//! Transmit is raw RGBA (`a=t,f=32`) via chunked base64, so cost scales with
//! pixel count and never with compressibility.

use std::hint::black_box;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::DVec2;
use tangle_model::CompiledScenario;
use tangle_present::{
    FrameStatus, Overlays, RendererBackend, SafetyOverlay, SceneBody, SceneFrame, SceneGeometry,
    Speed, Viewport, load_scenario,
};
use tangle_sim::AgentMode;
use tangle_tui::{KittyBackend, Multiplexer, TuiSession};

/// The example's boxed error type, matching the backend contract.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Default frames measured per case.
const DEFAULT_FRAMES: u32 = 240;

/// A sink that discards bytes; timing, not output, is the measurement.
#[derive(Default)]
struct NullSink;

impl Write for NullSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() -> Result<(), BoxError> {
    let scenario = walking()?;
    let frames = frames();
    if std::env::var_os("TANGLE_BENCH_PTY").is_some() {
        println!(
            "{}",
            measure_walking(io::stdout(), Arc::clone(&scenario), frames)?
        );
        println!("{}", measure_dense(io::stdout(), scenario, frames)?);
    } else {
        println!(
            "{}",
            measure_walking(NullSink, Arc::clone(&scenario), frames)?
        );
        println!("{}", measure_dense(NullSink, scenario, frames)?);
    }
    Ok(())
}

fn frames() -> u32 {
    std::env::var("TANGLE_BENCH_FRAMES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_FRAMES)
}

fn walking() -> Result<Arc<CompiledScenario>, BoxError> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/walking/walking_guide_v1.json5");
    Ok(Arc::new(load_scenario(&path)?))
}

fn measure_walking<W: Write>(
    writer: W,
    scenario: Arc<CompiledScenario>,
    frames: u32,
) -> Result<String, BoxError> {
    let backend = KittyBackend::with_mux(writer, Arc::clone(&scenario), Multiplexer::None);
    let mut session = TuiSession::with_backend(scenario, 0, backend)?;
    session.resize(80, 24);
    let (width, height) = session.backend().image_size();

    // Warm up, then measure.
    for _ in 0..10 {
        session.advance_and_render(1.0 / 60.0)?;
    }
    let before = session.backend().wire_bytes_sent();
    let start = Instant::now();
    for _ in 0..frames {
        session.advance_and_render(1.0 / 60.0)?;
        black_box(session.tick());
    }
    let elapsed = start.elapsed();
    let wire = session.backend().wire_bytes_sent() - before;
    Ok(report("walking", width, height, frames, elapsed, wire))
}

fn measure_dense<W: Write>(
    writer: W,
    scenario: Arc<CompiledScenario>,
    frames: u32,
) -> Result<String, BoxError> {
    let geometry = Arc::new(SceneGeometry::from_scenario(&scenario));
    let mut backend = KittyBackend::with_mux(writer, scenario, Multiplexer::None);
    backend.resize_terminal(160, 48)?;
    let (width, height) = backend.image_size();

    let bodies: Vec<SceneBody> = (0..200)
        .map(|index| {
            let lane = (index % 20) as f64;
            SceneBody {
                id: index,
                position: DVec2::new((index as f64 * 1.7) % 180.0, lane * 1.4 - 14.0),
                heading_rad: (index as f64 * 0.11).sin(),
                length_m: 2.6,
                width_m: 1.2,
                mode: AgentMode::Vehicle,
                body_kind: AgentMode::Vehicle.body_kind(),
                segments: Vec::new(),
                speed_mps: Some(6.0),
                path: None,
                path_distance_m: None,
                route: None,
                profile: None,
                decision: None,
            }
        })
        .collect();

    let mut frame = SceneFrame {
        scenario_id: "dense".to_owned(),
        time_seconds: 0.0,
        tick: 0,
        status: FrameStatus {
            agents: bodies.len(),
            speed: Speed::Real,
            paused: false,
            selection: None,
        },
        viewport: Viewport::new(DVec2::new(90.0, 0.0), 0.4),
        geometry,
        bodies,
        overlays: Overlays::default(),
        safety: SafetyOverlay::default(),
    };

    for _ in 0..10 {
        backend.draw(&frame)?;
        backend.present()?;
    }
    let before = backend.wire_bytes_sent();
    let start = Instant::now();
    for frame_index in 0..frames {
        frame.tick = u64::from(frame_index);
        frame.time_seconds = f64::from(frame_index) / 60.0;
        backend.draw(&frame)?;
        backend.present()?;
    }
    let elapsed = start.elapsed();
    let wire = backend.wire_bytes_sent() - before;
    Ok(report("dense", width, height, frames, elapsed, wire))
}

fn report(
    name: &str,
    width: u32,
    height: u32,
    frames: u32,
    elapsed: Duration,
    wire: u64,
) -> String {
    let per_frame = elapsed.as_secs_f64() * 1000.0 / f64::from(frames);
    let wire_per_frame = wire as f64 / f64::from(frames);
    format!(
        "{name:>7}: {width}x{height}px  {per_frame:.2} ms/frame ({:.0} fps)  \
         {wire_per_frame:.0} bytes/frame on the wire",
        (1000.0 / per_frame).round(),
    )
}
