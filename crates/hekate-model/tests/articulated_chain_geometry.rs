//! Increment 3 tractor-semitrailer geometry: parse, validate, and compile.
//!
//! `scenarios/phase2/inc3/tractor_semitrailer_v2.json5` authors the first
//! checked-in `articulated_chain` body and `articulated_wheeled` motion: a
//! two-segment tractor-semitrailer chain with a hitch offset on its trailing
//! segment and an articulation limit. This node is strictly the static
//! authoring/compile/validate slice named by
//! `tas-4ep58y0syjtnwny4bgcg5q41j7`: hitch kinematics, runtime dispatch, and
//! swept collision are a later child's, so this test proves only that the
//! document parses, validates with no diagnostics, and compiles to the
//! existing `AgentBody::ArticulatedChain`/`AgentMotion::ArticulatedWheeled`
//! components — it makes no runtime-behavior claim.

use hekate_model::{
    AgentBody, AgentFamily, AgentMotion, ModeBodySource, ModeTemplateSource, MotionKind,
    compile_mode_template, parse_scenario_source_v2, validate_v2,
};

/// The checked-in tractor-semitrailer fixture, the document this gate compiles.
const FIXTURE: &str = include_str!("../../../scenarios/phase2/inc3/tractor_semitrailer_v2.json5");

/// The single template the fixture authors.
fn fixture_template() -> ModeTemplateSource {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    source
        .mode_templates
        .into_iter()
        .find(|template| template.id == "tractor_semitrailer")
        .expect("the fixture declares mode template 'tractor_semitrailer'")
}

#[test]
fn the_fixture_is_a_valid_version_2_document() {
    let source = parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
    assert_eq!(source.schema_version, 2);
    assert_eq!(source.mode_templates.len(), 1);

    let diagnostics = validate_v2(&source);
    assert!(
        diagnostics.is_empty(),
        "the tractor-semitrailer fixture must be a valid version-2 document, got {diagnostics:?}"
    );
}

#[test]
fn the_authored_body_is_a_two_segment_articulated_chain() {
    let template = fixture_template();
    assert_eq!(template.motion, MotionKind::ArticulatedWheeled);
    let ModeBodySource::ArticulatedChain {
        segments,
        articulation_limit_rad,
    } = &template.body
    else {
        panic!("the fixture authors an articulated_chain body");
    };
    assert_eq!(segments.len(), 2, "a tractor and one trailer");
    assert!(
        segments[0].hitch_offset_m.is_none(),
        "the lead segment hitches nothing ahead"
    );
    assert!(
        segments[1].hitch_offset_m.is_some(),
        "the trailing segment authors its hitch offset"
    );
    assert!((articulation_limit_rad.min - 0.9).abs() < 1e-12);
    assert!((articulation_limit_rad.max - 0.9).abs() < 1e-12);
}

#[test]
fn the_fixture_compiles_to_the_articulated_wheeled_family_with_pinned_geometry() {
    let template = fixture_template();
    let compiled = compile_mode_template(&template)
        .unwrap_or_else(|diagnostics| panic!("the fixture template compiles: {diagnostics:?}"));

    assert_eq!(compiled.family(), Some(AgentFamily::ArticulatedWheeled));
    assert_eq!(compiled.motion(), AgentMotion::ArticulatedWheeled);
    assert!(
        compiled.validate().is_empty(),
        "the compiled bundle is internally consistent"
    );

    let AgentBody::ArticulatedChain {
        segments,
        articulation_limit_rad,
    } = compiled.body()
    else {
        panic!("the compiled body is an articulated chain");
    };

    // Pin the compiled geometry so a regression in `compiled_body` (a lost
    // segment, a swapped dimension, or a dropped hitch offset) fails this test
    // rather than surfacing later.
    assert_eq!(segments.len(), 2);
    let tractor = segments[0];
    assert!((tractor.length_m().min() - 6.0).abs() < 1e-9);
    assert!((tractor.length_m().max() - 6.0).abs() < 1e-9);
    assert!((tractor.width_m().min() - 2.5).abs() < 1e-9);
    assert!((tractor.width_m().max() - 2.5).abs() < 1e-9);
    assert!(tractor.hitch_offset_m().is_none());

    let trailer = segments[1];
    assert!((trailer.length_m().min() - 13.6).abs() < 1e-9);
    assert!((trailer.length_m().max() - 13.6).abs() < 1e-9);
    assert!((trailer.width_m().min() - 2.55).abs() < 1e-9);
    assert!((trailer.width_m().max() - 2.55).abs() < 1e-9);
    let hitch = trailer
        .hitch_offset_m()
        .expect("the trailer authors a hitch offset");
    assert!((hitch.min() - 1.2).abs() < 1e-9);
    assert!((hitch.max() - 1.2).abs() < 1e-9);

    assert!((articulation_limit_rad.min() - 0.9).abs() < 1e-9);
    assert!((articulation_limit_rad.max() - 0.9).abs() < 1e-9);

    // The mode's widest segment is the trailer, which is also the mode's
    // envelope width.
    assert!((compiled.envelope_width_m() - 2.55).abs() < 1e-9);
}
