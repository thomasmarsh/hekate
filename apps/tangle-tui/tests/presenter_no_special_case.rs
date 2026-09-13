//! Guard: a presenter renders a body from scene data, never from a scenario or
//! mode name.
//!
//! `PHASE_2_PLAN.md` Increment 0 requires the viewer and terminal presenters to
//! render every body kind and its ordered segments "so later modes add no
//! mode-specific presenter branch". The shared `tangle-present` shape decision
//! (`SceneBody::shapes`) is the only place that maps a body to its drawn shape;
//! a presenter that names a scenario or a mode to choose a shape has
//! reintroduced the branch this gate forbids.
//!
//! The check is a source-text assertion, the same drift-guard pattern
//! `crates/tangle-sim/tests/synthetic_template_no_branch.rs` uses: it observes
//! the one thing no behavioral test can — that no presenter module names a
//! scenario or mode. `#[cfg(test)]` modules are excluded, so a fixture that
//! needs a named scenario does not trip the guard.

/// Phase 2 mode names no presenter may branch on.
const PHASE_2_MODE_NAMES: [&str; 7] = [
    "bicycle",
    "scooter",
    "bus",
    "truck",
    "tractor",
    "articulated",
    "motorcycle",
];

/// The Phase 1 mode type a presenter must not dispatch on: the shape decision
/// reads the body kind, not the mode.
const PHASE_1_MODE_TYPE: &str = "AgentMode";

/// Reading a body's mode is the branch this guard forbids; the shape decision
/// reads `body_kind` instead.
const BODY_MODE_ACCESS: &str = ".mode";

/// A checked-in Phase 1 scenario id, so the guard cannot silently drift from a
/// real scenario name.
const SCENARIO_ID: &str = "walking_guide_v1";
const WORKING_SCENARIO: &str = include_str!("../../../scenarios/walking/walking_guide_v1.json5");

/// A presenter that chooses a shape from the scenario id would compare it. The
/// viewer host legitimately loads a default scenario file, so a raw scenario
/// name is checked only in the shape modules; every module is checked for a
/// comparison against the frame's scenario id.
const SCENARIO_DISPATCH: [&str; 3] = [
    "scenario_id ==",
    "scenario_id !=",
    "match frame.scenario_id",
];

/// The modules that turn a scene body into cells, pixels, or Bevy meshes. These
/// are the only places a shape, mode, or scenario branch could reappear.
const SHAPE_MODULES: [(&str, &str); 3] = [
    (
        "apps/tangle-tui/src/raster.rs",
        include_str!("../src/raster.rs"),
    ),
    (
        "apps/tangle-tui/src/pixel.rs",
        include_str!("../src/pixel.rs"),
    ),
    (
        "apps/tangle-viewer/src/lib.rs",
        include_str!("../../../apps/tangle-viewer/src/lib.rs"),
    ),
];

/// The viewer's ECS systems host. It may load a default scenario, but no body
/// system may branch on a mode.
const VIEWER_MAIN: &str = include_str!("../../../apps/tangle-viewer/src/main.rs");

/// The production part of a module, excluding its `#[cfg(test)]` module.
fn production(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

#[test]
fn the_scenario_id_this_guard_names_is_a_real_scenario() {
    assert!(
        WORKING_SCENARIO.contains(SCENARIO_ID),
        "the guard must name the scenario id '{SCENARIO_ID}' the fixture declares"
    );
}

#[test]
fn no_shape_module_names_a_scenario_or_mode() {
    for (name, source) in SHAPE_MODULES {
        let source = production(source);
        assert!(
            !source.contains(SCENARIO_ID),
            "{name} names scenario '{SCENARIO_ID}'; a shape must come from scene data, not a \
             scenario branch"
        );
        assert!(
            !source.contains(PHASE_1_MODE_TYPE),
            "{name} names the mode type '{PHASE_1_MODE_TYPE}'; a shape must come from the body \
             kind, not a mode branch"
        );
        assert!(
            !source.contains(BODY_MODE_ACCESS),
            "{name} reads a body's mode; the shared shape decision reads the body kind instead"
        );
        for mode in PHASE_2_MODE_NAMES {
            assert!(
                !source.contains(mode),
                "{name} names the mode '{mode}'; a shape must come from scene data, not a mode \
                 branch"
            );
        }
        assert_no_scenario_dispatch(name, source);
    }
}

#[test]
fn the_viewer_body_systems_name_no_mode_or_scenario() {
    let source = production(VIEWER_MAIN);
    assert!(
        !source.contains(PHASE_1_MODE_TYPE),
        "the viewer names the mode type '{PHASE_1_MODE_TYPE}'; a body system must consume \
         `body_visuals` rather than branch on a mode"
    );
    for mode in PHASE_2_MODE_NAMES {
        assert!(
            !source.contains(mode),
            "the viewer names the mode '{mode}'; a body system must not branch on a mode"
        );
    }
    assert_no_scenario_dispatch("apps/tangle-viewer/src/main.rs", source);
}

/// Fail if a presenter chooses its output by comparing the scenario id.
fn assert_no_scenario_dispatch(name: &str, source: &str) {
    for pattern in SCENARIO_DISPATCH {
        assert!(
            !source.contains(pattern),
            "{name} dispatches on the scenario ('{pattern}'); a shape must come from scene data, \
             not a scenario branch"
        );
    }
}
