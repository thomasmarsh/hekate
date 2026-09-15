//! End-to-end checks for the terminal session on the checked-in walking
//! scenario: the same seed reaches the same kernel ticks as direct stepping,
//! and a frame is captured through the backend's `Write` sink rather than a
//! real terminal.

use std::path::PathBuf;
use std::sync::Arc;

use hekate_model::CompiledScenario;
use hekate_present::load_scenario;
use hekate_sim::{RunConfig, Simulation, SnapshotDetail};
use hekate_tui::{ColorDepth, TuiSession};

const SEED: u64 = 0;

fn walking() -> Arc<CompiledScenario> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/walking/walking_guide_v1.json5");
    Arc::new(load_scenario(&path).expect("walking scenario loads"))
}

#[test]
fn the_same_seed_reaches_the_same_ticks_as_direct_stepping() {
    let scenario = walking();
    let mut session = TuiSession::new(
        Arc::clone(&scenario),
        SEED,
        Vec::new(),
        ColorDepth::Truecolor,
    )
    .expect("session starts");

    // One second of wall time partitioned irregularly still visits 20 steps.
    for frame in [0.17, 0.03, 0.22, 0.05, 0.31, 0.001, 0.219] {
        session.advance(frame);
    }
    assert_eq!(session.tick(), 20);

    let mut direct =
        Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("direct run");
    for _ in 0..20 {
        direct.step();
    }
    assert_eq!(
        format!("{:?}", session.simulation().snapshot(SnapshotDetail::Full)),
        format!("{:?}", direct.snapshot(SnapshotDetail::Full)),
        "the terminal session diverged from direct kernel stepping"
    );
}

#[test]
fn a_frame_is_captured_through_the_write_sink() {
    let mut session =
        TuiSession::new(walking(), 7, Vec::new(), ColorDepth::Truecolor).expect("session starts");
    session.resize(100, 30);
    session.advance_and_render(0.05).expect("frame renders");

    let text = String::from_utf8(session.into_writer()).expect("output is UTF-8");
    assert!(text.contains("walking_guide_v1"));
    assert!(text.contains("seed 7"));
    assert!(text.contains("tick 1"));
    assert!(text.contains("\x1b[38;2;"));
    // The guide path is drawn into the grid.
    assert!(text.contains('*'));
}
