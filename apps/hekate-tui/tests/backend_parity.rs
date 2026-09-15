//! Cross-backend parity: choosing the Kitty graphics backend must not change
//! which kernel ticks a session visits.
//!
//! The session owns the playback clock and the shared controller, so the
//! backend is a pure sink. This test drives the character-cell backend and the
//! Kitty graphics backend through the same seed with the same irregular wall-
//! clock partitions and asserts they reach identical kernel state.

use std::path::PathBuf;
use std::sync::Arc;

use hekate_model::CompiledScenario;
use hekate_present::load_scenario;
use hekate_sim::{RunConfig, Simulation, SnapshotDetail};
use hekate_tui::{ColorDepth, KittyBackend, Multiplexer, TuiSession};

const SEED: u64 = 0;
/// Irregular frame partitions over one second of wall time.
const FRAMES: [f64; 7] = [0.17, 0.03, 0.22, 0.05, 0.31, 0.001, 0.219];

fn walking() -> Arc<CompiledScenario> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/walking/walking_guide_v1.json5");
    Arc::new(load_scenario(&path).expect("walking scenario loads"))
}

#[test]
fn the_cell_and_kitty_backends_visit_the_same_ticks() {
    let scenario = walking();
    let mut cells = TuiSession::new(
        Arc::clone(&scenario),
        SEED,
        Vec::new(),
        ColorDepth::Truecolor,
    )
    .expect("cell session starts");
    cells.resize(40, 12);

    let kitty_backend =
        KittyBackend::with_mux(Vec::new(), Arc::clone(&scenario), Multiplexer::None);
    let mut kitty = TuiSession::with_backend(Arc::clone(&scenario), SEED, kitty_backend)
        .expect("kitty session starts");
    kitty.resize(40, 12);

    for frame in FRAMES {
        cells.advance_and_render(frame).expect("cell frame renders");
        kitty
            .advance_and_render(frame)
            .expect("kitty frame renders");
    }

    assert_eq!(cells.tick(), 20);
    assert_eq!(kitty.tick(), 20);

    let mut direct =
        Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("direct run");
    for _ in 0..20 {
        direct.step();
    }
    let direct = format!("{:?}", direct.snapshot(SnapshotDetail::Full));
    assert_eq!(
        format!("{:?}", cells.simulation().snapshot(SnapshotDetail::Full)),
        direct,
        "the cell backend diverged from direct stepping"
    );
    assert_eq!(
        format!("{:?}", kitty.simulation().snapshot(SnapshotDetail::Full)),
        direct,
        "the kitty backend diverged from direct stepping"
    );
}
