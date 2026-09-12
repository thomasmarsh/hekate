//! Golden byte fixtures for the Kitty graphics protocol output.
//!
//! These pin the bytes the backend would write to a real terminal, without one:
//! command framing, chunked transmission, a full transmit-plus-place frame, and
//! image deletion. Regenerate with `scripts/regen-goldens.sh` and inspect the
//! diff before committing it.

use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec2;
use tangle_model::CompiledScenario;
use tangle_present::{
    PresentationController, RendererBackend, SceneFrame, SceneGeometry, Viewport, load_scenario,
};
use tangle_sim::{RunConfig, Simulation, SnapshotDetail};
use tangle_tui::{
    KittyBackend, Multiplexer, RunInfo, apc, delete_image, encode, screen_wrap, tmux_wrap,
};

const SEED: u64 = 0;
const TICKS: u64 = 20;
const STEP_SECS: f64 = 0.05;
/// Terminal size chosen so the transmitted frame is small but chunked.
const COLUMNS: u32 = 8;
const ROWS: u32 = 5;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn walking() -> Arc<CompiledScenario> {
    let path = repo_path("scenarios/walking/walking_guide_v1.json5");
    Arc::new(load_scenario(&path).expect("walking scenario loads"))
}

/// Compare `actual` with a checked-in byte golden, writing it instead when
/// `UPDATE_GOLDENS` is set.
fn check_bytes(relative: &str, actual: &[u8]) {
    let path = repo_path(relative);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden has a parent")).expect("create dir");
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read golden '{}': {error}\nrun scripts/regen-goldens.sh",
            path.display()
        )
    });
    assert_eq!(
        actual,
        expected,
        "golden '{}' changed; run scripts/regen-goldens.sh",
        path.display()
    );
}

/// A frame at a fixed tick fitted into the tiny test scene area.
fn fixed_frame(scenario: &CompiledScenario) -> SceneFrame {
    let mut sim = Simulation::new(scenario.clone(), RunConfig::new(SEED)).expect("builds");
    let previous = sim.snapshot(SnapshotDetail::Full);
    for _ in 0..TICKS {
        sim.step();
    }
    let current = sim.snapshot(SnapshotDetail::Full);

    let geometry = SceneGeometry::from_scenario(scenario);
    let bounds = geometry
        .bounds()
        .unwrap_or((DVec2::splat(-20.0), DVec2::splat(20.0)));
    let screen = (f64::from(COLUMNS), f64::from(ROWS - 3) * 2.0);
    let controller =
        PresentationController::new(geometry, STEP_SECS, Viewport::fit(bounds, screen, 1.25));
    controller.project(&previous, &current)
}

fn small_backend(scenario: Arc<CompiledScenario>) -> KittyBackend<Vec<u8>> {
    let mut backend = KittyBackend::with_mux(Vec::new(), scenario, Multiplexer::None);
    backend
        .resize_terminal(COLUMNS, ROWS)
        .expect("resize succeeds");
    backend.set_run_info(RunInfo {
        seed: SEED,
        spawned: 6,
        despawned: 0,
    });
    backend
}

#[test]
fn command_framing_matches_golden() {
    let mut bytes = Vec::new();
    // A payload-free command omits the protocol separator.
    bytes.extend_from_slice(&apc("a=d,d=A", b""));
    // A payload-carrying command keeps it.
    bytes.extend_from_slice(&apc("a=t,f=32,s=1,v=1,i=7,m=0", b"foo"));
    // tmux and GNU screen passthrough wrapping.
    bytes.extend_from_slice(&tmux_wrap(&apc("i=7", b"")));
    bytes.extend_from_slice(&screen_wrap(&apc("i=7", b"")));
    check_bytes("tests/golden/renderer/kitty_framing.bin", &bytes);
}

#[test]
fn chunked_transmission_matches_golden() {
    let data: Vec<u8> = (0..12 * 1024).map(|index| (index % 251) as u8).collect();
    let frame = encode("a=t,f=32,s=64,v=48,i=9", &data);

    assert!(frame.sequences.len() > 1, "the fixture must chunk");
    assert_eq!(frame.data_bytes, data.len());
    assert_eq!(frame.sequences.len(), 4);
    assert!(
        frame.sequences[0].starts_with(b"\x1b_Ga=t,f=32,s=64,v=48,i=9,m=1;"),
        "only the first chunk carries the control data"
    );
    assert!(frame.sequences.last().unwrap().starts_with(b"\x1b_Gm=0;"));

    let bytes: Vec<u8> = frame.sequences.concat();
    check_bytes("tests/golden/renderer/kitty_chunking.bin", &bytes);
}

#[test]
fn a_transmitted_placement_matches_golden() {
    let scenario = walking();
    let mut backend = small_backend(Arc::clone(&scenario));
    let frame = fixed_frame(&scenario);

    backend.draw(&frame).expect("draw succeeds");
    backend.present().expect("present succeeds");

    assert_eq!(backend.frames_transmitted(), 1);
    check_bytes(
        "tests/golden/renderer/kitty_present.bin",
        &backend.into_writer(),
    );
}

#[test]
fn image_deletion_matches_golden() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&delete_image(1));
    bytes.extend_from_slice(&delete_image(31));
    check_bytes("tests/golden/renderer/kitty_deletion.bin", &bytes);
}
