//! Compiled agent components: composing bundles through the public surface.
//!
//! These tests drive the exported component model the way a compiler and a
//! kernel will: compose a bundle from components alone, then dispatch on the
//! body/motion family the bundle derived. No test passes a mode id or template
//! name, because the component model has no parameter for one.

use glam::DVec2;
use hekate_model::{
    AgentAccess, AgentBehaviorProfile, AgentBody, AgentComponents, AgentCore, AgentFamily,
    AgentIntent, AgentLifecycle, AgentMotion, AgentOccupancy, AgentPose, AgentRoute, AgentVelocity,
    BodyKind, BodySegment, FacilityKind, GroupMembership, GroupRole, MovementId, NominalDirection,
    PedestrianGroupId, PedestrianRouteId, ProfileRange, RuleKind, SocialState, SpeedPolicy,
    TacticalCapabilities, TacticalCapability,
};

fn range(min: f64, max: f64) -> ProfileRange {
    ProfileRange::new(min, max)
}

fn driving_tactics() -> TacticalCapabilities {
    [
        TacticalCapability::Follow,
        TacticalCapability::Stop,
        TacticalCapability::Yield,
        TacticalCapability::ChooseLateralPosition,
        TacticalCapability::ChangeLane,
        TacticalCapability::Overtake,
    ]
    .into_iter()
    .collect()
}

fn path_access(direction: NominalDirection) -> AgentAccess {
    AgentAccess::new(
        vec![FacilityKind::Path],
        direction,
        SpeedPolicy::limited(13.4),
        vec![RuleKind::Yield, RuleKind::Signal],
    )
}

/// A pedestrian: a circle body that walks holonomically.
fn pedestrian_bundle() -> AgentComponents {
    AgentComponents::compose(
        AgentCore::new(
            AgentRoute::Pedestrian(PedestrianRouteId::from_index(0)),
            AgentPose::new(DVec2::new(0.0, -15.0), std::f64::consts::FRAC_PI_2),
            AgentVelocity::new(DVec2::new(0.0, 1.3)),
            AgentIntent::Tactic(TacticalCapability::Follow),
            AgentBehaviorProfile::walking(range(1.0, 1.6), range(0.9, 1.0)),
            AgentLifecycle::Active,
        ),
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
            vec![RuleKind::Free, RuleKind::Stop],
        ),
        AgentOccupancy::OperatorOnly,
        SocialState::GroupMember(GroupMembership::new(
            PedestrianGroupId::from_index(1),
            GroupRole::Member,
        )),
    )
    .expect("a circle that walks composes as the holonomic-circle family")
}

/// A passenger car: an oriented box that steers as a single rigid body.
fn passenger_car_bundle() -> AgentComponents {
    AgentComponents::compose(
        AgentCore::new(
            AgentRoute::Movement(MovementId::from_index(0)),
            AgentPose::new(DVec2::new(-20.0, 0.0), 0.0),
            AgentVelocity::new(DVec2::new(12.0, 0.0)),
            AgentIntent::Travel,
            AgentBehaviorProfile::wheeled(
                range(9.0, 15.0),
                range(1.0, 2.0),
                range(1.2, 2.5),
                range(2.0, 3.5),
                range(0.9, 1.0),
            ),
            AgentLifecycle::Active,
        ),
        AgentBody::Box {
            length_m: range(4.0, 5.2),
            width_m: range(1.7, 2.0),
        },
        AgentMotion::SingleBodyWheeled,
        driving_tactics(),
        path_access(NominalDirection::Forward),
        AgentOccupancy::Fixed { occupants: 2 },
        SocialState::Individual,
    )
    .expect("a box that steers composes as the wheeled-box family")
}

/// An articulated truck: an ordered chain that steers as one kinematic body.
fn articulated_bundle() -> AgentComponents {
    AgentComponents::compose(
        AgentCore::new(
            AgentRoute::Movement(MovementId::from_index(1)),
            AgentPose::new(DVec2::new(-40.0, 0.0), 0.0),
            AgentVelocity::new(DVec2::new(18.0, 0.0)),
            AgentIntent::Travel,
            AgentBehaviorProfile::wheeled(
                range(20.0, 24.0),
                range(1.5, 2.5),
                range(0.6, 1.1),
                range(1.5, 2.5),
                range(0.7, 0.9),
            ),
            AgentLifecycle::Active,
        ),
        AgentBody::ArticulatedChain {
            segments: vec![
                BodySegment::new(range(5.0, 5.5), range(2.4, 2.5)),
                BodySegment::new(range(12.0, 13.6), range(2.4, 2.6)),
            ],
        },
        AgentMotion::ArticulatedWheeled,
        driving_tactics(),
        path_access(NominalDirection::Forward),
        AgentOccupancy::OperatorOnly,
        SocialState::Individual,
    )
    .expect("a chain that steers composes as the articulated-wheeled family")
}

