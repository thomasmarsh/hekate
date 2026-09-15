//! The terminal presenter opens the checked-in Increment 2 fixtures.
//!
//! `hekate-tui` renders only the shared `SceneFrame`, so a fixture that starts a
//! session, advances its clock, and draws here is one the terminal viewer opens:
//! the version-2 load path, the scenario's compiled facilities, the narrow and
//! motor bodies its demand spawns, and the route-relative overlays the frame
//! derives all reach the rasterizer. The passing fixture's inspector footer is
//! asserted to be exactly the shared summaries for the maneuvering body, which
//! is the terminal half of the backend parity
//! `crates/hekate-present/tests/inc2_fixture_overlays.rs` pins in its golden.
//!
//! The occupied opposing corridor's wrong-way interval needs an entry request a
//! checked-in scenario authors none of; `TuiSession::request_wrong_way_entry` is
//! the host seam a normal run uses to record one, and its own unit tests drive
//! that seam against this fixture. This file pins that the fixture itself opens
//! and draws in the terminal backend.

use std::path::PathBuf;
use std::sync::Arc;

use glam::DVec2;
use hekate_model::CompiledScenario;
use hekate_present::{
    MAX_TICKS_PER_FRAME, PresentationController, SceneFrame, SceneGeometry, ViewCommand, Viewport,
    corridor_summary, load_scenario, maneuver_summary, predicted_gap_summary,
    target_offset_summary, wrong_way_summary,
};
use hekate_sim::{RunConfig, Simulation, SnapshotDetail};
use hekate_tui::{BACKGROUND, ColorDepth, TuiSession};

const SEED: u64 = 0;
/// The pinned Standard step both fixtures run at.
const STEP_SECS: f64 = 0.05;
/// A tick inside the passing fixture's one overtaking interval, which the
/// shared golden pins as opening at tick 712 and closing after tick 1097.
const PASS_TICK: u64 = 800;
const COLUMNS: u32 = 120;
const ROWS: u32 = 40;
/// One second of wall time at the 0.05 s standard step.
const ACTIVE_SECONDS: f64 = 1.0;

/// The passing fixture, relative to the repository root.
const PASSING_FIXTURE: &str = "scenarios/phase2/inc2/narrow_passing_v2.json5";
/// The contextual wrong-way fixture, relative to the repository root.
const WRONG_WAY_FIXTURE: &str = "scenarios/phase2/inc2/narrow_wrong_way_v2.json5";

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn fixture(relative: &str) -> Arc<CompiledScenario> {
    Arc::new(
        load_scenario(&repo_path(relative))
            .unwrap_or_else(|error| panic!("{relative} did not load: {error}")),
    )
}

/// The frame the shared controller projects for `scenario` after `ticks` steps,
/// driven exactly as the session drives it: one whole step per tick, with each
/// step's records folded into the overlays first.
fn projected(scenario: &Arc<CompiledScenario>, ticks: u64) -> SceneFrame {
    let mut sim =
        Simulation::new((**scenario).clone(), RunConfig::new(SEED)).expect("the fixture builds");
    let mut controller = PresentationController::new(
        SceneGeometry::from_scenario(scenario),
        STEP_SECS,
        Viewport::new(DVec2::ZERO, 1.0),
    );
    let mut previous = sim.snapshot(SnapshotDetail::Full);
    let mut frame = controller.project(&previous, &previous);
    for _ in 0..ticks {
        let step = sim.step();
        controller.observe_events(step.time().tick(), step.events());
        let current = sim.snapshot(SnapshotDetail::Full);
        frame = controller.project(&previous, &current);
        previous = current;
    }
    frame
}

/// Whether `grid` drew anything but the background.
fn drew_anything(grid: &hekate_tui::CellGrid) -> bool {
    (0..grid.height()).any(|row| {
        (0..grid.width()).any(|column| {
            grid.get(i64::from(column), i64::from(row))
                .is_some_and(|cell| cell.fg != BACKGROUND)
        })
    })
}

/// Advance a session to `tick` in whole frames, since the presentation clock
/// caps how many steps one frame may take.
fn advance_to(session: &mut TuiSession<hekate_tui::CellBackend<Vec<u8>>>, tick: u64) {
    while session.tick() < tick {
        let remaining = tick - session.tick();
        session.advance(remaining.min(MAX_TICKS_PER_FRAME) as f64 * STEP_SECS);
    }
}

