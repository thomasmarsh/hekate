//! Increment 1 narrow mode templates: parse, validate, compile, and differ.
//!
//! `docs/schema-v2-contract.md` ("mode_templates narrow extensions") adds the
//! `bicycle` and `scooter` version-2 templates with capsule bodies, `facility`
//! access, and the narrow wheeled profile set. This test compiles the checked-in
//! fixture, proves the two narrow templates differ from each other and from
//! `passenger_car` in body, limits, steering/lateral parameters, and access,
//! and proves the compiler's output depends on the body/motion pair and never
//! on the template id: two templates that differ only in id compile to equal
//! components. The last two tests are the no-id-branch guard: a source-text
//! assertion over the shared model modules, plus a falsification probe that the
//! guard detects a branch, ignores prose, and ignores test code.

use tangle_model::{
    AgentBody, AgentFamily, AgentMotion, CompiledModeTemplate, CompiledScenario, FacilityKind,
    ModeTemplateSource, NominalDirection, ProfileRange, compile_mode_template,
    parse_scenario_source_v2, validate_v2,
};

/// The checked-in narrow-mode fixture, the objects this gate compiles.
const FIXTURE: &str = include_str!("fixtures/narrow_mode_templates_v2.json5");

/// The narrow template ids the fixture authors and shared code must not name.
const NARROW_MODE_IDS: [&str; 2] = ["bicycle", "scooter"];

/// The Increment 0 template the narrow templates must differ from.
const PASSENGER_CAR: &str = "passenger_car";

/// The shared model modules a named-mode branch must not appear in.
///
/// `mode_template.rs` is the compiler, `components.rs` is the compiled
/// component model and the family dispatch it derives, `compiled.rs` is the
/// compiled scenario that bundles the templates, `source.rs` is the authored
/// schema, and `validate.rs` is semantic validation. Each is production code
/// the authored templates flow through as data; none may branch on a template
/// id. Test modules (which name authored ids in fixtures) are excluded by
/// [`production_code`].
const SHARED_MODULES: [(&str, &str); 5] = [
    (
        "mode_template.rs (mode-template compiler)",
        include_str!("../src/mode_template.rs"),
    ),
    (
        "components.rs (compiled component model)",
        include_str!("../src/components.rs"),
    ),
    (
        "compiled.rs (compiled scenario)",
        include_str!("../src/compiled.rs"),
    ),
    (
        "source.rs (authored schema)",
        include_str!("../src/source.rs"),
    ),
    (
        "validate.rs (semantic validation)",
        include_str!("../src/validate.rs"),
    ),
];

/// The production code of one module: everything before its `#[cfg(test)]`
/// module. Test fixtures legitimately name authored ids, so a branch guard
/// observes only the code that ships.
fn production_code(module: &str) -> &str {
    module.split("#[cfg(test)]").next().unwrap_or(module)
}

/// Whether a module's production code names a narrow template id as a Rust
/// string literal. A branch on the id is `id == "bicycle"` or `"scooter" =>`;
/// prose (`such as a bicycle or scooter`) and test fixtures carry no literal.
fn production_names_a_narrow_id(module: &str) -> Option<&'static str> {
    let production = production_code(module);
    NARROW_MODE_IDS.into_iter().find(|id| {
        production.contains(&format!("\"{id}\"")) || production.contains(&format!("'{id}'"))
    })
}

/// The parsed template named `id` from the checked-in fixture.
fn fixture_template(id: &str) -> ModeTemplateSource {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    source
        .mode_templates
        .into_iter()
        .find(|template| template.id == id)
        .unwrap_or_else(|| panic!("the fixture declares mode template '{id}'"))
}

/// The compiled bundle for the fixture template named `id`.
fn compiled_template(id: &str) -> CompiledModeTemplate {
    compile_mode_template(&fixture_template(id))
        .unwrap_or_else(|diagnostics| panic!("template '{id}' compiles: {diagnostics:?}"))
}

/// A distribution's `(min, max)` pair.
fn bounds(range: ProfileRange) -> (f64, f64) {
    (range.min(), range.max())
}

