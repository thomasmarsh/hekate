//! Increment 2 version-2 source shapes: mode lateral parameters, facility
//! lateral policy, side-by-side adjacencies, clearance bands, and maneuver
//! policy.
//!
//! The exact shapes are fixed by `docs/schema-v2-contract.md` under *Increment
//! 2 additions: lateral motion, passing, and wrong-way travel*. These tests
//! cover the parse-only contract: structural parsing with
//! [`parse_scenario_source_v2`], canonical serialization, the Increment 1
//! absence rules, and strictness on malformed tags or unknown fields.
//! Compilation and cross-reference validation of the new shapes belong to
//! later leaves.

use hekate_model::{
    AdjacencySide, CommitPolicySource, FacilityLateralPolicySource, ManeuverPolicySource,
    ModeLateralSource, PassingSide, PermissionKind, ScenarioSourceV2, TacticKind,
    WrongWayPolicySource, parse_scenario_source_v2, to_canonical_v2_json,
};

/// One document authoring every Increment 2 source shape, in normalized
/// declaration order, with the contract's normalized values.
const INCREMENT_2: &str = r#"{
    schema_version: 2,
    id: 'increment_2_shapes',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'bikeway_centerline', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
        { id: 'road_centerline', points: [ { x: 0.0, y: 3.0 }, { x: 120.0, y: 3.0 } ] },
    ],
    portals: [],
    regions: [
        { id: 'bikeway_band', points: [
            { x: 0.0, y: -1.5 }, { x: 120.0, y: -1.5 },
            { x: 120.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
        { id: 'road_band', points: [
            { x: 0.0, y: 1.5 }, { x: 120.0, y: 1.5 },
            { x: 120.0, y: 4.5 }, { x: 0.0, y: 4.5 } ] },
    ],
    mode_templates: [ {
        id: 'bicycle',
        body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
            radius_m: { min: 0.35, max: 0.35 } },
        motion: 'single_body_wheeled',
        tactics: [ 'follow', 'change_lane', 'overtake', 'pass', 'reverse_direction' ],
        access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
        occupancy: 'operator_only',
        profiles: {
            speed_mps: { min: 4.5, max: 4.5 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
            lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
            lateral_clearance_m: { min: 0.3, max: 0.3 },
            compliance: { min: 1.0, max: 1.0 },
        },
        lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
    } ],
    facilities: [
        { id: 'bikeway_eastbound', region: 'bikeway_band',
            reference_path: 'bikeway_centerline', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', lateral_policy: { passing_side: 'most_clearance' },
            speed_policy: { limit_mps: null } },
        { id: 'road_eastbound_curb', region: 'road_band',
            reference_path: 'road_centerline', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_adjacencies: [
        { id: 'bikeway_to_roadside', first: 'bikeway_eastbound',
            second: 'road_eastbound_curb', side: 'right' },
    ],
    clearance_bands: [
        { id: 'narrow_close_pass', threshold_m: 0.75, violation: true,
            applies_to_modes: [ 'bicycle' ] },
        { id: 'motor_close_pass', threshold_m: 1.0, violation: true,
            applies_to_modes: [ 'passenger_car' ] },
        { id: 'study_band', threshold_m: 1.5, violation: false },
    ],
    maneuver_policy: {
        commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
        wrong_way: { min_time_saving_s: 10.0, max_opposing_density_per_km: 40.0,
            urgency: 0.5 },
    },
    permissions: [
        { id: 'bicycle_may_overtake', kind: 'overtake', holder: 'bicycle',
            target: 'bikeway_eastbound', effect: 'permit' },
        { id: 'bicycle_holds_side', kind: 'lane_use', holder: 'bicycle',
            target: 'bikeway_eastbound', effect: 'obligate' },
        { id: 'bicycle_may_cross', kind: 'crossing', holder: 'bicycle',
            target: 'crossing_north', effect: 'permit' },
    ],
}"#;

/// One document authoring no Increment 2 field: the Increment 1 behaviour.
const INCREMENT_1: &str = r#"{
    schema_version: 2,
    id: 'increment_1_shapes',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 20.0, y: 0.0 } ] } ],
    portals: [],
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
}"#;

fn increment_2() -> ScenarioSourceV2 {
    parse_scenario_source_v2(INCREMENT_2).expect("the Increment 2 document parses")
}

#[test]
fn increment_2_source_shapes_parse_with_their_authored_values() {
    let source = increment_2();

    let template = &source.mode_templates[0];
    assert_eq!(
        template.tactics,
        [
            TacticKind::Follow,
            TacticKind::ChangeLane,
            TacticKind::Overtake,
            TacticKind::Pass,
            TacticKind::ReverseDirection,
        ]
    );
    assert_eq!(
        template.lateral,
        Some(ModeLateralSource {
            target_clearance_m: 0.75,
            horizon_s: 4.0,
        })
    );

    assert_eq!(
        source.facilities[0].lateral_policy,
        Some(FacilityLateralPolicySource {
            passing_side: PassingSide::MostClearance,
        })
    );
    assert_eq!(source.facilities[1].lateral_policy, None);

    assert_eq!(source.facility_adjacencies.len(), 1);
    let adjacency = &source.facility_adjacencies[0];
    assert_eq!(adjacency.id, "bikeway_to_roadside");
    assert_eq!(adjacency.first, "bikeway_eastbound");
    assert_eq!(adjacency.second, "road_eastbound_curb");
    assert_eq!(adjacency.side, AdjacencySide::Right);

    // Bands keep their authored ids and declaration order; the ordering, not a
    // sort, is what makes the reported set deterministic.
    let bands: Vec<(&str, f64, bool)> = source
        .clearance_bands
        .iter()
        .map(|band| (band.id.as_str(), band.threshold_m, band.violation))
        .collect();
    assert_eq!(
        bands,
        [
            ("narrow_close_pass", 0.75, true),
            ("motor_close_pass", 1.0, true),
            ("study_band", 1.5, false),
        ]
    );
    assert_eq!(
        source.clearance_bands[0].applies_to_modes,
        Some(vec!["bicycle".to_owned()])
    );
    assert_eq!(source.clearance_bands[2].applies_to_modes, None);

    assert_eq!(
        source.maneuver_policy,
        Some(ManeuverPolicySource {
            commit: Some(CommitPolicySource {
                min_predicted_clearance_m: 0.25,
                hold_timeout_s: 2.0,
            }),
            wrong_way: Some(WrongWayPolicySource {
                min_time_saving_s: 10.0,
                max_opposing_density_per_km: 40.0,
                urgency: 0.5,
            }),
        })
    );

    let kinds: Vec<PermissionKind> = source
        .permissions
        .iter()
        .map(|permission| permission.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            PermissionKind::Overtake,
            PermissionKind::LaneUse,
            PermissionKind::Crossing,
        ]
    );
}

