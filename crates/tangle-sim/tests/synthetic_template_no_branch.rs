//! The Increment 0 extension gate: no shared module branches on the synthetic
//! mode.
//!
//! `PHASE_2_PLAN.md` "Agent composition" allows runtime code to dispatch to a
//! small number of motion and controller families but forbids duplicating
//! interaction, event, or metric logic per named mode. The synthetic template
//! fixture (TAS-070) alters body dimensions, kinematic limits, and facility
//! access, so if those differences need a branch on its mode id, shared code
//! will name the id and this test fails. That is the fixture's purpose: the
//! components carry the differences as data, and mode-specific dispatch breaks
//! the gate.
//!
//! The check is deliberately a source-text assertion, the same drift-guard
//! pattern `model_cards.rs` uses for the model cards: it observes the one thing
//! no behavioral test can — that the mode name never appears in the shared
//! modules the template is supposed to flow through as data.

/// The synthetic mode id the fixture authors and shared code must not name.
const SYNTHETIC_MODE_ID: &str = "synthetic_hauler";

/// The checked-in synthetic fixture, so this guard cannot silently drift from
/// the id the fixture actually declares.
const FIXTURE: &str =
    include_str!("../../tangle-model/tests/fixtures/synthetic_mode_template_v2.json5");

/// The shared interaction, event, metric, stage, and controller modules the
/// synthetic mode must flow through as data.
///
/// `control.rs` and `pedestrian.rs` are the two model cards; `controller.rs` is
/// the replaceable-model seam; `stage.rs` and `sim.rs` are the four stages and
/// their kernel implementation; `event.rs`, `metrics.rs`, and `safety.rs` are
/// the event, metric, and safety passes. None of them may name the mode.
const SHARED_MODULES: [(&str, &str); 8] = [
    ("sim.rs (interaction kernel)", include_str!("../src/sim.rs")),
    (
        "stage.rs (controller stages)",
        include_str!("../src/stage.rs"),
    ),
    (
        "controller.rs (model seam)",
        include_str!("../src/controller.rs"),
    ),
    (
        "control.rs (vehicle model card)",
        include_str!("../src/control.rs"),
    ),
    (
        "pedestrian.rs (pedestrian model card)",
        include_str!("../src/pedestrian.rs"),
    ),
    ("event.rs (typed events)", include_str!("../src/event.rs")),
    (
        "metrics.rs (online metrics)",
        include_str!("../src/metrics.rs"),
    ),
    ("safety.rs (safety pass)", include_str!("../src/safety.rs")),
];

#[test]
fn the_fixture_declares_the_mode_id_this_guard_searches_for() {
    assert!(
        FIXTURE.contains(SYNTHETIC_MODE_ID),
        "the synthetic fixture must declare mode id '{SYNTHETIC_MODE_ID}'"
    );
}

#[test]
fn no_shared_module_names_the_synthetic_mode() {
    for (name, source) in SHARED_MODULES {
        assert!(
            !source.contains(SYNTHETIC_MODE_ID),
            "shared module {name} names the synthetic mode '{SYNTHETIC_MODE_ID}'; the template \
             must reach shared behavior through its components, not a mode branch"
        );
    }
}
