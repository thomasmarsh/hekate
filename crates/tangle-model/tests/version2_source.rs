//! Version-2 source shapes: version negotiation, validation, and compilation.
//!
//! These tests exercise the public entry points a reader uses: negotiation via
//! [`parse_scenario_document`], structural parsing via
//! [`parse_scenario_source_v2`], semantic validation via [`validate_v2`], and
//! compilation via [`CompiledScenario::compile_v2`].

use tangle_model::{
    CompiledScenario, DiagnosticCode, ScenarioDocument, parse_scenario_document,
    parse_scenario_source, parse_scenario_source_v2, validate, validate_v2,
};

/// A version-1 flow: one guide path, one movement, and portal demand.
const V1_FLOW: &str = r#"{
    schema_version: 1,
    id: 'v1_flow',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] } ],
    portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
    ],
    movements: [ { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 } ],
    demand: [ { id: 'inflow', portal: 'entry', rate_vph: 600.0,
        routes: [ { movement: 'through', weight: 1.0 } ] } ],
    profiles: {
        speed_mps: { min: 9.0, max: 15.0 },
        length_m: { min: 4.0, max: 5.2 },
        width_m: { min: 1.7, max: 2.0 },
        time_gap_s: { min: 1.0, max: 2.0 },
        max_accel_mps2: { min: 1.2, max: 2.5 },
        comfortable_brake_mps2: { min: 2.0, max: 3.5 },
    },
}"#;

/// The version-2 equivalent of [`V1_FLOW`]: the same layout with an explicit
/// movement direction, a passenger-car mode template, and mode-tagged demand.
const V2_FLOW: &str = r#"{
    schema_version: 2,
    id: 'v1_flow',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] } ],
    portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
    ],
    movements: [ { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
        direction: 'forward' } ],
    mode_templates: [ {
        id: 'passenger_car',
        body: { kind: 'box', length_m: { min: 4.0, max: 5.2 },
            width_m: { min: 1.7, max: 2.0 } },
        motion: 'single_body_wheeled',
        tactics: [ 'follow', 'stop', 'yield' ],
        access: { facility_kinds: [ 'path' ] },
        occupancy: 'operator_only',
        profiles: {
            speed_mps: { min: 9.0, max: 15.0 },
            max_accel_mps2: { min: 1.2, max: 2.5 },
            comfortable_brake_mps2: { min: 2.0, max: 3.5 },
            time_gap_s: { min: 1.0, max: 2.0 },
            compliance: { min: 1.0, max: 1.0 },
        },
    } ],
    demand: [ { id: 'inflow', mode: 'passenger_car',
        spawn: { rate: {
            portal: 'entry',
            rate_per_hour: 600.0,
            interval_s: { start_s: 0.0, end_s: null },
            choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
        } } } ],
}"#;

fn v2_source() -> tangle_model::ScenarioSourceV2 {
    parse_scenario_source_v2(V2_FLOW).expect("version-2 flow parses")
}

#[test]
fn version_2_document_parses_validates_and_compiles() {
    let document = parse_scenario_document(V2_FLOW).expect("version-2 flow is negotiated");
    let ScenarioDocument::V2(source) = document else {
        panic!("a schema_version: 2 document must negotiate to a version-2 source");
    };
    assert_eq!(
        validate_v2(&source),
        Vec::new(),
        "the version-2 flow is valid"
    );

    let compiled = CompiledScenario::compile_v2(source).expect("the version-2 flow compiles");
    assert_eq!(compiled.schema_version(), 2);
    assert_eq!(compiled.demand().len(), 1);
    assert_eq!(compiled.demand()[0].rate_vph(), 600.0);
    assert_eq!(compiled.movements().len(), 1);
}

#[test]
fn version_2_compilation_matches_the_version_1_compiled_fields() {
    let v1 =
        CompiledScenario::compile(parse_scenario_source(V1_FLOW).expect("version-1 flow parses"))
            .expect("version-1 flow compiles");
    let v2 = CompiledScenario::compile_v2(v2_source()).expect("version-2 flow compiles");

    // Version 2 does not change compiled behavior in this increment: the
    // materialized fields equal what the equivalent version-1 source produces.
    assert_eq!(v1.paths(), v2.paths());
    assert_eq!(v1.portals(), v2.portals());
    assert_eq!(v1.movements(), v2.movements());
    assert_eq!(v1.profiles(), v2.profiles());
    assert_eq!(v1.demand(), v2.demand());
    assert_eq!(v1.pedestrian_demand(), v2.pedestrian_demand());
    assert_eq!(v1.population(), v2.population());
}