#[test]
fn increment_2_source_shapes_round_trip_with_authored_precision() {
    let source = increment_2();
    let canonical = to_canonical_v2_json(&source);

    // Authored precision is preserved, not rounded or rescaled.
    assert!(canonical.contains("\"target_clearance_m\": 0.75"));
    assert!(canonical.contains("\"horizon_s\": 4.0"));
    assert!(canonical.contains("\"threshold_m\": 1.0"));
    assert!(canonical.contains("\"max_opposing_density_per_km\": 40.0"));
    assert!(canonical.contains("\"urgency\": 0.5"));

    let reparsed = parse_scenario_source_v2(&canonical).expect("canonical JSON reparses");
    assert_eq!(reparsed, source, "a canonical round trip is lossless");
    assert_eq!(
        to_canonical_v2_json(&reparsed),
        canonical,
        "re-serializing a parsed document reproduces the same bytes"
    );
}

#[test]
fn absent_increment_2_fields_keep_the_increment_1_behavior() {
    let source = parse_scenario_source_v2(INCREMENT_1).expect("the Increment 1 document parses");

    assert!(source.facility_adjacencies.is_empty());
    assert!(source.clearance_bands.is_empty());
    assert!(source.maneuver_policy.is_none());
    assert!(source.mode_templates[0].lateral.is_none());
    assert!(source.facilities.is_empty());

    // An omitted field is omitted from the normalized document, so a scenario
    // authoring none of Increment 2 serializes exactly as Increment 1 did.
    let canonical = to_canonical_v2_json(&source);
    for absent in [
        "\"facility_adjacencies\"",
        "\"clearance_bands\"",
        "\"maneuver_policy\"",
        "\"lateral\"",
    ] {
        assert!(
            !canonical.contains(absent),
            "an unauthored Increment 2 field must not be serialized: {absent}"
        );
    }
}

#[test]
fn malformed_increment_2_tags_and_unknown_fields_are_rejected() {
    // No alias tags: the four tactic values are spelled exactly as authored.
    let alias_tactic = INCREMENT_2.replace("'change_lane'", "'change_lanes'");
    assert_ne!(
        alias_tactic, INCREMENT_2,
        "the tactic tag must be rewritten"
    );
    assert!(
        parse_scenario_source_v2(&alias_tactic).is_err(),
        "a tactic alias must not be accepted"
    );

    // No alias enum spelling for the passing side.
    let alias_side = INCREMENT_2.replace("'most_clearance'", "'most-clearance'");
    assert!(
        parse_scenario_source_v2(&alias_side).is_err(),
        "a passing-side alias must not be accepted"
    );

    // An unknown adjacency side is not an implicit direction.
    let bad_side = INCREMENT_2.replace("side: 'right'", "side: 'up'");
    assert!(
        parse_scenario_source_v2(&bad_side).is_err(),
        "an unknown side tag must fail to parse"
    );

    // No implicit units: a unit-bearing string is not a number.
    let implicit_clearance =
        INCREMENT_2.replace("target_clearance_m: 0.75", "target_clearance_m: '0.75 m'");
    assert!(
        parse_scenario_source_v2(&implicit_clearance).is_err(),
        "a unit-bearing string must not parse as a metre value"
    );
    let implicit_horizon = INCREMENT_2.replace("horizon_s: 4.0", "horizon_s: '4 s'");
    assert!(
        parse_scenario_source_v2(&implicit_horizon).is_err(),
        "a unit-bearing string must not parse as a second value"
    );

    // Unknown fields stay rejected at every level that denies them.
    let unknown_top = INCREMENT_2.replace("schema_version: 2", "schema_version: 2, typo: true");
    assert!(
        parse_scenario_source_v2(&unknown_top).is_err(),
        "an unknown top-level field must fail to parse"
    );
    let unknown_policy = INCREMENT_2.replace("maneuver_policy: {", "maneuver_policy: { typo: 1,");
    assert!(
        parse_scenario_source_v2(&unknown_policy).is_err(),
        "an unknown maneuver-policy field must fail to parse"
    );

    // A required field inside a present object is not defaulted.
    let missing_violation =
        INCREMENT_2.replace("threshold_m: 0.75, violation: true,", "threshold_m: 0.75,");
    assert!(
        parse_scenario_source_v2(&missing_violation).is_err(),
        "a present band without `violation` must fail to parse"
    );
}