/// The passing fixture draws in the terminal backend, and the inspector footer
/// for the maneuvering body is exactly the shared layer's summaries.
#[test]
fn the_passing_fixture_draws_and_its_inspector_is_the_shared_projection() {
    let scenario = fixture(PASSING_FIXTURE);
    assert_eq!(
        scenario.schema_version(),
        2,
        "the fixture is a version 2 run"
    );
    let mut session = TuiSession::new(
        Arc::clone(&scenario),
        SEED,
        Vec::new(),
        ColorDepth::Truecolor,
    )
    .expect("the passing fixture starts a session");
    session.resize(COLUMNS, ROWS);

    advance_to(&mut session, PASS_TICK);
    assert_eq!(
        session.tick(),
        PASS_TICK,
        "the session visits the shared ticks"
    );

    let expected = projected(&scenario, PASS_TICK);
    let maneuver = expected
        .maneuver_overlays()
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            panic!("the passing fixture must carry its overtaking interval at tick {PASS_TICK}")
        });
    let agent = maneuver.agent();

    // Select the maneuvering body by its own position, so the footer describes
    // the body whose overlays the shared frame carries.
    let position = session
        .simulation()
        .snapshot(SnapshotDetail::Full)
        .agents()
        .iter()
        .find(|sample| sample.id.index() == agent)
        .expect("the maneuvering body is alive")
        .position;
    session.apply(ViewCommand::SelectNearest(position));
    assert_eq!(session.selection(), Some(agent));
    session.draw().expect("the session draws");
    let footer = session.backend().footer_line().to_owned();
    assert!(
        footer.contains(&format!("Agent #{agent}")),
        "the footer names the selected body: {footer}"
    );

    let corridor = expected
        .corridors()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let target = expected
        .target_offsets()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let gap = expected
        .predicted_gaps()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    let rule = expected
        .wrong_way_overlays()
        .into_iter()
        .find(|overlay| overlay.agent() == agent);
    for summary in [
        corridor_summary(corridor.as_ref()),
        target_offset_summary(target.as_ref()),
        predicted_gap_summary(gap.as_ref()),
        maneuver_summary(Some(&maneuver)),
        wrong_way_summary(rule.as_ref()),
    ] {
        assert!(
            footer.contains(&summary),
            "the terminal footer must render the shared summary '{summary}': {footer}"
        );
    }

    // The overlays also reach the rasterizer: the grid is the scene area and is
    // not blank.
    let grid = session
        .backend()
        .grid()
        .expect("the session drew a grid for the last frame");
    assert_eq!((grid.width(), grid.height()), (COLUMNS, ROWS - 3));
    assert!(drew_anything(grid), "the passing fixture drew a blank grid");
}

/// The occupied opposing fixture opens through the same loader and draws in the
/// terminal backend, with the Increment 2 overlay toggles available.
#[test]
fn the_occupied_opposing_fixture_starts_a_session_and_draws() {
    let scenario = fixture(WRONG_WAY_FIXTURE);
    assert_eq!(
        scenario.schema_version(),
        2,
        "the fixture is a version 2 run"
    );
    let mut session =
        TuiSession::new(scenario, SEED, Vec::new(), ColorDepth::Truecolor).expect("session starts");
    session.resize(COLUMNS, ROWS);

    assert_eq!(session.advance(ACTIVE_SECONDS), 20);
    session.draw().expect("the session draws");

    let grid = session
        .backend()
        .grid()
        .expect("the session drew a grid for the last frame");
    assert_eq!((grid.width(), grid.height()), (COLUMNS, ROWS - 3));
    assert!(
        drew_anything(grid),
        "the occupied opposing fixture drew a blank grid"
    );

    // The fixture opens into the Increment 2 terminal surface: the legend offers
    // all five overlay toggles while nothing is selected.
    let legend = session.backend().footer_line();
    for key in [
        "c corridor",
        "t target",
        "p gap",
        "m maneuver",
        "o wrong-way",
    ] {
        assert!(
            legend.contains(key),
            "the terminal legend must offer '{key}': {legend}"
        );
    }
}
