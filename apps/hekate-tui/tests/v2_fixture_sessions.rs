//! The terminal presenter opens every checked-in Increment 1 narrow fixture.
//!
//! `hekate-tui` loads through `hekate_present::load_scenario`, so a fixture that
//! session starts and draws here is one the terminal viewer opens: the version-2
//! load path, the scenario's compiled facilities, and the narrow capsule bodies
//! its demand spawns all reach the rasterizer. Kitty drawing is the pixel twin of
//! the same scene projection, so this exercises the character-cell path the
//! `--backend ascii` run uses.

use std::path::PathBuf;
use std::sync::Arc;

use hekate_present::load_scenario;
use hekate_tui::{BACKGROUND, ColorDepth, TuiSession};

/// The six checked-in Increment 1 version-2 fixtures.
const V2_FIXTURES: [&str; 6] = [
    "narrow_isolated_straight_v2.json5",
    "narrow_isolated_curve_v2.json5",
    "narrow_isolated_braking_v2.json5",
    "narrow_following_v2.json5",
    "narrow_signal_v2.json5",
    "narrow_crossing_v2.json5",
];

const SEED: u64 = 20260913;
const COLUMNS: u32 = 80;
const ROWS: u32 = 24;
/// One second of wall time at the 0.05 s standard step.
const ACTIVE_SECONDS: f64 = 1.0;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn every_increment_1_fixture_starts_a_session_and_draws() {
    for name in V2_FIXTURES {
        let path = repo_path(&format!("scenarios/phase2/inc1/{name}"));
        let scenario = Arc::new(
            load_scenario(&path).unwrap_or_else(|error| panic!("{name} did not load: {error}")),
        );
        let mut session = TuiSession::new(scenario, SEED, Vec::new(), ColorDepth::Truecolor)
            .unwrap_or_else(|error| panic!("{name} did not start a session: {error}"));
        session.resize(COLUMNS, ROWS);

        assert_eq!(session.advance(ACTIVE_SECONDS), 20, "{name}");
        session
            .draw()
            .unwrap_or_else(|error| panic!("{name} did not draw: {error}"));
        let grid = session
            .backend()
            .grid()
            .unwrap_or_else(|| panic!("{name} drew no grid"));
        // The grid is the scene area: the backend reserves three rows for the
        // status and inspector lines.
        assert_eq!((grid.width(), grid.height()), (COLUMNS, ROWS - 3), "{name}");
        // One second of the fixture drew its geometry, so the session is not a
        // blank frame that only avoided an error.
        assert!(
            (0..grid.height()).any(|row| (0..grid.width()).any(|column| {
                grid.get(i64::from(column), i64::from(row))
                    .is_some_and(|cell| cell.fg != BACKGROUND)
            })),
            "{name} drew a blank grid"
        );
    }
}
