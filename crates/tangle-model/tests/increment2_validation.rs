//! Increment 2 lateral, clearance, transition, and wrong-way validation.
//!
//! The rules are fixed by `docs/schema-v2-contract.md` under *Increment 2
//! additions: lateral motion, passing, and wrong-way travel* → the
//! `mode_templates[]` lateral capabilities and maneuver parameters, the
//! `facilities[]` lateral-use policy, `facility_adjacencies[]`,
//! `clearance_bands[]`, `maneuver_policy`, and the `permissions[]` resolution
//! rules. Each rule has a stable [`DiagnosticCode`] and a table-driven rejecting
//! case; the base document (`base_document_is_valid`) is the accepting case.
//!
//! The no-opposing-path case is deliberately *not* a rejection. The contract
//! makes a `nominal_direction` `permit`/`obligate` whose target has no
//! physically connected opposing traversal inert — the permitted set never
//! leaves the physically possible set — and closes it at runtime through the
//! wrong-way decision's `no_opposing_path` reason. `an_unconnected_opposing_
//! statement_is_inert_for_the_traversal` proves that.

use tangle_model::{
    AdjacencySide, CompiledScenario, DiagnosticCode, DirectionSet, FacilityDirection, FacilityId,
    LateralUse, ModeTemplateId, MovementDirection, PassingSide, PermissionEffect, ScenarioSourceV2,
    TacticKind, parse_scenario_source_v2, validate_v2,
};

