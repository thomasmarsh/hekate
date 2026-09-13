//! Increment 1 gate: no shared kernel module branches on a narrow template id.
//!
//! `PHASE_2_PLAN.md` "Agent composition" allows runtime code to dispatch to a
//! small number of motion and controller families but forbids duplicating
//! interaction, event, or metric logic per named mode. The narrow-mode fixture
//! (TAS-076) authors `bicycle` and `scooter` templates whose body, limits,
//! steering response, clearance, and access differ, so if those differences
//! need a branch on a mode id, shared code will name the id and this test
//! fails. That is the fixture's purpose: the components carry the differences
//! as data, the spawn path dispatches on the compiled [`AgentFamily`], and the
//! mode name never appears in the shared modules the template flows through.
//!
//! The check is deliberately a source-text assertion, the same drift-guard
//! pattern `synthetic_template_no_branch.rs` (TAS-070) and
//! `tangle-model`'s `narrow_mode_templates.rs` (TAS-076) use: it observes the
//! one thing no behavioral test can — that the mode name never appears in the
//! shared modules the template is supposed to flow through as data (and never
//! in the narrow model either). The narrow module's model cards name the modes
//! in prose, so the guard searches for a *Rust string literal*, not a
//! substring; the falsification probe pins that distinction.
//!
//! [`AgentFamily`]: tangle_model::AgentFamily

/// The narrow template ids the fixture authors and shared code must not name.
const NARROW_MODE_IDS: [&str; 2] = ["bicycle", "scooter"];

/// The checked-in narrow fixture, so this guard cannot silently drift from the
/// ids the fixture actually declares.
const FIXTURE: &str =
    include_str!("../../tangle-model/tests/fixtures/narrow_mode_templates_v2.json5");

/// The shared interaction, event, metric, stage, and controller modules the
/// narrow modes must flow through as data, plus the narrow model module.
///
/// `sim.rs` is the interaction kernel that spawns narrow agents and dispatches
/// their motion; `stage.rs` is the four controller stages; `controller.rs` is
/// the replaceable-model seam; `control.rs` is the shared vehicle model card;
/// `event.rs`, `metrics.rs`, and `safety.rs` are the event, metric, and safety
/// passes; `narrow.rs` is the narrow model itself, which must also not branch
/// on the id. None of them may name the mode as a Rust string literal.
const GUARDED_MODULES: [(&str, &str); 8] = [
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
    ("narrow.rs (narrow model)", include_str!("../src/narrow.rs")),
    ("event.rs (typed events)", include_str!("../src/event.rs")),
    (
        "metrics.rs (online metrics)",
        include_str!("../src/metrics.rs"),
    ),
    ("safety.rs (safety pass)", include_str!("../src/safety.rs")),
];

/// The production code of one module: everything before its `#[cfg(test)]`
/// module. Test fixtures legitimately name authored ids, so a branch guard
/// observes only the code that ships.
fn production_code(module: &str) -> &str {
    module.split("#[cfg(test)]").next().unwrap_or(module)
}

/// Whether a module's production code names a narrow template id as a Rust
/// string literal. A branch on the id is `id == "bicycle"` or `"scooter" =>`;
/// prose (`the bicycle card`) and test fixtures carry no literal.
fn production_names_a_narrow_id(module: &str) -> Option<&'static str> {
    let production = production_code(module);
    NARROW_MODE_IDS.into_iter().find(|id| {
        production.contains(&format!("\"{id}\"")) || production.contains(&format!("'{id}'"))
    })
}

#[test]
fn the_fixture_declares_the_narrow_ids_this_guard_searches_for() {
    for id in NARROW_MODE_IDS {
        assert!(
            FIXTURE.contains(&format!("id: '{id}'")),
            "the narrow fixture must declare mode id '{id}' the guard searches for"
        );
    }
}

#[test]
fn no_shared_module_names_a_narrow_mode_id() {
    for (name, source) in GUARDED_MODULES {
        if let Some(id) = production_names_a_narrow_id(source) {
            panic!(
                "module {name} names narrow template id '{id}' as a string literal; the narrow \
                 modes must reach shared behavior through their components and family, not a mode \
                 branch"
            );
        }
    }
}

#[test]
fn the_guard_detects_a_branch_and_ignores_prose_and_tests() {
    // The guard is not vacuous: it flags the branch a regression would add...
    assert_eq!(
        production_names_a_narrow_id("fn f(id: &str) { if id == \"bicycle\" { } }"),
        Some("bicycle")
    );
    assert_eq!(
        production_names_a_narrow_id("match id { \"scooter\" => 1, _ => 0 }"),
        Some("scooter")
    );
    // ...it ignores a doc-comment prose mention (the cards name the modes)...
    assert_eq!(
        production_names_a_narrow_id("/// the bicycle and scooter cards live here."),
        None
    );
    // ...and it ignores test code, where fixtures legitimately name the ids.
    assert_eq!(
        production_names_a_narrow_id(
            "fn spawn() {}\n#[cfg(test)]\nmod tests { const ID: &str = \"bicycle\"; }"
        ),
        None
    );
}
