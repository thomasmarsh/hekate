//! The synthetic version-2 mode template of the Increment 0 extension gate.
//!
//! `PHASE_2_PLAN.md` "Agent composition" requires a mode template to compile to
//! components and change body dimensions, kinematic limits, and facility access
//! without adding a named-mode branch to shared interaction code. This test
//! covers the compilation half: the checked-in synthetic fixture parses as a
//! version-2 document, `compile_mode_template` carries every altered authored
//! value into the compiled components, and the derived family comes from the
//! body and motion, not the template id. The runtime half — those components
//! driving the shared stages — lives in `crates/hekate-sim/src/sim.rs`, and the
//! no-branch guard lives in
//! `crates/hekate-sim/tests/synthetic_template_no_branch.rs`.

use std::collections::BTreeMap;

use hekate_model::{
    AgentBody, AgentFamily, AgentMotion, AgentOccupancy, FacilityKind, ModeBodySource,
    ModeTemplateSource, NominalDirection, ProfileRangeSource, compile_mode_template,
    parse_scenario_source_v2, validate_v2,
};

/// The checked-in synthetic template fixture, the object this gate compiles.
const FIXTURE: &str = include_str!("fixtures/synthetic_mode_template_v2.json5");

/// The synthetic template id the fixture authors.
///
/// The id is only an authoring handle: it must not select a family or a code
/// path. The components are the contract.
const SYNTHETIC_ID: &str = "synthetic_hauler";

/// The Increment 0 passenger-car bounding envelope from
/// `docs/schema-v2-contract.md`; the synthetic template must lie outside it.
const CAR_MAX_LENGTH_M: f64 = 5.2;
const CAR_MAX_WIDTH_M: f64 = 2.0;
const CAR_MIN_SPEED_MPS: f64 = 9.0;
const CAR_MAX_TIME_GAP_S: f64 = 2.0;
const CAR_MIN_MAX_ACCEL_MPS2: f64 = 1.2;
const CAR_MIN_BRAKE_MPS2: f64 = 2.0;

/// The Increment 0 pedestrian maximum desired speed from
/// `docs/schema-v2-contract.md`.
const PEDESTRIAN_MAX_SPEED_MPS: f64 = 1.6;

/// The authored synthetic template, parsed from the checked-in fixture.
fn synthetic_template() -> ModeTemplateSource {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    assert_eq!(
        source.mode_templates.len(),
        1,
        "the fixture declares exactly one mode template"
    );
    let template = source
        .mode_templates
        .into_iter()
        .next()
        .expect("the fixture declares one mode template");
    assert_eq!(
        template.id, SYNTHETIC_ID,
        "the fixture names the synthetic id"
    );
    template
}

/// An authored profile range as its `(min, max)` pair.
fn authored_range(profiles: &BTreeMap<String, ProfileRangeSource>, name: &str) -> (f64, f64) {
    let range = profiles
        .get(name)
        .unwrap_or_else(|| panic!("the fixture declares profile '{name}'"));
    (range.min, range.max)
}

#[test]
fn the_synthetic_fixture_is_a_valid_version_2_document() {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    assert_eq!(source.schema_version, 2);
    let diagnostics = validate_v2(&source);
    assert!(
        diagnostics.is_empty(),
        "the synthetic fixture must be a valid version-2 document, got {diagnostics:?}"
    );
}

