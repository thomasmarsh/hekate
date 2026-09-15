//! Phase 1 Increment 3 slice E: the initial vehicle and pedestrian model cards,
//! plus the Increment 1 bicycle and scooter model cards.
//!
//! Deliverable 5 requires explicit, replaceable controller interfaces and a
//! model card for each initial model. The interfaces are types, and the
//! behavioral proof that the kernel reaches both modes only through them is the
//! stub-controller swap in `crates/hekate-sim/src/controller.rs` plus the
//! measured fixture in `mixed_interaction.rs`. The cards are documentation, so
//! this test checks the card inventory directly from the source text with
//! `include_str!`: the required sections come from the checked-in model-card
//! template, so a card that loses a section, or states the sections out of the
//! template's order, fails here, and a card that stops naming its model family
//! or its replaceable interface fails here too. The narrow wheeled module
//! carries one `# Model card` block per mode (bicycle and scooter); this test
//! extracts each block and checks it separately. That is drift protection for a
//! documentation deliverable, and the one part of the deliverable no other test
//! can observe.

/// The vehicle model card: the documented IDM longitudinal controller.
const VEHICLE_CARD: &str = include_str!("../src/control.rs");
/// The pedestrian model card: the documented waypoint controller.
const PEDESTRIAN_CARD: &str = include_str!("../src/pedestrian.rs");
/// The narrow wheeled model module, which carries the bicycle and scooter cards
/// as two `# Model card` blocks in its module documentation.
const NARROW_MODULE: &str = include_str!("../src/narrow.rs");
/// The controller seam and model-card index.
const SEAM: &str = include_str!("../src/controller.rs");
/// The checked-in mixed gate fixture, run through both documented models.
const BENCHMARK: &str = include_str!("../../../scenarios/benchmarks/mixed_interaction_v1.json5");
/// The checked-in model-card template, the inventory of record for the required
/// sections every card must state, in order.
const TEMPLATE: &str = include_str!("../../../docs/model-card-template.md");

/// The required sections, in order, read from the template's `##` headings.
///
/// The template is the inventory of record, so the test derives the list from it
/// rather than hardcoding one: a new required section is a template edit, and
/// every card must then state it too. The template's `##` headings are its
/// section list, so the template itself uses `##` for nothing else.
fn required_sections() -> Vec<String> {
    TEMPLATE
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(str::trim)
        .map(str::to_owned)
        .collect()
}

/// Every `# Model card` block in `source`, as `(heading, block)`.
///
/// A block runs from its `# Model card` heading to the next one (or the end of
/// the source), so one module can carry one card per mode while the ordered
/// section scan stays per card.
fn model_card_blocks(source: &'static str) -> Vec<(&'static str, &'static str)> {
    let mut starts = Vec::new();
    let mut from = 0;
    while let Some(offset) = source[from..].find("# Model card") {
        let at = from + offset;
        starts.push(at);
        from = at + "# Model card".len();
    }
    let mut blocks = Vec::new();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(source.len());
        let block = &source[start..end];
        let heading = block.lines().next().unwrap_or("").trim();
        blocks.push((heading, block));
    }
    blocks
}

/// The narrow wheeled cards, in module order.
fn narrow_cards() -> Vec<(&'static str, &'static str)> {
    model_card_blocks(NARROW_MODULE)
}

/// The sections a card must carry, in order, as `(card name, text)`.
fn cards() -> Vec<(&'static str, &'static str)> {
    let mut cards = vec![
        ("vehicle (control.rs)", VEHICLE_CARD),
        ("pedestrian (pedestrian.rs)", PEDESTRIAN_CARD),
    ];
    cards.extend(narrow_cards());
    cards
}

#[test]
fn every_initial_model_card_states_the_full_model_inventory() {
    let sections = required_sections();
    assert!(
        !sections.is_empty(),
        "the model-card template must name at least one required section"
    );
    let cards = cards();
    assert_eq!(
        cards.len(),
        4,
        "the inventory is the vehicle, pedestrian, bicycle, and scooter cards"
    );
    for (name, card) in cards {
        assert!(
            card.contains("# Model card"),
            "the {name} card must open with a `# Model card` heading"
        );
        let mut cursor = 0;
        for section in &sections {
            let heading = format!("## {section}");
            let Some(found) = card[cursor..].find(&heading) else {
                panic!("the {name} card is missing the `{heading}` section in template order");
            };
            cursor += found + heading.len();
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

    // The narrow wheeled module carries one card per mode. Each names its mode,
    // the model family it shares with the vehicle, the replaceable narrow
    // interface, and the shared longitudinal cap counter.
    let narrow = narrow_cards();
    assert_eq!(
        narrow.len(),
        2,
        "the narrow module must carry the bicycle and scooter cards"
    );
    assert!(
        narrow[0].0.contains("bicycle"),
        "the first narrow card names the bicycle mode"
    );
    assert!(
        narrow[1].0.contains("scooter"),
        "the second narrow card names the scooter mode"
    );
    for (heading, card) in narrow {
        assert!(
            card.contains("Intelligent Driver Model"),
            "the {heading} card must cite the model family"
        );
        assert!(
            card.contains("NarrowWheeledController"),
            "the {heading} card must name its replaceable interface"
        );
        assert!(
            card.contains("emergency_cap_steps"),
            "the {heading} card must name the counted seam"
        );
    }
}

#[test]
fn the_seam_indexes_the_cards_and_interfaces() {
    for required in [
        "VehicleController",
        "PedestrianController",
        "ControllerModels",
        "crate::control",
        "crate::pedestrian",
        "Intelligent Driver Model",
        "Coulter",
        "docs/model-card-template.md",
        "NarrowWheeledController",
        "IdmNarrowWheeledController",
        "crate::narrow",
    ] {
        assert!(
            SEAM.contains(required),
            "the controller seam must name `{required}`"
        );
    }
}

#[test]
fn the_checked_in_mixed_fixture_runs_the_documented_models() {
    use hekate_model::{CompiledScenario, parse_scenario_source};
    use hekate_sim::{ControllerModelNames, RunConfig, Simulation};

    let source = parse_scenario_source(BENCHMARK).expect("the fixture parses");
    let scenario = CompiledScenario::compile(source).expect("the fixture compiles");
    let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("the fixture runs");
    sim.step();
    let names: ControllerModelNames = sim.controller_models();
    assert_eq!(names.vehicle, "idm");
    assert_eq!(names.narrow, "idm-narrow");
    assert_eq!(names.pedestrian, "waypoint");
    assert_ne!(
        names.vehicle, names.narrow,
        "each family must report its own model identity"
    );
    assert_ne!(names.narrow, names.pedestrian);
}
