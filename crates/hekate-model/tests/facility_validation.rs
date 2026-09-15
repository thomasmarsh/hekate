//! Increment 1 facility, connector, and permission validation rules.
//!
//! Each rule has a rejecting fixture that triggers its stable diagnostic and an
//! accepting fixture that does not. The base fixture is a two-facility bikeway
//! linked by one continuous connector; every reject case rewrites exactly one
//! authored fact from it.

use hekate_model::{CompiledScenario, DiagnosticCode, parse_scenario_source_v2, validate_v2};

/// A valid two-facility bikeway: a capsule `bicycle` mode, a west and an east
/// facility joined by one continuous connector, all inside the world boundary.
const FACILITY: &str = r#"{
    schema_version: 2,
    id: 'facility_case',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'west_path', points: [ { x: 0, y: 0 }, { x: 80, y: 0 } ] },
        { id: 'east_path', points: [ { x: 80, y: 0 }, { x: 160, y: 0 } ] },
    ],
    portals: [
        { id: 'entry', path: 'west_path', end: 'start', width_m: 3.0 },
        { id: 'exit', path: 'east_path', end: 'end', width_m: 3.0 },
    ],
    boundaries: [ { id: 'world', points: [
        { x: -10, y: -10 }, { x: 170, y: -10 }, { x: 170, y: 10 }, { x: -10, y: 10 }
    ] } ],
    regions: [
        { id: 'west_band', points: [
            { x: 0, y: -1.5 }, { x: 80, y: -1.5 }, { x: 80, y: 1.5 }, { x: 0, y: 1.5 }
        ] },
        { id: 'east_band', points: [
            { x: 80, y: -1.5 }, { x: 160, y: -1.5 }, { x: 160, y: 1.5 }, { x: 80, y: 1.5 }
        ] },
    ],
    mode_templates: [ {
        id: 'bicycle',
        body: { kind: 'capsule', length_m: { min: 1.6, max: 1.9 },
            radius_m: { min: 0.30, max: 0.40 } },
        motion: 'single_body_wheeled',
        tactics: [ 'follow', 'stop', 'yield' ],
        access: { facility_kinds: [ 'facility' ] },
        occupancy: 'operator_only',
        profiles: {
            speed_mps: { min: 3.5, max: 6.5 },
            max_accel_mps2: { min: 0.8, max: 1.5 },
            comfortable_brake_mps2: { min: 1.5, max: 3.0 },
            time_gap_s: { min: 0.8, max: 1.4 },
            steering_rate_max_rad_s: { min: 0.6, max: 1.2 },
            lateral_clearance_m: { min: 0.20, max: 0.50 },
            compliance: { min: 0.8, max: 1.0 },
        },
    } ],
    facilities: [
        { id: 'west', region: 'west_band', reference_path: 'west_path', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
        { id: 'east', region: 'east_band', reference_path: 'east_path', width_m: 3.5,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: 8.0 } },
    ],
    facility_connectors: [
        { id: 'west_to_east', from: { facility: 'west', direction: 'forward' },
            to: { facility: 'east', direction: 'forward' } },
    ],
    permissions: [],
}"#;

fn codes(text: &str) -> Vec<&'static str> {
    let source = parse_scenario_source_v2(text).expect("fixture parses");
    validate_v2(&source)
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

/// Rewrite one authored fact, failing loudly if the base fixture no longer
/// contains it.
fn rewrite(from: &str, to: &str) -> String {
    let rewritten = FACILITY.replace(from, to);
    assert_ne!(rewritten, FACILITY, "base fixture must contain {from:?}");
    rewritten
}

#[test]
fn accepts_a_well_formed_facility_document() {
    assert_eq!(
        codes(FACILITY),
        Vec::<&str>::new(),
        "the base bikeway is valid"
    );
}

#[test]
fn compiles_a_well_formed_facility_document() {
    let source = parse_scenario_source_v2(FACILITY).expect("parses");
    let compiled = CompiledScenario::compile_v2(source).expect("compiles");
    assert_eq!(compiled.facilities().len(), 2);
    assert_eq!(compiled.facility_connectors().len(), 1);
    assert_eq!(compiled.id_map().facilities(), ["west", "east"]);
}

#[test]
fn rejects_a_facility_region_outside_the_world() {
    let outside = rewrite(
        "{ x: 0, y: -1.5 }, { x: 80, y: -1.5 }, { x: 80, y: 1.5 }, { x: 0, y: 1.5 }",
        "{ x: 0, y: -1.5 }, { x: 80, y: -1.5 }, { x: 80, y: 1.5 }, { x: 0, y: 50.0 }",
    );
    assert!(codes(&outside).contains(&"E_FACILITY_OUTSIDE_WORLD"));
}