#[test]
fn the_synthetic_template_compiles_to_the_authored_components() {
    let source = synthetic_template();
    let compiled = compile_mode_template(&source).expect("the synthetic template compiles");

    assert_eq!(compiled.id(), SYNTHETIC_ID);
    assert!(compiled.validate().is_empty());

    // Body dimensions: the compiled box equals the authored box.
    let ModeBodySource::Box {
        length_m: authored_length,
        width_m: authored_width,
    } = &source.body
    else {
        panic!("the synthetic template authors a box body");
    };
    let AgentBody::Box {
        length_m: compiled_length,
        width_m: compiled_width,
    } = compiled.body()
    else {
        panic!("the compiled synthetic template carries a box body");
    };
    assert_eq!(
        (compiled_length.min(), compiled_length.max()),
        (authored_length.min, authored_length.max)
    );
    assert_eq!(
        (compiled_width.min(), compiled_width.max()),
        (authored_width.min, authored_width.max)
    );

    // Motion, occupancy, and access: the compiled values equal the authored
    // ones, with the compiler's documented defaults for the deferred access
    // fields.
    assert_eq!(compiled.motion(), AgentMotion::SingleBodyWheeled);
    assert_eq!(compiled.occupancy(), &AgentOccupancy::OperatorOnly);
    assert_eq!(
        compiled.access().facility_kinds(),
        source.access.facility_kinds.as_slice(),
        "the compiled facility access equals the authored facility kinds"
    );
    assert_eq!(
        compiled.access().nominal_direction(),
        NominalDirection::Either,
        "Increment 0 defers nominal direction and defaults to either"
    );

    // Kinematic limits: every compiled distribution equals the authored range.
    let profile = compiled.profile();
    assert_eq!(
        (
            profile.desired_speed_mps().min(),
            profile.desired_speed_mps().max()
        ),
        authored_range(&source.profiles, "speed_mps")
    );
    assert_eq!(
        (
            profile.max_accel_mps2().expect("wheeled").min(),
            profile.max_accel_mps2().expect("wheeled").max()
        ),
        authored_range(&source.profiles, "max_accel_mps2")
    );
    assert_eq!(
        (
            profile.comfortable_brake_mps2().expect("wheeled").min(),
            profile.comfortable_brake_mps2().expect("wheeled").max()
        ),
        authored_range(&source.profiles, "comfortable_brake_mps2")
    );
    assert_eq!(
        (
            profile.time_gap_s().expect("wheeled").min(),
            profile.time_gap_s().expect("wheeled").max()
        ),
        authored_range(&source.profiles, "time_gap_s")
    );
    assert_eq!(
        (profile.compliance().min(), profile.compliance().max()),
        authored_range(&source.profiles, "compliance")
    );

    // The family is the one the body/motion pair selects.
    assert_eq!(compiled.family(), Some(AgentFamily::WheeledBox));
}

#[test]
fn the_derived_family_comes_from_the_body_and_motion_not_the_template_id() {
    let first =
        compile_mode_template(&synthetic_template()).expect("the synthetic template compiles");
    let mut renamed = synthetic_template();
    renamed.id = "a_different_name".to_owned();
    let second = compile_mode_template(&renamed).expect("renaming the id keeps the components");

    assert_ne!(first.id(), second.id());
    assert_eq!(first.family(), second.family());
    assert_eq!(first.body(), second.body());
    assert_eq!(first.motion(), second.motion());
    assert_eq!(first.access(), second.access());
    assert_eq!(first.profile(), second.profile());
}

#[test]
fn the_synthetic_components_differ_from_the_increment_0_templates() {
    let compiled =
        compile_mode_template(&synthetic_template()).expect("the synthetic template compiles");
    let AgentBody::Box { length_m, width_m } = compiled.body() else {
        panic!("the synthetic template carries a box body");
    };
    let profile = compiled.profile();

    // Body dimensions exceed the passenger-car box.
    assert!(
        length_m.min() > CAR_MAX_LENGTH_M,
        "the synthetic length {} is not longer than the car's {CAR_MAX_LENGTH_M}",
        length_m.min()
    );
    assert!(
        width_m.min() > CAR_MAX_WIDTH_M,
        "the synthetic width {} is not wider than the car's {CAR_MAX_WIDTH_M}",
        width_m.min()
    );

    // Desired speed sits below the car's free-flow envelope and above the
    // pedestrian's, so it is a limit neither existing mode shares.
    let speed = profile.desired_speed_mps().min();
    assert!(speed < CAR_MIN_SPEED_MPS, "speed {speed}");
    assert!(speed > PEDESTRIAN_MAX_SPEED_MPS, "speed {speed}");

    // Acceleration and braking are gentler than the car's, and the following
    // gap is longer.
    assert!(
        profile.max_accel_mps2().expect("wheeled").max() < CAR_MIN_MAX_ACCEL_MPS2,
        "acceleration must be below the car's floor"
    );
    assert!(
        profile.comfortable_brake_mps2().expect("wheeled").max() < CAR_MIN_BRAKE_MPS2,
        "braking must be below the car's floor"
    );
    assert!(
        profile.time_gap_s().expect("wheeled").min() > CAR_MAX_TIME_GAP_S,
        "time gap must exceed the car's ceiling"
    );

    // Access differs from both: it reaches crossings the car may not use, and
    // it is not permitted in the pedestrian waiting areas.
    let access = compiled.access();
    assert!(access.permits(FacilityKind::Crossing));
    assert!(!access.permits(FacilityKind::WaitingArea));
}
