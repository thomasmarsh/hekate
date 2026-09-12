//! Golden snapshots for the shared scene projection and `ViewCommand`
//! handling.
//!
//! These pin the backend-agnostic frame every renderer consumes: the geometry,
//! body interpolation, status, viewport, and overlay state produced from a
//! fixed simulation snapshot. A failure means the shared layer changed what a
//! backend draws, not that a single backend regressed.
//!
//! Regenerate the goldens with `scripts/regen-goldens.sh` and inspect the diff
//! before committing it.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec2;
use tangle_model::CompiledScenario;
use tangle_present::{
    Overlay, PresentationController, RestartMode, SceneFrame, SceneGeometry, Speed, ViewCommand,
    Viewport, load_scenario,
};
use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

const SEED: u64 = 0;
const TICKS: u64 = 20;
const STEP_SECS: f64 = 0.05;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn walking() -> Arc<CompiledScenario> {
    let path = repo_path("scenarios/walking/walking_guide_v1.json5");
    Arc::new(load_scenario(&path).expect("walking scenario loads"))
}

/// Compare `actual` with a checked-in golden, writing it instead when
/// `UPDATE_GOLDENS` is set.
fn check_text(relative: &str, actual: &str) {
    let path = repo_path(relative);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden has a parent")).expect("create dir");
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|error| {
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

fn controller(scenario: &CompiledScenario) -> PresentationController {
    presentation(scenario, Viewport::new(DVec2::ZERO, 1.0))
}

fn presentation(scenario: &CompiledScenario, viewport: Viewport) -> PresentationController {
    let geometry = SceneGeometry::from_scenario(scenario);
    PresentationController::new(geometry, STEP_SECS, viewport)
}

/// Format the observable state of one projected frame.
fn describe(frame: &SceneFrame) -> String {
    format!(
        "  paused={} speed={} selection={:?} center={:?} scale={:.6} geometry={} vectors={}",
        frame.status.paused,
        frame.status.speed.label(),
        frame.status.selection,
        frame.viewport.center(),
        frame.viewport.scale(),
        frame.overlays.geometry,
        frame.overlays.vectors,
    )
}

#[test]
fn scene_projection_matches_golden() {
    let scenario = walking();
    let mut sim = Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("builds");
    for _ in 0..(TICKS - 1) {
        sim.step();
    }
    let previous = sim.snapshot(SnapshotDetail::Full);
    sim.step();
    let current = sim.snapshot(SnapshotDetail::Full);

    let mut controller = controller(&scenario);
    // Half a step of accumulated time interpolates bodies halfway.
    controller.advance(STEP_SECS / 2.0);
    let frame = controller.project(&previous, &current);

    check_text(
        "tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt",
        &format!("{frame:#?}\n"),
    );
}

#[test]
fn view_command_handling_matches_golden() {
    let scenario = walking();
    let sim = Simulation::new((*scenario).clone(), RunConfig::new(SEED)).expect("builds");
    let snapshot = sim.snapshot(SnapshotDetail::Full);
    let target = snapshot.agents()[0].position;

    let mut controller = controller(&scenario);
    let commands = [
        ViewCommand::TogglePause,
        ViewCommand::SingleStep,
        ViewCommand::SetSpeed(Speed::Fast),
        ViewCommand::TogglePause,
        ViewCommand::Pan(DVec2::new(3.0, -2.0)),
        ViewCommand::Zoom(0.5),
        ViewCommand::SelectNearest(target),
        ViewCommand::ToggleOverlay(Overlay::Vectors),
        ViewCommand::ToggleOverlay(Overlay::Geometry),
        ViewCommand::ClearSelection,
        ViewCommand::Restart(RestartMode::NextSeed),
    ];

    let mut log = String::new();
    for command in commands {
        let before = controller.project(&snapshot, &snapshot);
        let applied = controller.apply(&command, &before);
        let after = controller.project(&snapshot, &snapshot);
        writeln!(log, "command {command:?} => {applied:?}").expect("write to String");
        writeln!(log, "{}", describe(&after)).expect("write to String");
    }

    check_text("tests/golden/present/view_commands.txt", &log);
}
