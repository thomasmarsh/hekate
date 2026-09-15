//! Guard: the shared presentation layer and both backends' shape modules render
//! an Increment 2 run from projected data, never from a scenario or a mode.
//!
//! `PHASE_2_PLAN.md` Increment 2 requires the usable corridor, target offset,
//! predicted gap, maneuver state, and wrong-way rule state to reach a renderer
//! as backend-neutral overlay primitives "mapped to identifiers only". The
//! mapping (`crates/hekate-present/src/tactical.rs`) reads the versioned route
//! state and the typed edge records; if a shared module ever needed to name the
//! scenario it was rendering or the mode that produced a tactic, the data flow
//! the contract fixes would be broken and shared code would carry the branch
//! this gate forbids.
//!
//! The check is a source-text assertion, the same drift-guard pattern
//! `crates/hekate-sim/tests/narrow_mode_no_branch.rs` and
//! `synthetic_template_no_branch.rs` (and `apps/hekate-tui/tests/
//! presenter_no_special_case.rs`, which it complements for the Increment 2
//! scenario set) use: it observes the one thing no behavioral test can — that no
//! shared presentation module names an Increment 2 scenario or mode. Each name
//! is searched as a *Rust string literal* in the code that ships, so a module's
//! prose may discuss a mode and a `#[cfg(test)]` fixture may author one; only a
//! comparison or match arm trips the guard. The falsification probe pins that
//! distinction.
//!
//! Guarded modules: every `.rs` file under `crates/hekate-present/src/` (read
//! from disk, so a new shared module is guarded without editing this file), and
//! the shape and inspector modules both applications render bodies and overlays
//! with.

use std::path::{Path, PathBuf};

/// Every checked-in Increment 2 fixture, as `(scenario id, source)`, so neither
/// the scenario names nor the mode names this guard searches for can drift from
/// the files that declare them.
const INCREMENT_2_FIXTURES: [(&str, &str); 8] = [
    (
        "narrow_passing_v2",
        include_str!("../../../scenarios/phase2/inc2/narrow_passing_v2.json5"),
    ),
    (
        "narrow_passing_unsafe_v2",
        include_str!("../../../scenarios/phase2/inc2/narrow_passing_unsafe_v2.json5"),
    ),
    (
        "motor_passing_narrow_v2",
        include_str!("../../../scenarios/phase2/inc2/motor_passing_narrow_v2.json5"),
    ),
    (
        "motor_lane_change_v2",
        include_str!("../../../scenarios/phase2/inc2/motor_lane_change_v2.json5"),
    ),
    (
        "motor_lane_change_boundary_v2",
        include_str!("../../../scenarios/phase2/inc2/motor_lane_change_boundary_v2.json5"),
    ),
    (
        "narrow_wrong_way_v2",
        include_str!("../../../scenarios/phase2/inc2/narrow_wrong_way_v2.json5"),
    ),
    (
        "mixed_mode_profile_v2",
        include_str!("../../../scenarios/phase2/inc2/mixed_mode_profile_v2.json5"),
    ),
    (
        "mixed_mode_profile_v2_no_lateral",
        include_str!("../../../scenarios/phase2/inc2/mixed_mode_profile_v2_no_lateral.json5"),
    ),
];

/// The checked-in Increment 1 narrow-mode fixture, which authors the same
/// narrow modes without the Increment 2 surface.
const INCREMENT_1_NARROW_FIXTURE: &str =
    include_str!("../../../scenarios/phase2/inc1/narrow_crossing_v2.json5");

/// The mode ids the Increment 2 fixtures author. No shared presentation module
/// may name one.
const INCREMENT_2_MODE_IDS: [&str; 3] = ["bicycle", "scooter", "passenger_car"];

/// The two application modules that turn a scene body into cells or pixels, and
/// the two that draw the route-relative overlays and describe them: the only
/// places a shape, overlay, or inspector branch on a scenario or a mode could
/// reappear.
const APP_MODULES: [(&str, &str); 5] = [
    (
        "apps/hekate-tui/src/raster.rs",
        include_str!("../../../apps/hekate-tui/src/raster.rs"),
    ),
    (
        "apps/hekate-tui/src/pixel.rs",
        include_str!("../../../apps/hekate-tui/src/pixel.rs"),
    ),
    (
        "apps/hekate-tui/src/hud.rs",
        include_str!("../../../apps/hekate-tui/src/hud.rs"),
    ),
    (
        "apps/hekate-viewer/src/lib.rs",
        include_str!("../../../apps/hekate-viewer/src/lib.rs"),
    ),
    (
        "apps/hekate-viewer/src/main.rs",
        include_str!("../../../apps/hekate-viewer/src/main.rs"),
    ),
];

/// The names no shared presentation or shape module may name: every Increment 2
/// scenario id, then every mode id those fixtures author.
fn forbidden_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = INCREMENT_2_FIXTURES.iter().map(|(id, _)| *id).collect();
    ids.extend(INCREMENT_2_MODE_IDS);
    ids
}

