//! Mode template compilation: golden bundles and impossible-combination rejection.
//!
//! These tests drive the public compiler the way a reader and the kernel will:
//! compile an authored template into a [`CompiledModeTemplate`], check the
//! exact bundle a valid template produces, and construct bundles directly to
//! prove every impossible combination is rejected with a stable diagnostic
//! naming the authored template id. No test passes a mode name into the
//! compiler; the bundle depends only on the authored fields.

use std::collections::BTreeMap;

use glam::DVec2;
use hekate_model::{
    AccessSource, AgentAccess, AgentBehaviorProfile, AgentBody, AgentCore, AgentFamily,
    AgentIntent, AgentLifecycle, AgentMotion, AgentOccupancy, AgentPose, AgentRoute, AgentVelocity,
    BodySegment, CompiledModeTemplate, DiagnosticCode, FacilityKind, ModeBodySource,
    ModeTemplateSource, MotionKind, NominalDirection, OccupancyKind, PedestrianRouteId,
    ProfileRange, ProfileRangeSource, SocialState, SpeedPolicy, TacticKind, TacticalCapabilities,
    TacticalCapability, TransitOccupancy, compile_mode_template,
};

fn range(min: f64, max: f64) -> ProfileRange {
    ProfileRange::new(min, max)
}

fn profile_range(min: f64, max: f64) -> ProfileRangeSource {
    ProfileRangeSource { min, max }
}

fn profiles(entries: &[(&str, f64, f64)]) -> BTreeMap<String, ProfileRangeSource> {
    entries
        .iter()
        .map(|(name, min, max)| ((*name).to_owned(), profile_range(*min, *max)))
        .collect()
}

/// The Increment 0 `passenger_car` template from `docs/schema-v2-contract.md`.
fn passenger_car_template() -> ModeTemplateSource {
    ModeTemplateSource {
        id: "passenger_car".to_owned(),
        body: ModeBodySource::Box {
            length_m: profile_range(4.0, 5.2),
            width_m: profile_range(1.7, 2.0),
        },
        motion: MotionKind::SingleBodyWheeled,
        tactics: vec![TacticKind::Follow, TacticKind::Stop, TacticKind::Yield],
        access: AccessSource {
            facility_kinds: vec![FacilityKind::Path],
            nominal_direction: None,
            speed_policy: None,
        },
        occupancy: OccupancyKind::OperatorOnly,
        profiles: profiles(&[
            ("speed_mps", 9.0, 15.0),
            ("max_accel_mps2", 1.2, 2.5),
            ("comfortable_brake_mps2", 2.0, 3.5),
            ("time_gap_s", 1.0, 2.0),
            ("compliance", 1.0, 1.0),
        ]),
        lateral: None,
        wheelbase_m: None,
        steering_angle_max_rad: None,
    }
}

/// The Increment 0 `pedestrian` template from `docs/schema-v2-contract.md`.
fn pedestrian_template() -> ModeTemplateSource {
    ModeTemplateSource {
        id: "pedestrian".to_owned(),
        body: ModeBodySource::Circle {
            radius_m: profile_range(0.20, 0.30),
        },
        motion: MotionKind::HolonomicWalking,
        tactics: vec![TacticKind::Follow, TacticKind::Stop, TacticKind::Yield],
        access: AccessSource {
            facility_kinds: vec![
                FacilityKind::Path,
                FacilityKind::Crossing,
                FacilityKind::WaitingArea,
            ],
            nominal_direction: None,
            speed_policy: None,
        },
        occupancy: OccupancyKind::OperatorOnly,
        profiles: profiles(&[("speed_mps", 1.0, 1.6), ("compliance", 1.0, 1.0)]),
        lateral: None,
        wheelbase_m: None,
        steering_angle_max_rad: None,
    }
}

/// A box body that steers, for the impossible-combination bundles.
fn wheeled_box_components() -> (AgentBody, AgentMotion) {
    (
        AgentBody::Box {
            length_m: range(10.0, 12.0),
            width_m: range(2.5, 2.6),
        },
        AgentMotion::SingleBodyWheeled,
    )
}

#[test]
fn a_valid_wheeled_template_compiles_to_the_expected_bundle() {
    let compiled =
        compile_mode_template(&passenger_car_template()).expect("passenger car compiles");
    let expected = CompiledModeTemplate::new(
        "passenger_car".to_owned(),
        AgentBody::Box {
            length_m: range(4.0, 5.2),
            width_m: range(1.7, 2.0),
        },
        AgentMotion::SingleBodyWheeled,
        [
            TacticalCapability::Follow,
            TacticalCapability::Stop,
            TacticalCapability::Yield,
        ]
        .into_iter()
        .collect(),
        AgentAccess::new(
            vec![FacilityKind::Path],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            Vec::new(),
        ),
        AgentOccupancy::OperatorOnly,
        AgentBehaviorProfile::wheeled(
            range(9.0, 15.0),
            range(1.0, 2.0),
            range(1.2, 2.5),
            range(2.0, 3.5),
            range(1.0, 1.0),
        ),
    );
    assert_eq!(
        compiled, expected,
        "the compiled passenger-car bundle is the golden value"
    );
    assert_eq!(compiled.family(), Some(AgentFamily::WheeledBox));
    assert!(compiled.validate().is_empty());
}