#[test]
fn the_narrow_fixture_is_a_valid_version_2_document() {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    assert_eq!(source.schema_version, 2);
    assert_eq!(
        source.mode_templates.len(),
        3,
        "the fixture authors the car and both narrow templates"
    );
    let diagnostics = validate_v2(&source);
    assert!(
        diagnostics.is_empty(),
        "the narrow fixture must be a valid version-2 document, got {diagnostics:?}"
    );
}

#[test]
fn the_narrow_templates_compile_through_the_full_scenario() {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    let scenario = CompiledScenario::compile_v2(source).expect("the narrow scenario compiles");
    let ids: Vec<&str> = scenario
        .mode_templates()
        .iter()
        .map(CompiledModeTemplate::id)
        .collect();
    assert_eq!(ids, [PASSENGER_CAR, "bicycle", "scooter"]);
}

#[test]
fn a_narrow_template_compiles_to_a_capsule_bundle_with_narrow_parameters() {
    for id in NARROW_MODE_IDS {
        let compiled = compiled_template(id);
        assert_eq!(compiled.family(), Some(AgentFamily::WheeledCapsule));
        assert_eq!(compiled.body().kind(), tangle_model::BodyKind::Capsule);
        assert_eq!(compiled.motion(), AgentMotion::SingleBodyWheeled);
        assert_eq!(
            compiled.access().nominal_direction(),
            NominalDirection::Either
        );
        assert_eq!(compiled.access().speed_policy().limit_mps(), None);

        let AgentBody::Capsule { length_m, radius_m } = compiled.body() else {
            panic!("'{id}' authors a capsule body");
        };
        assert!(length_m.min() > 0.0 && radius_m.min() > 0.0);

        let profile = compiled.profile();
        let steering = profile
            .steering_rate_max_rad_s()
            .unwrap_or_else(|| panic!("'{id}' carries a steering-rate profile"));
        let clearance = profile
            .lateral_clearance_m()
            .unwrap_or_else(|| panic!("'{id}' carries a lateral-clearance profile"));
        assert!(steering.min() > 0.0 && steering.max() > 0.0);
        assert!(clearance.min() >= 0.0 && clearance.max() >= 0.0);
        assert!(
            compiled.validate().is_empty(),
            "the compiled '{id}' bundle is internally consistent"
        );
    }
}

#[test]
fn the_narrow_templates_differ_from_each_other_and_from_passenger_car() {
    let car = compiled_template(PASSENGER_CAR);
    let bicycle = compiled_template("bicycle");
    let scooter = compiled_template("scooter");

    // Body: the car is a box; both narrow modes are capsules, and their capsule
    // dimensions differ.
    assert_eq!(car.body().kind(), tangle_model::BodyKind::Box);
    let AgentBody::Capsule {
        length_m: bike_length,
        radius_m: bike_radius,
    } = bicycle.body()
    else {
        panic!("bicycle authors a capsule");
    };
    let AgentBody::Capsule {
        length_m: scooter_length,
        radius_m: scooter_radius,
    } = scooter.body()
    else {
        panic!("scooter authors a capsule");
    };
    assert_ne!(
        bounds(*bike_length),
        bounds(*scooter_length),
        "bicycle and scooter lengths differ"
    );
    assert_ne!(
        bounds(*bike_radius),
        bounds(*scooter_radius),
        "bicycle and scooter radii differ"
    );

    // Limits: bicycle and scooter differ on speed, acceleration, and braking,
    // and both sit below the car's free-flow speed envelope.
    let bike_profile = bicycle.profile();
    let scooter_profile = scooter.profile();
    let car_profile = car.profile();
    assert_ne!(
        bounds(bike_profile.desired_speed_mps()),
        bounds(scooter_profile.desired_speed_mps())
    );
    assert_ne!(
        bounds(bike_profile.max_accel_mps2().expect("wheeled")),
        bounds(scooter_profile.max_accel_mps2().expect("wheeled"))
    );
    assert_ne!(
        bounds(bike_profile.comfortable_brake_mps2().expect("wheeled")),
        bounds(scooter_profile.comfortable_brake_mps2().expect("wheeled"))
    );
    for profile in [bike_profile, scooter_profile] {
        assert!(
            profile.desired_speed_mps().max() < car_profile.desired_speed_mps().min(),
            "a narrow mode's speed envelope stays below the car's"
        );
        assert!(
            profile.time_gap_s().expect("wheeled").min()
                < car_profile.time_gap_s().expect("wheeled").min(),
            "a narrow mode follows more closely than the car"
        );
    }

    // Steering/lateral: the car carries neither narrow parameter; both narrow
    // modes carry both, and their steering response differs.
    assert_eq!(car_profile.steering_rate_max_rad_s(), None);
    assert_eq!(car_profile.lateral_clearance_m(), None);
    let bike_steering = bike_profile
        .steering_rate_max_rad_s()
        .expect("bicycle steers");
    let scooter_steering = scooter_profile
        .steering_rate_max_rad_s()
        .expect("scooter steers");
    assert_ne!(bounds(bike_steering), bounds(scooter_steering));
    assert!(bike_profile.lateral_clearance_m().is_some());
    assert!(scooter_profile.lateral_clearance_m().is_some());

    // Access: the car uses guide paths; both narrow modes use continuous
    // facilities and not paths.
    assert!(car.access().permits(FacilityKind::Path));
    assert!(!car.access().permits(FacilityKind::Facility));
    for compiled in [&bicycle, &scooter] {
        assert!(
            compiled.access().permits(FacilityKind::Facility),
            "a narrow mode serves the facility kind"
        );
        assert!(
            !compiled.access().permits(FacilityKind::Path),
            "a narrow mode is not path-access"
        );
    }
}

