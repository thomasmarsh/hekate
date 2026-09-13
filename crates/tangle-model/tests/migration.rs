//! Deterministic version-1 to version-2 migration.
//!
//! These tests pin the transform a later provenance step records: it is a pure
//! function of the version-1 document, it is a fixed point of its own canonical
//! form, and its bytes match the checked-in golden version 2 for a demand
//! scenario and for the Phase 1 walking-skeleton population.

use std::path::PathBuf;

use tangle_model::{
    CompiledScenario, MIGRATION_VERSION, MovementDirection, SUPPORTED_SCHEMA_VERSION,
    migrate_v1_to_v2, parse_scenario_source, parse_scenario_source_v2, to_canonical_v2_json,
    validate_v2,
};

/// A checked-in migration fixture, by file name.
fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/migration")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("fixture '{}' is checked in: {error}", path.display()))
}

/// The canonical normalized version 2 of one version-1 fixture.
fn migrated_bytes(v1: &str) -> String {
    let source = parse_scenario_source(v1).expect("the version-1 fixture parses");
    to_canonical_v2_json(&migrate_v1_to_v2(&source))
}

#[test]
fn migration_version_is_one() {
    assert_eq!(MIGRATION_VERSION, 1);
}

#[test]
fn migration_is_a_pure_function_of_the_version_1_document() {
    let v1 = fixture("demand_v1.json5");
    let source = parse_scenario_source(&v1).expect("the demand fixture parses");

    let first = migrate_v1_to_v2(&source);
    let second = migrate_v1_to_v2(&source);
    assert_eq!(first, second, "the transform has no hidden state");

    // Re-parsing the same bytes yields the same transform and the same bytes:
    // nothing in the mapping reads a clock, randomness, or a file.
    assert_eq!(migrated_bytes(&v1), to_canonical_v2_json(&first));
    assert_eq!(migrated_bytes(&v1), migrated_bytes(&v1));
}

#[test]
fn migration_is_idempotent_on_its_own_output() {
    for name in ["demand_v1.json5", "population_v1.json5"] {
        let normalized = migrated_bytes(&fixture(name));
        let reparsed =
            parse_scenario_source_v2(&normalized).expect("the normalized document parses");
        assert_eq!(
            to_canonical_v2_json(&reparsed),
            normalized,
            "re-normalizing '{name}' must be a fixed point"
        );
    }
}

#[test]
fn demand_fixture_migrates_to_its_golden_version_2() {
    let migrated = migrated_bytes(&fixture("demand_v1.json5"));
    assert_eq!(
        migrated,
        fixture("demand_v2.json"),
        "the demand fixture must migrate byte-for-byte to its golden version 2"
    );
}

#[test]
fn population_fixture_migrates_to_its_golden_version_2() {
    let migrated = migrated_bytes(&fixture("population_v1.json5"));
    assert_eq!(
        migrated,
        fixture("population_v2.json"),
        "the population fixture must migrate byte-for-byte to its golden version 2"
    );
}

#[test]
fn migrated_documents_are_valid_version_2() {
    for name in ["demand_v1.json5", "population_v1.json5"] {
        let normalized = migrated_bytes(&fixture(name));
        let source = parse_scenario_source_v2(&normalized).expect("the normalized document parses");
        assert_eq!(
            source.schema_version, SUPPORTED_SCHEMA_VERSION,
            "a normalized document is version 2"
        );
        assert_eq!(
            validate_v2(&source),
            Vec::new(),
            "the normalized document is valid version 2"
        );
    }
}

#[test]
fn movements_carry_the_direction_their_from_portal_fixes() {
    let source = parse_scenario_source(&fixture("demand_v1.json5")).expect("parses");
    let migrated = migrate_v1_to_v2(&source);

    // `eastbound` starts at the path's start portal and `westbound` at its end
    // portal, so the authored portal order fixes each direction explicitly.
    let directions: Vec<(&str, MovementDirection)> = migrated
        .movements
        .iter()
        .map(|movement| (movement.id.as_str(), movement.direction))
        .collect();
    assert_eq!(
        directions,
        vec![
            ("eastbound", MovementDirection::Forward),
            ("westbound", MovementDirection::Reverse),
        ]
    );

    // Both version-1 demand generators become mode-tagged version-2 demand.
    let modes: Vec<(&str, &str)> = migrated
        .demand
        .iter()
        .map(|entry| (entry.id.as_str(), entry.mode.as_str()))
        .collect();
    assert_eq!(
        modes,
        vec![
            ("eastbound_inflow", "passenger_car"),
            ("westbound_inflow", "passenger_car"),
            ("crossing_footfall", "pedestrian"),
        ]
    );
}

#[test]
fn migration_preserves_the_compiled_walking_skeleton() {
    let v1_source =
        parse_scenario_source(&fixture("population_v1.json5")).expect("the population fixture");
    let migrated = migrate_v1_to_v2(&v1_source);

    let v1 = CompiledScenario::compile(v1_source).expect("the version-1 walking skeleton compiles");
    let v2 = CompiledScenario::compile_v2(migrated).expect("the migrated scenario compiles");

    // Phase 1 behavior is unchanged: every compiled field is equal. The source
    // schema version is the one field that legitimately differs, because a
    // compiled scenario records the version its source declared.
    assert_eq!(v1.schema_version(), 1);
    assert_eq!(v2.schema_version(), SUPPORTED_SCHEMA_VERSION);
    assert_eq!(v1.id(), v2.id());
    assert_eq!(v1.paths(), v2.paths());
    assert_eq!(v1.portals(), v2.portals());
    assert_eq!(v1.boundaries(), v2.boundaries());
    assert_eq!(v1.regions(), v2.regions());
    assert_eq!(v1.movements(), v2.movements());
    assert_eq!(v1.crossings(), v2.crossings());
    assert_eq!(v1.waiting_areas(), v2.waiting_areas());
    assert_eq!(v1.pedestrian_routes(), v2.pedestrian_routes());
    assert_eq!(v1.conflict_regions(), v2.conflict_regions());
    assert_eq!(v1.rules(), v2.rules());
    assert_eq!(v1.signals(), v2.signals());
    assert_eq!(v1.demand(), v2.demand());
    assert_eq!(v1.pedestrian_demand(), v2.pedestrian_demand());
    assert_eq!(v1.profiles(), v2.profiles());
    assert_eq!(v1.pedestrian_profiles(), v2.pedestrian_profiles());
    assert_eq!(v1.population(), v2.population());
    assert_eq!(v1.id_map(), v2.id_map());
}