#[test]
fn a_valid_walking_template_compiles_to_the_expected_bundle() {
    let compiled = compile_mode_template(&pedestrian_template()).expect("pedestrian compiles");
    let expected = CompiledModeTemplate::new(
        "pedestrian".to_owned(),
        AgentBody::Circle {
            radius_m: range(0.20, 0.30),
        },
        AgentMotion::HolonomicWalking,
        [
            TacticalCapability::Follow,
            TacticalCapability::Stop,
            TacticalCapability::Yield,
        ]
        .into_iter()
        .collect(),
        AgentAccess::new(
            vec![
                FacilityKind::Path,
                FacilityKind::Crossing,
                FacilityKind::WaitingArea,
            ],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            Vec::new(),
        ),
        AgentOccupancy::OperatorOnly,
        AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)),
    );
    assert_eq!(
        compiled, expected,
        "the compiled pedestrian bundle is the golden value"
    );
    assert_eq!(compiled.family(), Some(AgentFamily::HolonomicCircle));
    assert!(compiled.validate().is_empty());
}

#[test]
fn the_compiled_bundle_depends_only_on_the_authored_fields() {
    let mut renamed = passenger_car_template();
    renamed.id = "a_different_name".to_owned();

    let first = compile_mode_template(&passenger_car_template()).expect("compiles");
    let second = compile_mode_template(&renamed).expect("compiles");

    assert_eq!(first.body(), second.body());
    assert_eq!(first.motion(), second.motion());
    assert_eq!(first.tactics(), second.tactics());
    assert_eq!(first.access(), second.access());
    assert_eq!(first.occupancy(), second.occupancy());
    assert_eq!(first.profile(), second.profile());
    assert_ne!(first.id(), second.id());
}

#[test]
fn a_compiled_template_composes_with_one_agents_core_and_social_state() {
    let template = compile_mode_template(&pedestrian_template()).expect("pedestrian compiles");
    let agent = template
        .compose(
            AgentCore::new(
                AgentRoute::Pedestrian(PedestrianRouteId::from_index(0)),
                AgentPose::new(DVec2::new(3.0, -2.0), 0.5),
                AgentVelocity::new(DVec2::new(0.0, 1.3)),
                AgentIntent::Travel,
                *template.profile(),
                AgentLifecycle::Active,
            ),
            SocialState::Individual,
        )
        .expect("a circle body with holonomic walking motion composes");
    assert_eq!(agent.family(), AgentFamily::HolonomicCircle);
    assert_eq!(agent.body(), template.body());
    assert_eq!(agent.occupancy(), template.occupancy());
}

#[test]
fn compose_rejects_a_template_whose_body_and_motion_no_family_serves() {
    let template = CompiledModeTemplate::new(
        "circle_steering".to_owned(),
        AgentBody::Circle {
            radius_m: range(0.20, 0.30),
        },
        AgentMotion::SingleBodyWheeled,
        TacticalCapabilities::none(),
        AgentAccess::new(
            vec![FacilityKind::Path],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            Vec::new(),
        ),
        AgentOccupancy::OperatorOnly,
        AgentBehaviorProfile::wheeled(
            range(4.0, 6.0),
            range(1.0, 1.5),
            range(1.0, 2.0),
            range(2.0, 3.0),
            range(0.8, 1.0),
        ),
    );
    let mismatch = template
        .compose(
            AgentCore::new(
                AgentRoute::Pedestrian(PedestrianRouteId::from_index(0)),
                AgentPose::new(DVec2::ZERO, 0.0),
                AgentVelocity::new(DVec2::ZERO),
                AgentIntent::Travel,
                *template.profile(),
                AgentLifecycle::Active,
            ),
            SocialState::Individual,
        )
        .expect_err("a circle cannot steer as a single rigid wheeled body");
    assert!(mismatch.to_string().contains("no agent family"));
}

#[test]
fn rejects_articulation_parameters_on_a_holonomic_body() {
    let bundle = CompiledModeTemplate::new(
        "holonomic_lorry".to_owned(),
        AgentBody::ArticulatedChain {
            segments: vec![BodySegment::new(range(5.0, 5.5), range(2.4, 2.5), None)],
            articulation_limit_rad: range(0.7, 0.9),
        },
        AgentMotion::HolonomicWalking,
        TacticalCapabilities::none(),
        AgentAccess::new(
            vec![FacilityKind::Path],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            Vec::new(),
        ),
        AgentOccupancy::OperatorOnly,
        AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)),
    );
    assert_eq!(bundle.family(), None);

    let diagnostics = bundle.validate();
    assert_eq!(diagnostics.len(), 1, "one impossible combination");
    assert_eq!(diagnostics[0].code, DiagnosticCode::ModeTemplateBodyMotion);
    assert_eq!(diagnostics[0].code.as_str(), "E_MODE_TEMPLATE_BODY_MOTION");
    assert_eq!(diagnostics[0].object.as_deref(), Some("holonomic_lorry"));
}