#[test]
fn composes_the_phase_one_families_from_components() {
    let pedestrian = pedestrian_bundle();
    assert_eq!(pedestrian.family(), AgentFamily::HolonomicCircle);
    assert_eq!(pedestrian.body().kind(), BodyKind::Circle);
    assert_eq!(pedestrian.motion(), AgentMotion::HolonomicWalking);

    let car = passenger_car_bundle();
    assert_eq!(car.family(), AgentFamily::WheeledBox);
    assert_eq!(car.body().kind(), BodyKind::Box);
    assert_eq!(car.motion(), AgentMotion::SingleBodyWheeled);

    // The two Phase 1 modes are distinguishable by family alone.
    assert_ne!(pedestrian.family(), car.family());
}

#[test]
fn composes_an_articulated_bundle_from_ordered_segments() {
    let truck = articulated_bundle();
    assert_eq!(truck.family(), AgentFamily::ArticulatedWheeled);
    assert_eq!(truck.body().kind(), BodyKind::ArticulatedChain);
    assert_eq!(
        truck.body().segment_count(),
        2,
        "both chain segments survive"
    );
    let AgentBody::ArticulatedChain { segments } = truck.body() else {
        panic!("the articulated bundle carries a chain body");
    };
    assert!(
        segments[0].length_m().max() < segments[1].length_m().min(),
        "chain order is preserved front to back"
    );
}

#[test]
fn the_family_is_a_function_of_body_and_motion_alone() {
    let pedestrian = pedestrian_bundle();
    // Same body and motion; a different core route, intent, profile, tactics,
    // access, occupancy, and social state.
    let other = AgentComponents::compose(
        AgentCore::new(
            AgentRoute::Pedestrian(PedestrianRouteId::from_index(9)),
            AgentPose::new(DVec2::new(3.0, 7.0), -0.5),
            AgentVelocity::new(DVec2::new(1.0, 0.2)),
            AgentIntent::Travel,
            AgentBehaviorProfile::walking(range(0.9, 1.1), range(0.1, 0.3)),
            AgentLifecycle::Departed,
        ),
        AgentBody::Circle {
            radius_m: range(0.24, 0.28),
        },
        AgentMotion::HolonomicWalking,
        TacticalCapabilities::none(),
        path_access(NominalDirection::Forward),
        AgentOccupancy::Fixed { occupants: 1 },
        SocialState::Individual,
    )
    .expect("the same body and motion compose");
    assert_eq!(other.family(), pedestrian.family());
    assert_ne!(other.core(), pedestrian.core());
    assert_ne!(other.tactics(), pedestrian.tactics());
    assert_ne!(other.access(), pedestrian.access());
    assert_ne!(other.occupancy(), pedestrian.occupancy());
    assert_ne!(other.social(), pedestrian.social());
}

#[test]
fn a_body_and_motion_pair_no_family_serves_is_rejected() {
    let mismatch = AgentComponents::compose(
        AgentCore::new(
            AgentRoute::Movement(MovementId::from_index(0)),
            AgentPose::new(DVec2::ZERO, 0.0),
            AgentVelocity::new(DVec2::ZERO),
            AgentIntent::Travel,
            AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)),
            AgentLifecycle::Active,
        ),
        AgentBody::Circle {
            radius_m: range(0.2, 0.3),
        },
        AgentMotion::SingleBodyWheeled,
        TacticalCapabilities::none(),
        path_access(NominalDirection::Forward),
        AgentOccupancy::OperatorOnly,
        SocialState::Individual,
    )
    .expect_err("a circle has no steering wheeled family");
    assert_eq!(mismatch.body(), BodyKind::Circle);
    assert_eq!(mismatch.motion(), AgentMotion::SingleBodyWheeled);
}