#[test]
fn rejects_a_facility_too_narrow_for_the_largest_eligible_body() {
    // A 1.0 m band cannot fit the 0.8 m capsule plus its 0.5 m clearance.
    let narrow = rewrite(
        "reference_path: 'west_path', width_m: 3.0",
        "reference_path: 'west_path', width_m: 1.0",
    );
    assert!(codes(&narrow).contains(&"E_FACILITY_TOO_NARROW"));
}

#[test]
fn rejects_a_discontinuous_connector() {
    let shifted = rewrite(
        "{ id: 'east_path', points: [ { x: 80, y: 0 }, { x: 160, y: 0 } ] }",
        "{ id: 'east_path', points: [ { x: 81, y: 0 }, { x: 160, y: 0 } ] }",
    );
    assert!(codes(&shifted).contains(&"E_FACILITY_CONNECTOR_DISCONTINUOUS"));
}

#[test]
fn rejects_an_unreachable_nominal_direction() {
    // The west facility's only connector traverses it forward, so a reverse
    // nominal direction is not physically possible.
    let reversed = rewrite(
        "nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },\n            lateral_use: 'shared', speed_policy: { limit_mps: null }",
        "nominal_direction: 'reverse', access: { modes: [ 'bicycle' ] },\n            lateral_use: 'shared', speed_policy: { limit_mps: null }",
    );
    assert!(codes(&reversed).contains(&"E_FACILITY_UNREACHABLE_DIRECTION"));
}

#[test]
fn rejects_a_mode_whose_template_does_not_serve_facilities() {
    let denied = rewrite(
        "access: { facility_kinds: [ 'facility' ] }",
        "access: { facility_kinds: [ 'path' ] }",
    );
    assert!(codes(&denied).contains(&"E_FACILITY_ACCESS_DENIED"));
}

#[test]
fn rejects_a_facility_that_permits_no_mode() {
    let empty = rewrite(
        "access: { modes: [ 'bicycle' ] },\n            lateral_use: 'shared', speed_policy: { limit_mps: null }",
        "access: { modes: [] },\n            lateral_use: 'shared', speed_policy: { limit_mps: null }",
    );
    assert!(codes(&empty).contains(&"E_FACILITY_ACCESS_EMPTY"));
}

#[test]
fn rejects_a_facility_with_an_undeclared_reference() {
    let unknown_region = rewrite("region: 'west_band'", "region: 'ghost_band'");
    assert!(codes(&unknown_region).contains(&"E_FACILITY_UNKNOWN_REGION"));

    let unknown_path = rewrite(
        "reference_path: 'west_path'",
        "reference_path: 'ghost_path'",
    );
    assert!(codes(&unknown_path).contains(&"E_FACILITY_UNKNOWN_PATH"));

    let unknown_mode = rewrite(
        "access: { modes: [ 'bicycle' ] },\n            lateral_use: 'shared', speed_policy: { limit_mps: null }",
        "access: { modes: [ 'ghost' ] },\n            lateral_use: 'shared', speed_policy: { limit_mps: null }",
    );
    assert!(codes(&unknown_mode).contains(&"E_FACILITY_UNKNOWN_MODE"));
}

#[test]
fn rejects_a_directional_facility_without_a_reference_path() {
    let headless = rewrite(
        "reference_path: 'west_path', width_m: 3.0,",
        "width_m: 3.0,",
    );
    let found = codes(&headless);
    assert!(found.contains(&"E_FACILITY_DIRECTION_WITHOUT_PATH"));
    assert!(found.contains(&"E_FACILITY_CONNECTOR_WITHOUT_REFERENCE"));
}

#[test]
fn rejects_invalid_facility_width_and_speed_limit() {
    let zero_width = rewrite(
        "reference_path: 'west_path', width_m: 3.0",
        "reference_path: 'west_path', width_m: 0.0",
    );
    assert!(codes(&zero_width).contains(&"E_FACILITY_WIDTH"));

    let zero_limit = rewrite(
        "speed_policy: { limit_mps: null }",
        "speed_policy: { limit_mps: 0.0 }",
    );
    assert!(codes(&zero_limit).contains(&"E_FACILITY_SPEED_LIMIT"));
}

#[test]
fn rejects_a_connector_to_an_undeclared_facility() {
    let ghost = rewrite(
        "to: { facility: 'east', direction: 'forward' }",
        "to: { facility: 'ghost', direction: 'forward' }",
    );
    assert!(codes(&ghost).contains(&"E_FACILITY_CONNECTOR_UNKNOWN_FACILITY"));
}

#[test]
fn rejects_a_connector_that_attaches_a_facility_without_a_reference() {
    let headless = rewrite("reference_path: 'east_path', width_m: 3.5", "width_m: 3.5");
    assert!(codes(&headless).contains(&"E_FACILITY_CONNECTOR_WITHOUT_REFERENCE"));
}

