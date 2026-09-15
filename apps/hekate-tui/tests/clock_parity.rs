//! Clock parity across the two renderers and direct stepping.
//!
//! The Bevy viewer and the terminal session both drive the kernel through the
//! shared `PresentationController` and its `PresentationClock`. This test runs
//! one seed three ways at the Standard step:
//!
//! - direct stepping, exactly `TICKS` calls to `Simulation::step`;
//! - the Bevy viewer's clock, advanced over an irregular frame partition;
//! - the terminal session's clock, advanced over many equal small frames.
//!
//! All three must reach the same ticks and hash to the same canonical trace.
//! The real `TuiSession` is then driven with the terminal partition to confirm
//! its actual loop lands on the same state, not just a mirror of it.

use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec2;
use hekate_cli::{TraceRecorder, canonical_trace};
use hekate_model::CompiledScenario;
use hekate_present::{PresentationController, SceneGeometry, Viewport, load_scenario};
use hekate_sim::{RunConfig, Simulation, SnapshotDetail};
use hekate_tui::{ColorDepth, TuiSession};

const SEED: u64 = 0;
const TICKS: u64 = 20;
const STEP_SECS: f64 = 0.05;
/// The frame partition the Bevy viewer sees on a stuttering host.
const BEVY_FRAMES: [f64; 7] = [0.17, 0.03, 0.22, 0.05, 0.31, 0.001, 0.219];

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn walking() -> Arc<CompiledScenario> {
    let path = repo_path("scenarios/walking/walking_guide_v1.json5");
    Arc::new(load_scenario(&path).expect("walking scenario loads"))
}

/// Drive a fresh simulation through the shared controller over `frames` and
/// return its canonical trace hash plus the tick reached.
fn drive_with_controller(scenario: &Arc<CompiledScenario>, frames: &[f64]) -> (String, u64) {
    let mut sim = Simulation::new((**scenario).clone(), RunConfig::new(SEED)).expect("builds");
    let geometry = SceneGeometry::from_scenario(scenario);
    let mut controller =
        PresentationController::new(geometry, STEP_SECS, Viewport::new(DVec2::ZERO, 1.0));
    let mut recorder = TraceRecorder::new(&sim, &sim.config(), TICKS);

    for &frame_secs in frames {
        let ticks = controller.advance(frame_secs);
        for _ in 0..ticks {
            recorder.record(&sim.step());
        }
    }
    let ticks = sim.time().tick();
    (recorder.finish(sim.finish()).hash().to_owned(), ticks)
}

#[test]
fn every_clock_visits_the_same_ticks_and_trace_hash() {
    let scenario = walking();
    let terminal_frames: Vec<f64> = std::iter::repeat_n(0.01, 100).collect();

    let direct =
        canonical_trace((*scenario).clone(), RunConfig::new(SEED), TICKS).expect("direct run");
    let (bevy_hash, bevy_ticks) = drive_with_controller(&scenario, &BEVY_FRAMES);
    let (terminal_hash, terminal_ticks) = drive_with_controller(&scenario, &terminal_frames);

    assert_eq!(bevy_ticks, TICKS, "the Bevy clock lost or gained ticks");
    assert_eq!(
        terminal_ticks, TICKS,
        "the terminal clock lost or gained ticks"
    );
    assert_eq!(
        bevy_hash,
        direct.hash(),
        "irregular Bevy frame pacing changed the canonical trace"
    );
    assert_eq!(
        terminal_hash,
        direct.hash(),
        "terminal frame pacing changed the canonical trace"
    );
}

#[test]
fn the_terminal_session_lands_on_the_same_state_as_direct_stepping() {
    let scenario = walking();
    let terminal_frames: Vec<f64> = std::iter::repeat_n(0.01, 100).collect();

    let mut session = TuiSession::new(
        Arc::clone(&scenario),
        SEED,
        Vec::new(),
        ColorDepth::Truecolor,
    )
    .expect("session starts");
    session.resize(80, 24);
    for &frame_secs in &terminal_frames {
        session.advance(frame_secs);
    }
    assert_eq!(session.tick(), TICKS);

    let mut direct = Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("builds");
    for _ in 0..TICKS {
        direct.step();
    }
    assert_eq!(
        format!("{:?}", session.simulation().snapshot(SnapshotDetail::Full)),
        format!("{:?}", direct.snapshot(SnapshotDetail::Full)),
        "the terminal session diverged from direct stepping"
    );
}