#[test]
fn rejects_transit_dwell_without_capacity() {
    let (body, motion) = wheeled_box_components();
    let profile = AgentBehaviorProfile::wheeled(
        range(9.0, 15.0),
        range(1.0, 2.0),
        range(1.2, 2.5),
        range(2.0, 3.5),
        range(1.0, 1.0),
    );
    let access = || {
        AgentAccess::new(
            vec![FacilityKind::Path],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            Vec::new(),
        )
    };

    // Transit occupancy with a zero passenger capacity.
    let zero_capacity = CompiledModeTemplate::new(
        "transit_zero".to_owned(),
        body.clone(),
        motion,
        [TacticalCapability::ServeStop].into_iter().collect(),
        access(),
        AgentOccupancy::Transit(TransitOccupancy::new(0, 0)),
        profile,
    );
    let diagnostics = zero_capacity.validate();
    assert_eq!(diagnostics.len(), 1, "one impossible combination");
    assert_eq!(diagnostics[0].code, DiagnosticCode::ModeTemplateOccupancy);
    assert_eq!(diagnostics[0].code.as_str(), "E_MODE_TEMPLATE_OCCUPANCY");
    assert_eq!(diagnostics[0].object.as_deref(), Some("transit_zero"));

    // A stop-service (dwell) tactic with no transit capacity at all.
    let dwell_without_capacity = CompiledModeTemplate::new(
        "dwell_no_capacity".to_owned(),
        body,
        motion,
        [TacticalCapability::ServeStop].into_iter().collect(),
        access(),
        AgentOccupancy::OperatorOnly,
        profile,
    );
    let diagnostics = dwell_without_capacity.validate();
    assert_eq!(diagnostics.len(), 1, "one impossible combination");
    assert_eq!(diagnostics[0].code, DiagnosticCode::ModeTemplateOccupancy);
    assert_eq!(diagnostics[0].object.as_deref(), Some("dwell_no_capacity"));
}

#[test]
fn accepts_a_transit_template_that_declares_capacity() {
    let (body, motion) = wheeled_box_components();
    let bundle = CompiledModeTemplate::new(
        "bus".to_owned(),
        body,
        motion,
        [TacticalCapability::ServeStop].into_iter().collect(),
        AgentAccess::new(
            vec![FacilityKind::Path],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            Vec::new(),
        ),
        AgentOccupancy::Transit(TransitOccupancy::new(60, 0)),
        AgentBehaviorProfile::wheeled(
            range(9.0, 15.0),
            range(1.0, 2.0),
            range(1.2, 2.5),
            range(2.0, 3.5),
            range(1.0, 1.0),
        ),
    );
    assert!(bundle.validate().is_empty());
    assert_eq!(bundle.family(), Some(AgentFamily::WheeledBox));
}

#[test]
fn the_compiler_rejects_an_impossible_source_combination_naming_the_template() {
    // A box body that walks holonomically: invalid per the version-2 contract,
    // and the compiler rejects it with the same stable code as validation.
    let template = ModeTemplateSource {
        id: "walking_box".to_owned(),
        body: ModeBodySource::Box {
            length_m: profile_range(4.0, 5.2),
            width_m: profile_range(1.7, 2.0),
        },
        motion: MotionKind::HolonomicWalking,
        tactics: vec![TacticKind::Follow],
        access: AccessSource {
            facility_kinds: vec![FacilityKind::Path],
            nominal_direction: None,
            speed_policy: None,
        },
        occupancy: OccupancyKind::OperatorOnly,
        profiles: profiles(&[("speed_mps", 1.0, 1.6), ("compliance", 1.0, 1.0)]),
        lateral: None,
        wheelbase_m: None,
        steering_angle_max_rad: None,
    };
    let error = compile_mode_template(&template).expect_err("a box cannot walk");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, DiagnosticCode::ModeTemplateBodyMotion);
    assert_eq!(error[0].object.as_deref(), Some("walking_box"));
}

#[test]
fn the_compiler_rejects_a_missing_profile_naming_the_template() {
    let mut template = passenger_car_template();
    template.profiles.remove("time_gap_s");
    let error = compile_mode_template(&template).expect_err("a missing profile is rejected");
    assert!(
        error.iter().any(
            |diagnostic| diagnostic.code == DiagnosticCode::ModeTemplateProfile
                && diagnostic.object.as_deref() == Some("passenger_car")
        ),
        "the missing-profile diagnostic names the authored template id"
    );
}