#[test]
fn rejects_undeclared_permission_references() {
    let unknown_holder = rewrite(
        "permissions: [],",
        "permissions: [ { id: 'p', kind: 'nominal_direction', holder: 'ghost', target: 'west', effect: 'permit' } ],",
    );
    assert!(codes(&unknown_holder).contains(&"E_PERMISSION_UNKNOWN_HOLDER"));

    let unknown_target = rewrite(
        "permissions: [],",
        "permissions: [ { id: 'p', kind: 'nominal_direction', holder: 'bicycle', target: 'ghost', effect: 'permit' } ],",
    );
    assert!(codes(&unknown_target).contains(&"E_PERMISSION_UNKNOWN_TARGET"));
}

#[test]
fn distinguishes_an_illegal_route_from_a_physically_impossible_one() {
    // The west facility grants the bicycle access, but a permission prohibits
    // it: the route is illegal, not physically impossible.
    let prohibited = rewrite(
        "permissions: [],",
        "permissions: [ { id: 'p', kind: 'nominal_direction', holder: 'bicycle', target: 'west', effect: 'prohibit' } ],",
    );
    let found = codes(&prohibited);
    assert!(found.contains(&"E_PERMISSION_ROUTE_PROHIBITED"));
    assert!(!found.contains(&"E_FACILITY_TOO_NARROW"));
    assert!(!found.contains(&"E_FACILITY_CURVATURE"));
    assert!(!found.contains(&"E_FACILITY_UNREACHABLE_DIRECTION"));

    // A permit or obligation on the same route is a legal route.
    let permitted = rewrite(
        "permissions: [],",
        "permissions: [ { id: 'p', kind: 'nominal_direction', holder: 'bicycle', target: 'west', effect: 'obligate' } ],",
    );
    assert_eq!(codes(&permitted), Vec::<&str>::new());
}

#[test]
fn requires_the_narrow_wheeled_steering_and_clearance_profiles() {
    let no_steering = rewrite(
        "steering_rate_max_rad_s: { min: 0.6, max: 1.2 },\n            lateral_clearance_m: { min: 0.20, max: 0.50 },",
        "lateral_clearance_m: { min: 0.20, max: 0.50 },",
    );
    assert!(codes(&no_steering).contains(&"E_MODE_TEMPLATE_PROFILE"));

    let no_clearance = rewrite(
        "steering_rate_max_rad_s: { min: 0.6, max: 1.2 },\n            lateral_clearance_m: { min: 0.20, max: 0.50 },",
        "steering_rate_max_rad_s: { min: 0.6, max: 1.2 },",
    );
    assert!(codes(&no_clearance).contains(&"E_MODE_TEMPLATE_PROFILE"));
}

#[test]
fn accepts_a_zero_lateral_clearance_and_rejects_a_negative_one() {
    let zero = rewrite(
        "lateral_clearance_m: { min: 0.20, max: 0.50 }",
        "lateral_clearance_m: { min: 0.0, max: 0.0 }",
    );
    assert_eq!(codes(&zero), Vec::<&str>::new());

    let negative = rewrite(
        "lateral_clearance_m: { min: 0.20, max: 0.50 }",
        "lateral_clearance_m: { min: -0.10, max: 0.50 }",
    );
    assert!(codes(&negative).contains(&"E_PROFILE_NON_NEGATIVE"));
}

#[test]
fn distinguishes_an_absent_speed_limit_from_an_explicit_null() {
    // The whole policy is required.
    let no_policy = rewrite(", speed_policy: { limit_mps: null } },", " },");
    assert!(
        parse_scenario_source_v2(&no_policy).is_err(),
        "a facility without speed_policy must fail to parse"
    );

    // `limit_mps` is itself required and nullable: absent is a parse error,
    // an explicit null validates as unlimited.
    let no_limit = rewrite("speed_policy: { limit_mps: null }", "speed_policy: {}");
    assert!(
        parse_scenario_source_v2(&no_limit).is_err(),
        "a speed policy without limit_mps must fail to parse"
    );
    assert_eq!(codes(FACILITY), Vec::<&str>::new());

    let limited = rewrite(
        "speed_policy: { limit_mps: null }",
        "speed_policy: { limit_mps: 4.0 }",
    );
    assert_eq!(codes(&limited), Vec::<&str>::new());
}

#[test]
fn compile_v2_rejects_a_malformed_facility_without_panicking() {
    let unknown_region = rewrite("region: 'west_band'", "region: 'ghost_band'");
    let source = parse_scenario_source_v2(&unknown_region).expect("parses");
    let diagnostics = CompiledScenario::compile_v2(source).expect_err("must be rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::FacilityUnknownRegion)
    );
}

#[test]
fn diagnostic_codes_have_stable_strings() {
    assert_eq!(
        DiagnosticCode::FacilityTooNarrow.as_str(),
        "E_FACILITY_TOO_NARROW"
    );
    assert_eq!(
        DiagnosticCode::PermissionRouteProhibited.as_str(),
        "E_PERMISSION_ROUTE_PROHIBITED"
    );
    assert_eq!(
        DiagnosticCode::ProfileNonNegativeInvalid.as_str(),
        "E_PROFILE_NON_NEGATIVE"
    );
}