#[test]
fn version_1_document_still_reads_validates_and_compiles() {
    let document = parse_scenario_document(V1_FLOW).expect("version-1 flow is negotiated");
    let ScenarioDocument::V1(source) = document else {
        panic!("a schema_version: 1 document must negotiate to a version-1 source");
    };
    assert_eq!(
        validate(&source),
        Vec::new(),
        "version 1 keeps its direct validation path"
    );
    assert!(CompiledScenario::compile(source).is_ok());
}

#[test]
fn unknown_schema_version_is_rejected_with_the_stable_diagnostic() {
    let unknown = V2_FLOW.replace("schema_version: 2", "schema_version: 3");
    let error = parse_scenario_document(&unknown).expect_err("version 3 is not readable");
    let diagnostic = error
        .diagnostic()
        .expect("a version failure carries a diagnostic");
    assert_eq!(diagnostic.code, DiagnosticCode::UnsupportedSchemaVersion);
    assert_eq!(diagnostic.code.as_str(), "E_SCHEMA_VERSION");
}

#[test]
fn missing_required_version_2_field_is_rejected_not_defaulted() {
    // `direction` is required on every version-2 movement; it is not defaulted
    // to the version-1 implicit order.
    let no_direction = V2_FLOW.replace("direction: 'forward'", "");
    assert!(
        parse_scenario_source_v2(&no_direction).is_err(),
        "a movement without direction must fail to parse"
    );

    // `mode` is required on every version-2 demand source.
    let no_mode = V2_FLOW.replace("mode: 'passenger_car',", "");
    assert!(
        parse_scenario_source_v2(&no_mode).is_err(),
        "a demand source without mode must fail to parse"
    );

    // A profile the family requires is a validation failure, not a fallback to
    // the version-1 compliance default.
    let no_compliance = V2_FLOW.replace("compliance: { min: 1.0, max: 1.0 },", "");
    let source = parse_scenario_source_v2(&no_compliance).expect("structurally parses");
    let codes: Vec<&str> = validate_v2(&source)
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"E_MODE_TEMPLATE_PROFILE"));
}

#[test]
fn version_2_validation_rejects_inconsistent_shapes() {
    // A circular body cannot use the wheeled motion family.
    let body_mismatch = V2_FLOW
        .replace(
            "body: { kind: 'box', length_m: { min: 4.0, max: 5.2 },\n            width_m: { min: 1.7, max: 2.0 } }",
            "body: { kind: 'circle', radius_m: { min: 0.2, max: 0.3 } }",
        );
    assert_ne!(body_mismatch, V2_FLOW, "fixture body must be rewritten");
    let found = codes(&body_mismatch);
    assert!(found.contains(&"E_MODE_TEMPLATE_BODY_MOTION"));

    // A declared mode template must be referenced by demand.
    let unknown_mode = V2_FLOW.replace("mode: 'passenger_car',", "mode: 'ghost',");
    assert!(codes(&unknown_mode).contains(&"E_DEMAND_UNKNOWN_MODE"));

    // A movement's explicit direction must agree with its portal order.
    let reversed = V2_FLOW.replace("direction: 'forward'", "direction: 'reverse'");
    assert!(codes(&reversed).contains(&"E_MOVEMENT_DIRECTION"));

    // A movement demand must reference a declared movement starting at its portal.
    let unknown_movement = V2_FLOW.replace(
        "{ movement: 'through', weight: 1.0 }",
        "{ movement: 'ghost', weight: 1.0 }",
    );
    assert!(codes(&unknown_movement).contains(&"E_DEMAND_UNKNOWN_MOVEMENT"));
}

fn codes(document: &str) -> Vec<&'static str> {
    let source = parse_scenario_source_v2(document).expect("fixture parses");
    validate_v2(&source)
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}
