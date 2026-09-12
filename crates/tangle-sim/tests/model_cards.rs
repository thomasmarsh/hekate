//! Phase 1 Increment 3 slice E: the initial vehicle and pedestrian model cards.
//!
//! Deliverable 5 requires explicit, replaceable controller interfaces and a
//! model card for each initial model. The interfaces are types, and the
//! behavioral proof that the kernel reaches both modes only through them is the
//! stub-controller swap in `crates/tangle-sim/src/controller.rs` plus the
//! measured fixture in `mixed_interaction.rs`. The cards are documentation, so
//! this test checks the card inventory directly from the source text with
//! `include_str!`: a card that loses its state, parameter, constant,
//! decision-input, bounds, tie-break, or emergency-backstop section fails here,
//! and a card that stops naming its model family or its replaceable interface
//! fails here too. That is drift protection for a documentation deliverable, and
//! the one part of the deliverable no other test can observe.

/// The vehicle model card: the documented IDM longitudinal controller.
const VEHICLE_CARD: &str = include_str!("../src/control.rs");
/// The pedestrian model card: the documented waypoint controller.
const PEDESTRIAN_CARD: &str = include_str!("../src/pedestrian.rs");
/// The controller seam and model-card index.
const SEAM: &str = include_str!("../src/controller.rs");
/// The checked-in mixed gate fixture, run through both documented models.
const BENCHMARK: &str = include_str!("../../../scenarios/benchmarks/mixed_interaction_v1.json5");

/// Sections every model card must state, in this order.
///
/// The seam's model-card index documents the same inventory, and the cards
/// themselves are the account of the model: state variables, sampled parameters,
/// model constants, decision inputs, bounds, tie-breaks, and the emergency
/// backstop outside the bounds.
const CARD_SECTIONS: [&str; 7] = [
    "## State",
    "## Parameters",
    "## Constants",
    "## Decision inputs",
    "## Bounds",
    "## Tie-breaks",
    "## Emergency backstop",
];

/// The sections a card must carry, in order, as `(card name, text)`.
fn cards() -> [(&'static str, &'static str); 2] {
    [
        ("vehicle (control.rs)", VEHICLE_CARD),
        ("pedestrian (pedestrian.rs)", PEDESTRIAN_CARD),
    ]
}

#[test]
fn both_initial_model_cards_state_the_full_model_inventory() {
    for (name, card) in cards() {
        assert!(
            card.contains("# Model card"),
            "the {name} card must open with a `# Model card` heading"
        );
        let mut cursor = 0;
        for section in CARD_SECTIONS {
            let Some(found) = card[cursor..].find(section) else {
                panic!("the {name} card is missing the `{section}` section after its bounds");
            };
            cursor += found + section.len();
        }
    }
}

#[test]
fn each_card_names_its_model_family_and_its_replaceable_interface() {
    for (family, card) in [
        ("Intelligent Driver Model", VEHICLE_CARD),
        ("Treiber", VEHICLE_CARD),
    ] {
        assert!(
            card.contains(family),
            "the vehicle card must cite the `{family}` model family"
        );
    }
    for family in ["Coulter", "Helbing"] {
        assert!(
            PEDESTRIAN_CARD.contains(family),
            "the pedestrian card must cite the `{family}` work it approximates"
        );
    }
    // Each card must point at the replaceable interface the kernel calls.
    assert!(
        VEHICLE_CARD.contains("VehicleController"),
        "the vehicle card must name its replaceable interface"
    );
    assert!(
        PEDESTRIAN_CARD.contains("crate::controller"),
        "the pedestrian card must name its replaceable interface"
    );
    // Each card must name the counted seam for the exception to its own bounds.
    assert!(VEHICLE_CARD.contains("emergency_cap_steps"));
    assert!(PEDESTRIAN_CARD.contains("pedestrian_cap_steps"));
}

#[test]
fn the_seam_indexes_both_cards_and_both_interfaces() {
    for required in [
        "VehicleController",
        "PedestrianController",
        "ControllerModels",
        "crate::control",
        "crate::pedestrian",
        "Intelligent Driver Model",
        "Coulter",
    ] {
        assert!(
            SEAM.contains(required),
            "the controller seam must name `{required}`"
        );
    }
}

#[test]
fn the_checked_in_mixed_fixture_runs_the_two_documented_models() {
    use tangle_model::{CompiledScenario, parse_scenario_source};
    use tangle_sim::{ControllerModelNames, RunConfig, Simulation};

    let source = parse_scenario_source(BENCHMARK).expect("the fixture parses");
    let scenario = CompiledScenario::compile(source).expect("the fixture compiles");
    let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("the fixture runs");
    sim.step();
    let names: ControllerModelNames = sim.controller_models();
    assert_eq!(names.vehicle, "idm");
    assert_eq!(names.pedestrian, "waypoint");
    assert_ne!(
        names.vehicle, names.pedestrian,
        "each mode must report its own model identity"
    );
}