/// One well-formed Increment 2 document: a lateral-capable capsule mode, a
/// facility with a fixed lateral policy, a side-by-side adjacency between two
/// touching bands, a crossing, three permissions, two increasing clearance
/// bands, and a complete maneuver policy.
const INCREMENT_2: &str = r#"{
    schema_version: 2,
    id: 'increment_2_validation',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'east_path', points: [ { x: 0.0, y: 0.0 }, { x: 100.0, y: 0.0 } ] },
        { id: 'west_path', points: [ { x: 0.0, y: -3.0 }, { x: 100.0, y: -3.0 } ] },
        { id: 'cross_path', points: [ { x: 60.0, y: -8.0 }, { x: 60.0, y: 8.0 } ] },
    ],
    portals: [
        { id: 'cross_south', path: 'cross_path', end: 'start', width_m: 2.0 },
        { id: 'cross_north', path: 'cross_path', end: 'end', width_m: 2.0 },
    ],
    regions: [
        { id: 'east_band', points: [
            { x: 0.0, y: -1.5 }, { x: 100.0, y: -1.5 },
            { x: 100.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
        { id: 'west_band', points: [
            { x: 0.0, y: -4.5 }, { x: 100.0, y: -4.5 },
            { x: 100.0, y: -1.5 }, { x: 0.0, y: -1.5 } ] },
        { id: 'crossing_zone', points: [
            { x: 57.0, y: -3.0 }, { x: 63.0, y: -3.0 },
            { x: 63.0, y: 3.0 }, { x: 57.0, y: 3.0 } ] },
    ],
    mode_templates: [ {
        id: 'bicycle',
        body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
            radius_m: { min: 0.35, max: 0.35 } },
        motion: 'single_body_wheeled',
        tactics: [ 'follow', 'stop', 'yield', 'change_lane', 'overtake', 'pass',
            'reverse_direction' ],
        access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
        occupancy: 'operator_only',
        profiles: {
            speed_mps: { min: 4.5, max: 6.5 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
            lateral_accel_max_mps2: { min: 1.9, max: 1.9 },
            lateral_clearance_m: { min: 0.3, max: 0.3 },
            compliance: { min: 0.8, max: 1.0 },
        },
        lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
    } ],
    facilities: [
        { id: 'east', region: 'east_band', reference_path: 'east_path', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', lateral_policy: { passing_side: 'left' },
            speed_policy: { limit_mps: null } },
        { id: 'west', region: 'west_band', reference_path: 'west_path', width_m: 3.5,
            nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_adjacencies: [
        { id: 'lanes', first: 'east', second: 'west', side: 'right' },
    ],
    movements: [
        { id: 'cross_movement', from: 'cross_south', to: 'cross_north', path: 'cross_path',
            priority: 0, direction: 'forward' },
    ],
    crossings: [
        { id: 'crossing_north', region: 'crossing_zone', movements: [ 'cross_movement' ] },
    ],
    permissions: [
        { id: 'bike_contraflow', kind: 'nominal_direction', holder: 'bicycle', target: 'east',
            effect: 'permit' },
        { id: 'bike_may_overtake', kind: 'overtake', holder: 'bicycle', target: 'east',
            effect: 'permit' },
        { id: 'bike_holds_left', kind: 'lane_use', holder: 'bicycle', target: 'east',
            effect: 'obligate' },
        { id: 'bike_may_cross', kind: 'crossing', holder: 'bicycle', target: 'crossing_north',
            effect: 'permit' },
    ],
    clearance_bands: [
        { id: 'narrow_close_pass', threshold_m: 0.75, violation: true,
            applies_to_modes: [ 'bicycle' ] },
        { id: 'study_band', threshold_m: 1.5, violation: false },
    ],
    maneuver_policy: {
        commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
        wrong_way: { min_time_saving_s: 10.0, max_opposing_density_per_km: 40.0,
            urgency: 0.5 },
    },
}"#;

/// A two-facility bikeway whose `west` band is reachable only forward, so no
/// reverse traversal is physically possible on it. The mode declares
/// `reverse_direction`, so the document authors a wrong-way policy.
const FORWARD_ONLY: &str = r#"{
    schema_version: 2,
    id: 'forward_only',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'west_path', points: [ { x: 0.0, y: 0.0 }, { x: 100.0, y: 0.0 } ] },
        { id: 'east_path', points: [ { x: 100.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] },
    ],
    portals: [],
    regions: [
        { id: 'west_band', points: [
            { x: 0.0, y: -1.5 }, { x: 100.0, y: -1.5 },
            { x: 100.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
        { id: 'east_band', points: [
            { x: 100.0, y: -1.5 }, { x: 200.0, y: -1.5 },
            { x: 200.0, y: 1.5 }, { x: 100.0, y: 1.5 } ] },
    ],
    mode_templates: [ {
        id: 'cycle',
        body: { kind: 'box', length_m: { min: 1.6, max: 1.6 },
            width_m: { min: 0.7, max: 0.7 } },
        motion: 'single_body_wheeled',
        tactics: [ 'follow', 'stop', 'yield', 'reverse_direction' ],
        access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
        occupancy: 'operator_only',
        profiles: {
            speed_mps: { min: 4.0, max: 4.0 },
            max_accel_mps2: { min: 1.0, max: 1.0 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            compliance: { min: 1.0, max: 1.0 },
        },
    } ],
    facilities: [
        { id: 'west', region: 'west_band', reference_path: 'west_path', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
        { id: 'east', region: 'east_band', reference_path: 'east_path', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_connectors: [
        { id: 'west_to_east', from: { facility: 'west', direction: 'forward' },
            to: { facility: 'east', direction: 'forward' } },
    ],
    permissions: [
        { id: 'cycle_may_oppose', kind: 'nominal_direction', holder: 'cycle', target: 'west',
            effect: 'permit' },
    ],
    maneuver_policy: {
        wrong_way: { min_time_saving_s: 10.0, max_opposing_density_per_km: 40.0,
            urgency: 0.5 },
    },
}"#;

fn base() -> ScenarioSourceV2 {
    parse_scenario_source_v2(INCREMENT_2).expect("the base document parses")
}

fn codes(source: &ScenarioSourceV2) -> Vec<DiagnosticCode> {
    validate_v2(source).iter().map(|d| d.code).collect()
}

/// A mutation applied to the base document; a `fn` so the rejecting table can
/// be a `const`.
type Mutation = fn(&mut ScenarioSourceV2);

fn codes_of(mutate: Mutation) -> Vec<DiagnosticCode> {
    let mut source = base();
    mutate(&mut source);
    codes(&source)
}

#[test]
fn base_document_is_valid() {
    let source = base();
    assert_eq!(
        validate_v2(&source),
        Vec::new(),
        "the base Increment 2 document must validate cleanly"
    );
}

/// Every rejecting rule, its stable code, and the one authored fact that breaks
/// it. A value the JSON5 surface cannot spell (NaN) is set directly on the
/// parsed source, and the validator must stay total on it.
const REJECTIONS: &[(&str, DiagnosticCode, Mutation)] = &[
    (
        "lateral horizon zero",
        DiagnosticCode::ModeLateralHorizon,
        |s| {
            s.mode_templates[0].lateral.as_mut().unwrap().horizon_s = 0.0;
        },
    ),
    (
        "lateral horizon NaN",
        DiagnosticCode::ModeLateralHorizon,
        |s| {
            s.mode_templates[0].lateral.as_mut().unwrap().horizon_s = f64::NAN;
        },
    ),
    (
        "lateral clearance negative",
        DiagnosticCode::ModeLateralClearance,
        |s| {
            s.mode_templates[0]
                .lateral
                .as_mut()
                .unwrap()
                .target_clearance_m = -0.1;
        },
    ),
    (
        "lateral clearance NaN",
        DiagnosticCode::ModeLateralClearance,
        |s| {
            s.mode_templates[0]
                .lateral
                .as_mut()
                .unwrap()
                .target_clearance_m = f64::NAN;
        },
    ),
    (
        "lateral without a lateral tactic",
        DiagnosticCode::ModeTemplateLateral,
        |s| {
            s.mode_templates[0].tactics =
                vec![TacticKind::Follow, TacticKind::Stop, TacticKind::Yield];
        },
    ),
    (
        "lateral on a walking template",
        DiagnosticCode::ModeTemplateLateral,
        |s| {
            s.mode_templates[0].motion = tangle_model::MotionKind::HolonomicWalking;
        },
    ),
    (
        "lateral tactic without maneuver policy",
        DiagnosticCode::ManeuverPolicyMissing,
        |s| {
            s.maneuver_policy = None;
        },
    ),
    (
        "reverse_direction without a wrong-way policy",
        DiagnosticCode::WrongWayPolicyMissing,
        |s| {
            s.maneuver_policy.as_mut().unwrap().wrong_way = None;
        },
    ),
    (
        "lateral mode missing a required lateral profile",
        DiagnosticCode::ModeTemplateProfile,
        |s| {
            s.mode_templates[0]
                .profiles
                .remove("lateral_accel_max_mps2");
        },
    ),
    (
        "commit clearance floor negative",
        DiagnosticCode::CommitPolicyInvalid,
        |s| {
            s.maneuver_policy
                .as_mut()
                .unwrap()
                .commit
                .as_mut()
                .unwrap()
                .min_predicted_clearance_m = -0.1;
        },
    ),
    (
        "commit hold timeout zero",
        DiagnosticCode::CommitPolicyInvalid,
        |s| {
            s.maneuver_policy
                .as_mut()
                .unwrap()
                .commit
                .as_mut()
                .unwrap()
                .hold_timeout_s = 0.0;
        },
    ),
    (
        "commit floor above the target clearance",
        DiagnosticCode::CommitClearanceExceedsTarget,
        |s| {
            s.maneuver_policy
                .as_mut()
                .unwrap()
                .commit
                .as_mut()
                .unwrap()
                .min_predicted_clearance_m = 1.0;
        },
    ),
    (
        "wrong-way urgency above one",
        DiagnosticCode::WrongWayPolicyInvalid,
        |s| {
            s.maneuver_policy
                .as_mut()
                .unwrap()
                .wrong_way
                .as_mut()
                .unwrap()
                .urgency = 1.5;
        },
    ),
    (
        "wrong-way density negative",
        DiagnosticCode::WrongWayPolicyInvalid,
        |s| {
            s.maneuver_policy
                .as_mut()
                .unwrap()
                .wrong_way
                .as_mut()
                .unwrap()
                .max_opposing_density_per_km = -1.0;
        },
    ),
    (
        "clearance band threshold zero",
        DiagnosticCode::ClearanceBandThreshold,
        |s| {
            s.clearance_bands[0].threshold_m = 0.0;
        },
    ),
    (
        "clearance band threshold NaN",
        DiagnosticCode::ClearanceBandThreshold,
        |s| {
            s.clearance_bands[0].threshold_m = f64::NAN;
        },
    ),
    (
        "clearance bands out of order",
        DiagnosticCode::ClearanceBandOrder,
        |s| {
            s.clearance_bands[1].threshold_m = s.clearance_bands[0].threshold_m;
        },
    ),
    (
        "clearance band empty mode list",
        DiagnosticCode::ClearanceBandModesEmpty,
        |s| {
            s.clearance_bands[0].applies_to_modes = Some(Vec::new());
        },
    ),
    (
        "clearance band unknown mode",
        DiagnosticCode::ClearanceBandUnknownMode,
        |s| {
            s.clearance_bands[0].applies_to_modes = Some(vec!["ghost".to_owned()]);
        },
    ),
    (
        "lateral policy without a reference path",
        DiagnosticCode::FacilityLateralWithoutReference,
        |s| {
            s.facilities[0].reference_path = None;
        },
    ),
    (
        "centered facility with a lateral policy",
        DiagnosticCode::FacilityLateralCentered,
        |s| {
            s.facilities[0].lateral_use = LateralUse::Centered;
        },
    ),
    (
        "passing side no body can occupy",
        DiagnosticCode::FacilityPassingSideUnusable,
        |s| {
            s.facilities[0].width_m = 1.2;
        },
    ),
    (
        "adjacency to an undeclared facility",
        DiagnosticCode::FacilityAdjacencyUnknownFacility,
        |s| {
            s.facility_adjacencies[0].second = "ghost".to_owned();
        },
    ),
    (
        "adjacency joining a facility to itself",
        DiagnosticCode::FacilityAdjacencySelf,
        |s| {
            s.facility_adjacencies[0].second = s.facility_adjacencies[0].first.clone();
        },
    ),
    (
        "adjacency without a reference path",
        DiagnosticCode::FacilityAdjacencyWithoutReference,
        |s| {
            s.facilities[1].reference_path = None;
        },
    ),
    (
        "adjacency across disjoint bands",
        DiagnosticCode::FacilityAdjacencyDisjoint,
        |s| {
            for point in &mut s.regions[1].points {
                point.y += 100.0;
            }
        },
    ),
    (
        "adjacency on the wrong side",
        DiagnosticCode::FacilityAdjacencySide,
        |s| {
            s.facility_adjacencies[0].side = AdjacencySide::Left;
        },
    ),
    (
        "permission kind disagrees with a movement target",
        DiagnosticCode::PermissionTargetKind,
        |s| {
            s.permissions[2].target = "cross_movement".to_owned();
        },
    ),
    (
        "permission kind disagrees with a facility target",
        DiagnosticCode::PermissionTargetKind,
        |s| {
            s.permissions[3].target = "east".to_owned();
        },
    ),
    (
        "permission repeats a specificity with another effect",
        DiagnosticCode::PermissionEffectConflict,
        |s| {
            let mut duplicate = s.permissions[0].clone();
            duplicate.id = "duplicate".to_owned();
            duplicate.effect = PermissionEffect::Prohibit;
            s.permissions.push(duplicate);
        },
    ),
    (
        "permission repeats a specificity identically",
        DiagnosticCode::PermissionEffectConflict,
        |s| {
            let mut duplicate = s.permissions[1].clone();
            duplicate.id = "duplicate".to_owned();
            s.permissions.push(duplicate);
        },
    ),
    (
        "overtake permission without a capable holder",
        DiagnosticCode::PermissionOvertakeCapability,
        |s| {
            s.mode_templates[0]
                .tactics
                .retain(|tactic| *tactic != TacticKind::Overtake);
        },
    ),
    (
        "lane-use obligation without a lateral policy",
        DiagnosticCode::PermissionLaneUseObligation,
        |s| {
            s.facilities[0].lateral_policy = None;
        },
    ),
    (
        "lane-use obligation on most_clearance",
        DiagnosticCode::PermissionLaneUseObligation,
        |s| {
            s.facilities[0]
                .lateral_policy
                .as_mut()
                .unwrap()
                .passing_side = PassingSide::MostClearance;
        },
    ),
    (
        "nominal statement about an either facility",
        DiagnosticCode::PermissionNominalEither,
        |s| {
            s.facilities[0].nominal_direction = FacilityDirection::Either;
        },
    ),
    (
        "permission holder undeclared",
        DiagnosticCode::PermissionUnknownHolder,
        |s| {
            s.permissions[1].holder = "ghost".to_owned();
        },
    ),
    (
        "permission target undeclared",
        DiagnosticCode::PermissionUnknownTarget,
        |s| {
            s.permissions[1].target = "ghost".to_owned();
        },
    ),
];

#[test]
fn rejects_each_malformed_increment_2_policy() {
    for (name, code, mutate) in REJECTIONS {
        let found = codes_of(*mutate);
        assert!(
            found.contains(code),
            "{name}: expected {} ({code:?}), got {found:?}",
            code.as_str()
        );
    }
}

#[test]
fn rejects_a_nominal_statement_that_targets_a_crossing() {
    // A nominal-direction statement cannot name a crossing; the kind fixes the
    // object kind, so the target is a kind mismatch rather than an unknown id.
    let mut source = base();
    source.permissions[0].target = "crossing_north".to_owned();
    let found = codes(&source);
    assert!(found.contains(&DiagnosticCode::PermissionTargetKind));
    assert!(!found.contains(&DiagnosticCode::PermissionUnknownTarget));
}

#[test]
fn legal_prohibition_stays_distinct_from_physical_impossibility() {
    // A prohibit that contradicts a facility's granted access is an illegal
    // route, not a physically impossible one.
    let mut source = base();
    source.permissions[0].effect = PermissionEffect::Prohibit;
    let found = codes(&source);
    assert!(found.contains(&DiagnosticCode::PermissionRouteProhibited));
    assert!(!found.contains(&DiagnosticCode::FacilityTooNarrow));
    assert!(!found.contains(&DiagnosticCode::FacilityCurvature));
    assert!(!found.contains(&DiagnosticCode::FacilityUnreachableDirection));
}

#[test]
fn malformed_structural_inputs_fail_to_parse_not_panic() {
    // No rule falls back to a parser or a Phase 1 default: a structurally
    // malformed document is a parse error, and the validator never sees it.
    for malformed in [
        INCREMENT_2.replace("threshold_m: 0.75,", "threshold_m: 'close',"),
        INCREMENT_2.replace("facility_adjacencies: [", "facility_adjacencies: {"),
        INCREMENT_2.replace("passing_side: 'left'", "passing_side: 'west'"),
        INCREMENT_2.replace("urgency: 0.5", "urgency: 'half'"),
    ] {
        assert_ne!(malformed, INCREMENT_2, "the fixture must be rewritten");
        assert!(
            parse_scenario_source_v2(&malformed).is_err(),
            "a malformed document must fail to parse"
        );
    }
}

/// [`FORWARD_ONLY`] with the reverse connector added, so both directions are
/// physically possible on `west`.
fn both_ways() -> ScenarioSourceV2 {
    let with_reverse = FORWARD_ONLY.replace(
        "facility_connectors: [",
        "facility_connectors: [ { id: 'east_to_west', \
         from: { facility: 'east', direction: 'reverse' }, \
         to: { facility: 'west', direction: 'reverse' } },",
    );
    assert_ne!(with_reverse, FORWARD_ONLY, "the fixture must be rewritten");
    parse_scenario_source_v2(&with_reverse).expect("the two-way document parses")
}

/// Compile the document and return the traversal policy of `cycle` on `west`.
fn cycle_on_west(source: ScenarioSourceV2) -> tangle_model::FacilityTraversalPolicy {
    let scenario = CompiledScenario::compile_v2(source).expect("the document compiles");
    scenario
        .traversal_policy(
            ModeTemplateId::from_index(0),
            FacilityId::from_index(0),
            None,
        )
        .expect("cycle is eligible on west")
}

#[test]
fn an_unconnected_opposing_statement_is_inert_for_the_traversal() {
    // `west` is connected only forward, so no opposing traversal exists. The
    // statement validates cleanly (it is not rejected) and never widens the
    // permitted set past what the topology connects; the runtime wrong-way
    // decision closes the case with its `no_opposing_path` reason.
    let forward = parse_scenario_source_v2(FORWARD_ONLY).expect("the document parses");
    assert_eq!(
        validate_v2(&forward),
        Vec::new(),
        "an unconnected opposing statement is inert, not a validation failure"
    );

    for effect in [PermissionEffect::Permit, PermissionEffect::Obligate] {
        let mut source = parse_scenario_source_v2(FORWARD_ONLY).expect("the document parses");
        source.permissions[0].effect = effect;
        assert_eq!(validate_v2(&source), Vec::new());
        let policy = cycle_on_west(source);
        assert_eq!(
            policy.physically_possible_directions(),
            DirectionSet::FORWARD
        );
        assert_eq!(policy.permitted_directions(), DirectionSet::FORWARD);
        assert!(
            policy
                .permitted_directions()
                .is_subset_of(policy.physically_possible_directions()),
            "a permission never widens physical possibility"
        );
        assert!(!policy.permits(MovementDirection::Reverse));
    }
}

#[test]
fn a_connected_opposing_statement_widens_the_permitted_direction() {
    // With the reverse connector present the opposing traversal is physically
    // possible, so the statement stops being inert: a `permit` adds it and an
    // `obligate` leaves only it.
    let permitted = parse_scenario_source_v2(FORWARD_ONLY).expect("the document parses");
    let policy = cycle_on_west(permitted);
    assert_eq!(
        policy.physically_possible_directions(),
        DirectionSet::FORWARD
    );

    let mut both = both_ways();
    both.permissions[0].effect = PermissionEffect::Permit;
    let policy = cycle_on_west(both);
    assert_eq!(policy.physically_possible_directions(), DirectionSet::BOTH);
    assert_eq!(policy.permitted_directions(), DirectionSet::BOTH);

    let mut both = both_ways();
    both.permissions[0].effect = PermissionEffect::Obligate;
    let policy = cycle_on_west(both);
    assert_eq!(policy.permitted_directions(), DirectionSet::REVERSE);
    assert_eq!(policy.traversable_directions(), DirectionSet::REVERSE);
}

#[test]
fn a_connected_but_unpermitted_traversal_stays_physically_possible() {
    // Absent a statement, only the nominal direction is permitted, but a
    // connected reverse traversal stays in the physically possible set: it is
    // the violation context a non-compliant decision may take, and it is never
    // silently dropped.
    let mut source = both_ways();
    source.permissions.clear();
    assert_eq!(validate_v2(&source), Vec::new());
    let policy = cycle_on_west(source);
    assert_eq!(policy.permitted_directions(), DirectionSet::FORWARD);
    assert_eq!(policy.physically_possible_directions(), DirectionSet::BOTH);
    assert!(
        policy
            .physically_possible_directions()
            .contains(MovementDirection::Reverse)
    );
    assert!(!policy.permits(MovementDirection::Reverse));
}

#[test]
fn increment_2_diagnostic_codes_have_stable_strings() {
    for (code, text) in [
        (DiagnosticCode::ModeLateralHorizon, "E_MODE_LATERAL_HORIZON"),
        (
            DiagnosticCode::ModeLateralClearance,
            "E_MODE_LATERAL_CLEARANCE",
        ),
        (
            DiagnosticCode::ModeTemplateLateral,
            "E_MODE_TEMPLATE_LATERAL",
        ),
        (
            DiagnosticCode::ManeuverPolicyMissing,
            "E_MANEUVER_POLICY_MISSING",
        ),
        (
            DiagnosticCode::WrongWayPolicyMissing,
            "E_WRONG_WAY_POLICY_MISSING",
        ),
        (DiagnosticCode::CommitPolicyInvalid, "E_COMMIT_POLICY"),
        (
            DiagnosticCode::CommitClearanceExceedsTarget,
            "E_COMMIT_CLEARANCE",
        ),
        (DiagnosticCode::WrongWayPolicyInvalid, "E_WRONG_WAY_POLICY"),
        (
            DiagnosticCode::ClearanceBandThreshold,
            "E_CLEARANCE_BAND_THRESHOLD",
        ),
        (DiagnosticCode::ClearanceBandOrder, "E_CLEARANCE_BAND_ORDER"),
        (
            DiagnosticCode::ClearanceBandModesEmpty,
            "E_CLEARANCE_BAND_MODES_EMPTY",
        ),
        (
            DiagnosticCode::ClearanceBandUnknownMode,
            "E_CLEARANCE_BAND_UNKNOWN_MODE",
        ),
        (
            DiagnosticCode::FacilityLateralWithoutReference,
            "E_FACILITY_LATERAL_WITHOUT_REFERENCE",
        ),
        (
            DiagnosticCode::FacilityLateralCentered,
            "E_FACILITY_LATERAL_CENTERED",
        ),
        (
            DiagnosticCode::FacilityPassingSideUnusable,
            "E_FACILITY_PASSING_SIDE_UNUSABLE",
        ),
        (
            DiagnosticCode::FacilityAdjacencyUnknownFacility,
            "E_FACILITY_ADJACENCY_UNKNOWN_FACILITY",
        ),
        (
            DiagnosticCode::FacilityAdjacencySelf,
            "E_FACILITY_ADJACENCY_SELF",
        ),
        (
            DiagnosticCode::FacilityAdjacencyWithoutReference,
            "E_FACILITY_ADJACENCY_WITHOUT_REFERENCE",
        ),
        (
            DiagnosticCode::FacilityAdjacencyDisjoint,
            "E_FACILITY_ADJACENCY_DISJOINT",
        ),
        (
            DiagnosticCode::FacilityAdjacencySide,
            "E_FACILITY_ADJACENCY_SIDE",
        ),
        (
            DiagnosticCode::PermissionTargetKind,
            "E_PERMISSION_TARGET_KIND",
        ),
        (
            DiagnosticCode::PermissionEffectConflict,
            "E_PERMISSION_EFFECT_CONFLICT",
        ),
        (
            DiagnosticCode::PermissionOvertakeCapability,
            "E_PERMISSION_OVERTAKE_CAPABILITY",
        ),
        (
            DiagnosticCode::PermissionLaneUseObligation,
            "E_PERMISSION_LANE_USE_OBLIGATION",
        ),
        (
            DiagnosticCode::PermissionNominalEither,
            "E_PERMISSION_NOMINAL_EITHER",
        ),
    ] {
        assert_eq!(code.as_str(), text);
    }
}