/// The production code of one module: everything before its `#[cfg(test)]`
/// module. Test fixtures legitimately author scenario documents and mode ids, so
/// a branch guard observes only the code that ships.
fn production_code(module: &str) -> &str {
    module.split("#[cfg(test)]").next().unwrap_or(module)
}

/// The forbidden name a module's production code names as a Rust string
/// literal, if it names one. A branch on a name is `mode == "bicycle"` or
/// `"narrow_passing_v2" =>`; prose (`the bicycle card`) and test fixtures carry
/// no shipping literal.
fn production_names_a_forbidden_id(module: &str) -> Option<&'static str> {
    let production = production_code(module);
    forbidden_ids().into_iter().find(|id| {
        production.contains(&format!("\"{id}\"")) || production.contains(&format!("'{id}'"))
    })
}

/// The path of `crates/hekate-present/src`'s module root.
fn presentation_source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every module under `crates/hekate-present/src`, as `(path, source)`, sorted
/// by path so a failure names a stable module. Reading the directory means a
/// module added to the shared layer is guarded without editing this file.
fn presentation_modules() -> Vec<(String, String)> {
    let directory = presentation_source_dir();
    let mut modules: Vec<(String, String)> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot read '{}': {error}", directory.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read '{}': {error}", path.display()));
            (format!("crates/hekate-present/src/{}", name(&path)), source)
        })
        .collect();
    modules.sort();
    modules
}

/// The file name of `path`.
fn name(path: &Path) -> String {
    path.file_name()
        .expect("a module path has a file name")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn the_fixtures_declare_the_ids_this_guard_searches_for() {
    for (id, source) in INCREMENT_2_FIXTURES {
        assert!(
            source.contains(&format!("id: '{id}'")),
            "the fixture this guard names must declare scenario id '{id}'"
        );
    }
    // Every mode id is declared by a fixture of this set, either as a mode
    // template or as a demand source's mode.
    for mode in INCREMENT_2_MODE_IDS {
        assert!(
            INCREMENT_2_FIXTURES
                .iter()
                .any(|(_, source)| source.contains(&format!("id: '{mode}'"))),
            "an Increment 2 fixture must declare mode id '{mode}' the guard searches for"
        );
    }
    // The same modes reach the Increment 1 fixtures, which carry no Increment 2
    // surface: the guard is not naming a mode only Increment 2 knows.
    for mode in ["bicycle", "scooter"] {
        assert!(
            INCREMENT_1_NARROW_FIXTURE.contains(&format!("id: '{mode}'")),
            "the Increment 1 narrow fixture must declare mode id '{mode}'"
        );
    }
}

#[test]
fn no_shared_module_names_an_increment_2_scenario_or_mode() {
    let mut guarded = presentation_modules();
    // The scan is not vacuous: it must reach the module that maps the tactical
    // state and the crate root that re-exports it.
    for expected in [
        "crates/hekate-present/src/lib.rs",
        "crates/hekate-present/src/tactical.rs",
    ] {
        assert!(
            guarded.iter().any(|(path, _)| path == expected),
            "the presentation scan missed '{expected}': {found:?}",
            found = guarded.iter().map(|(path, _)| path).collect::<Vec<_>>()
        );
    }
    guarded.extend(
        APP_MODULES
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned())),
    );
    for (path, source) in &guarded {
        if let Some(id) = production_names_a_forbidden_id(source) {
            panic!(
                "module {path} names '{id}' as a string literal; an Increment 2 scenario or mode \
                 must reach a renderer as projected data, not as a branch"
            );
        }
    }
}

#[test]
fn the_guard_detects_a_branch_and_ignores_prose_and_tests() {
    // The guard is not vacuous: it flags the branches a regression would add...
    assert_eq!(
        production_names_a_forbidden_id("fn f(id: &str) { if id == \"bicycle\" { } }"),
        Some("bicycle")
    );
    assert_eq!(
        production_names_a_forbidden_id("match mode { \"passenger_car\" => 1, _ => 0 }"),
        Some("passenger_car")
    );
    assert_eq!(
        production_names_a_forbidden_id(
            "fn f(frame: &SceneFrame) { if frame.scenario_id == \"narrow_wrong_way_v2\" { } }"
        ),
        Some("narrow_wrong_way_v2")
    );
    assert_eq!(
        production_names_a_forbidden_id("const ID: &str = 'scooter';"),
        Some("scooter")
    );
    // ...it ignores doc-comment prose, which names the modes and fixtures...
    assert_eq!(
        production_names_a_forbidden_id(
            "/// the bicycle and scooter corridors of narrow_passing_v2 live here."
        ),
        None
    );
    // ...and it ignores test code, where fixtures legitimately name them.
    assert_eq!(
        production_names_a_forbidden_id(
            "fn spawn() {}\n#[cfg(test)]\nmod tests { const ID: &str = \"bicycle\"; }"
        ),
        None
    );
}
