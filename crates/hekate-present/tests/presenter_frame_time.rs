//! TAS-110: headless presenter frame time for the Increment 2 profile.
//!
//! The profiled and benchmarked runs are headless, so the presenter's own frame
//! cost is measured separately: the shared, backend-agnostic projection every
//! renderer consumes ([`PresentationController::project`]) over a bounded set of
//! the profile's own snapshot pairs. No Bevy viewer is started and no backend
//! draws; this is the projection cost alone, which is what a frame's bodies,
//! status, viewport, and overlays cost to build.
//!
//! ```sh
//! cargo test --release -p hekate-present --test presenter_frame_time -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glam::DVec2;
use hekate_present::{PresentationController, SceneGeometry, Viewport, load_scenario};
use hekate_sim::{RunConfig, Simulation, SnapshotDetail};

/// The checked-in Increment 2 representative mixed-mode profile.
const PROFILE: &str = "scenarios/phase2/inc2/mixed_mode_profile_v2.json5";
/// The fixture's seed bank entry the profile measures on.
const SEED: u64 = 11;
/// The default fixed step, the presenter's interpolation step.
const STEP_SECS: f64 = 0.05;
/// Ticks stepped before the timed window, so the corridor is populated and
/// passing. 3 000 ticks is 150 simulated seconds, well past the ~44 s a car
/// needs to cross, so cars are catching the slower leaders.
const WARMUP_TICKS: u64 = 3000;
/// Frames projected in the timed window. 5 000 distinct pairs give a stable
/// microsecond-per-frame figure at a sub-percent resolution.
const FRAMES: usize = 5000;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
#[ignore = "release-mode wall-clock measurement; run with --release --ignored"]
fn increment_2_profile_frame_time() {
    let scenario = Arc::new(load_scenario(&repo_path(PROFILE)).expect("the profile loads"));
    let mut sim =
        Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("the profile builds");
    for _ in 0..WARMUP_TICKS {
        sim.step();
    }

    // Distinct, populated frame pairs the presenter interpolates between, built
    // once outside the timed interval.
    let mut pairs = Vec::with_capacity(FRAMES);
    let mut previous = sim.snapshot(SnapshotDetail::Full);
    for _ in 0..FRAMES {
        sim.step();
        let current = sim.snapshot(SnapshotDetail::Full);
        pairs.push((previous, current.clone()));
        previous = current;
    }

    let geometry = SceneGeometry::from_scenario(&scenario);
    let controller =
        PresentationController::new(geometry, STEP_SECS, Viewport::new(DVec2::ZERO, 1.0));
    // One warm projection so the first timed frame does not pay first-touch.
    let _ = controller.project(&pairs[0].0, &pairs[0].1);

    let start = Instant::now();
    let mut body_frames = 0usize;
    for (previous, current) in &pairs {
        let frame = controller.project(previous, current);
        body_frames += frame.bodies.len();
        std::hint::black_box(&frame);
    }
    let secs = start.elapsed().as_secs_f64();
    let per_frame_us = secs * 1e6 / FRAMES as f64;
    let bodies_per_frame = body_frames as f64 / FRAMES as f64;
    println!(
        "presenter: frames={FRAMES} wall={secs:.4}s us_per_frame={per_frame_us:.3} \
         bodies_per_frame={bodies_per_frame:.3}"
    );
    assert!(body_frames > 0, "the projection must draw live bodies");
}
