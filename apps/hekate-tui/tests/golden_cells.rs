//! Golden character grids for the terminal character backend.
//!
//! The grid is the backend's whole render: glyphs plus foreground and
//! background colors for a fixed scenario, seed, viewport, and tick. It is
//! captured through the same `TuiSession` and `CellBackend` a real run uses,
//! then serialized with the same ANSI writer the terminal sees.
//!
//! Regenerate with `scripts/regen-goldens.sh` and inspect the diff before
//! committing it.

use std::path::PathBuf;
use std::sync::Arc;

use hekate_model::CompiledScenario;
use hekate_present::load_scenario;
use hekate_tui::{ColorDepth, TuiSession};

const SEED: u64 = 0;
const COLUMNS: u32 = 80;
const ROWS: u32 = 24;
/// One second of wall time at the 0.05 s standard step.
const ACTIVE_SECONDS: f64 = 1.0;
const EXPECTED_TICKS: u64 = 20;

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

#[test]
fn character_grid_matches_golden() {
    let scenario = walking();
    let mut session =
        TuiSession::new(scenario, SEED, Vec::new(), ColorDepth::Truecolor).expect("session starts");
    session.resize(COLUMNS, ROWS);

    let ticks = session.advance(ACTIVE_SECONDS);
    assert_eq!(ticks, EXPECTED_TICKS);

    session.draw().expect("draw succeeds");
    let grid = session.backend().grid().expect("draw produced a grid");
    assert_eq!((grid.width(), grid.height()), (COLUMNS, ROWS - 3));

    check_bytes(
        "tests/golden/renderer/walking_guide_v1.seed0.tick20.80x24.cells.ansi",
        grid.to_ansi(ColorDepth::Truecolor).as_bytes(),
    );
}