#[test]
fn renaming_a_template_id_does_not_change_its_components() {
    // The falsification probe for the no-id-branch rule: if shared code branched
    // on the id, the renamed template would compile to different components.
    for id in NARROW_MODE_IDS {
        let mut renamed = fixture_template(id);
        renamed.id = "a_different_name".to_owned();

        let first = compiled_template(id);
        let second = compile_mode_template(&renamed).expect("renaming the id keeps the components");

        assert_ne!(first.id(), second.id());
        assert_eq!(first.body(), second.body());
        assert_eq!(first.motion(), second.motion());
        assert_eq!(first.tactics(), second.tactics());
        assert_eq!(first.access(), second.access());
        assert_eq!(first.occupancy(), second.occupancy());
        assert_eq!(first.profile(), second.profile());
        assert_eq!(first.family(), second.family());
    }
}

#[test]
fn the_fixture_declares_the_narrow_ids_this_guard_searches_for() {
    for id in NARROW_MODE_IDS {
        assert!(
            FIXTURE.contains(&format!("id: '{id}'")),
            "the fixture must declare mode id '{id}' the guard searches for"
        );
    }
}

#[test]
fn no_shared_model_module_branches_on_a_narrow_template_id() {
    for (name, source) in SHARED_MODULES {
        if let Some(id) = production_names_a_narrow_id(source) {
            panic!(
                "shared model module {name} names narrow template id '{id}'; the templates must \
                 reach shared behavior through their components, not a mode branch"
            );
        }
    }
}

#[test]
fn the_no_id_branch_guard_detects_a_branch_and_ignores_prose_and_tests() {
    // The guard is not vacuous: it flags the branch a regression would add...
    assert_eq!(
        production_names_a_narrow_id("fn f(id: &str) { if id == \"bicycle\" { } }"),
        Some("bicycle")
    );
    assert_eq!(
        production_names_a_narrow_id("match id { \"scooter\" => 1, _ => 0 }"),
        Some("scooter")
    );
    // ...it ignores a doc-comment mention and other prose...
    assert_eq!(
        production_names_a_narrow_id("/// such as a bicycle or scooter body."),
        None
    );
    // ...and it ignores test code, where fixtures legitimately name the ids.
    assert_eq!(
        production_names_a_narrow_id(
            "fn compile() {}\n#[cfg(test)]\nmod tests { const ID: &str = \"bicycle\"; }"
        ),
        None
    );
}
