//! Increment 2 compiled policy and traversal semantics.
//!
//! The compiled contract is fixed by `docs/schema-v2-contract.md` under
//! *Increment 2 additions: lateral motion, passing, and wrong-way travel* →
//! *Compiled semantics*. These tests cover the resolution the kernel reads:
//! mode-template, facility, connector, permission, and clearance-band ids
//! resolved once in stable source order; the three separate direction
//! properties; the usable lateral interval of an eligible body; the lateral and
//! longitudinal transition targets; and the Increment 1 behaviour a document
//! authoring no Increment 2 policy compiles to.
//!
//! Validation of the same shapes is a separate leaf (TAS-086), so every fixture
//! here authors policy `validate_v2` already accepts.

use glam::DVec2;
use tangle_model::{
    AdjacencySide, ClearanceBandId, CompiledModeTemplate, CompiledScenario, CrossingId,
    DiagnosticCode, DirectionSet, FacilityAdjacencyId, FacilityConnectorId, FacilityId, LateralUse,
    ModeTemplateId, MovementDirection, MovementId, PassingSide, PermissionEffect, PermissionId,
    PermissionKind, PermissionTarget, compile_mode_template, parse_scenario_source,
    parse_scenario_source_v2,
};

/// A version-2 document whose facilities cover a forward, a reverse, and an
/// either nominal direction, an adjacent pair of bands, a centered facility, a
/// facility only an adjacency connects, and a facility no adjacency reaches.
///
/// The layout, in metres:
///
/// - `bikeway_eastbound` band `y ∈ [-1.5, 1.5]` on a reference authored `+x`,
///   adjacent on its right to `bikeway_westbound`;
/// - `bikeway_westbound` band `y ∈ [-4.5, -1.5]` on a reference authored `-x`,
///   so the two bands face opposite world directions;
/// - `bikeway_east_exit` continues the eastbound facility across two directed
///   connectors, which is what makes both traversal directions physically
///   possible there;
/// - `bikeway_turnaround` is an `either` facility beside the exit band but
///   joined to nothing: proximity is never inferred into a transition;
/// - `road_eastbound` is centered, has a `reverse` nominal direction, and is
///   left only by a single reverse connector, so exactly one direction is
///   physically possible;
/// - `road_shoulder` lies beside the road with no connector of its own, so its
///   one connected direction comes from the adjacency alone;
/// - `road_out` carries that connector's other end.
const INCREMENT_2: &str = r#"{
    schema_version: 2,
    id: 'increment_2_compiled',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'bikeway_east', points: [ { x: 0.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
        { id: 'bikeway_west', points: [ { x: 120.0, y: -3.0 }, { x: 0.0, y: -3.0 } ] },
        { id: 'bikeway_exit', points: [ { x: 120.0, y: 0.0 }, { x: 240.0, y: 0.0 } ] },
        { id: 'turnaround_guide', points: [ { x: 240.0, y: 0.0 }, { x: 250.0, y: 0.0 } ] },
        { id: 'road_guide', points: [ { x: 0.0, y: 6.0 }, { x: 120.0, y: 6.0 } ] },
        { id: 'cross_guide', points: [ { x: 60.0, y: -10.0 }, { x: 60.0, y: 10.0 } ] },
        { id: 'approach_guide', points: [ { x: -120.0, y: 0.0 }, { x: 0.0, y: 0.0 } ] },
        { id: 'shoulder_guide', points: [ { x: 0.0, y: 9.0 }, { x: 120.0, y: 9.0 } ] },
        { id: 'road_out_guide', points: [ { x: -120.0, y: 6.0 }, { x: 0.0, y: 6.0 } ] },
    ],
    portals: [
        { id: 'cross_south', path: 'cross_guide', end: 'start', width_m: 2.0 },
        { id: 'cross_north', path: 'cross_guide', end: 'end', width_m: 2.0 },
    ],
    regions: [
        { id: 'east_band', points: [
            { x: 0.0, y: -1.5 }, { x: 120.0, y: -1.5 },
            { x: 120.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] },
        { id: 'west_band', points: [
            { x: 0.0, y: -4.5 }, { x: 120.0, y: -4.5 },
            { x: 120.0, y: -1.5 }, { x: 0.0, y: -1.5 } ] },
        { id: 'exit_band', points: [
            { x: 120.0, y: -1.5 }, { x: 240.0, y: -1.5 },
            { x: 240.0, y: 1.5 }, { x: 120.0, y: 1.5 } ] },
        { id: 'turnaround_band', points: [
            { x: 240.0, y: -1.0 }, { x: 250.0, y: -1.0 },
            { x: 250.0, y: 1.0 }, { x: 240.0, y: 1.0 } ] },
        { id: 'road_band', points: [
            { x: 0.0, y: 4.5 }, { x: 120.0, y: 4.5 },
            { x: 120.0, y: 7.5 }, { x: 0.0, y: 7.5 } ] },
        { id: 'crossing_zone', points: [
            { x: 57.0, y: -3.0 }, { x: 63.0, y: -3.0 },
            { x: 63.0, y: 3.0 }, { x: 57.0, y: 3.0 } ] },
        { id: 'approach_band', points: [
            { x: -120.0, y: -1.5 }, { x: 0.0, y: -1.5 },
            { x: 0.0, y: 1.5 }, { x: -120.0, y: 1.5 } ] },
        { id: 'shoulder_band', points: [
            { x: 0.0, y: 7.5 }, { x: 120.0, y: 7.5 },
            { x: 120.0, y: 10.5 }, { x: 0.0, y: 10.5 } ] },
        { id: 'road_out_band', points: [
            { x: -120.0, y: 4.5 }, { x: 0.0, y: 4.5 },
            { x: 0.0, y: 7.5 }, { x: -120.0, y: 7.5 } ] },
    ],
    mode_templates: [
        {
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
        },
        {
            id: 'scooter',
            body: { kind: 'capsule', length_m: { min: 1.4, max: 1.4 },
                radius_m: { min: 0.35, max: 0.35 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'pass', 'overtake' ],
            access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
                speed_policy: { limit_mps: null } },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 3.0, max: 5.0 },
                max_accel_mps2: { min: 1.0, max: 1.0 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                steering_rate_max_rad_s: { min: 0.8, max: 0.8 },
                lateral_accel_max_mps2: { min: 1.6, max: 1.6 },
                lateral_clearance_m: { min: 0.2, max: 0.2 },
                compliance: { min: 0.9, max: 1.0 },
            },
            lateral: { target_clearance_m: 0.4, horizon_s: 2.5 },
        },
        {
            id: 'passenger_car',
            body: { kind: 'box', length_m: { min: 4.0, max: 5.2 },
                width_m: { min: 1.7, max: 2.0 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility', 'path' ], nominal_direction: 'either',
                speed_policy: { limit_mps: 12.0 } },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 9.0, max: 15.0 },
                max_accel_mps2: { min: 1.2, max: 2.5 },
                comfortable_brake_mps2: { min: 2.0, max: 3.5 },
                time_gap_s: { min: 1.0, max: 2.0 },
                compliance: { min: 0.9, max: 1.0 },
            },
        },
    ],
    facilities: [
        { id: 'bikeway_eastbound', region: 'east_band', reference_path: 'bikeway_east',
            width_m: 3.0, nominal_direction: 'forward',
            access: { modes: [ 'bicycle', 'scooter' ] }, lateral_use: 'shared',
            lateral_policy: { passing_side: 'left' },
            speed_policy: { limit_mps: null } },
        { id: 'bikeway_westbound', region: 'west_band', reference_path: 'bikeway_west',
            width_m: 3.0, nominal_direction: 'forward', access: { modes: [ 'bicycle' ] },
            lateral_use: 'shared', lateral_policy: { passing_side: 'left' },
            speed_policy: { limit_mps: null } },
        { id: 'bikeway_east_exit', region: 'exit_band', reference_path: 'bikeway_exit',
            width_m: 3.0, nominal_direction: 'forward',
            access: { modes: [ 'bicycle', 'scooter' ] }, lateral_use: 'shared',
            speed_policy: { limit_mps: null } },
        { id: 'bikeway_turnaround', region: 'turnaround_band',
            reference_path: 'turnaround_guide', width_m: 2.0, nominal_direction: 'either',
            access: { modes: [ 'bicycle' ] }, lateral_use: 'shared',
            speed_policy: { limit_mps: null } },
        { id: 'road_eastbound', region: 'road_band', reference_path: 'road_guide',
            width_m: 3.5, nominal_direction: 'reverse', access: { modes: [ 'passenger_car' ] },
            lateral_use: 'centered', speed_policy: { limit_mps: 8.0 } },
        { id: 'bikeway_approach', region: 'approach_band', reference_path: 'approach_guide',
            width_m: 3.0, nominal_direction: 'forward',
            access: { modes: [ 'bicycle', 'scooter' ] }, lateral_use: 'shared',
            speed_policy: { limit_mps: null } },
        { id: 'road_shoulder', region: 'shoulder_band', reference_path: 'shoulder_guide',
            width_m: 3.0, nominal_direction: 'either',
            access: { modes: [ 'bicycle', 'passenger_car' ] }, lateral_use: 'shared',
            speed_policy: { limit_mps: null } },
        { id: 'road_out', region: 'road_out_band', reference_path: 'road_out_guide',
            width_m: 3.5, nominal_direction: 'either', access: { modes: [ 'passenger_car' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_connectors: [
        { id: 'bikeway_to_exit', from: { facility: 'bikeway_eastbound', direction: 'forward' },
            to: { facility: 'bikeway_east_exit', direction: 'forward' } },
        { id: 'exit_to_bikeway', from: { facility: 'bikeway_east_exit', direction: 'reverse' },
            to: { facility: 'bikeway_eastbound', direction: 'reverse' } },
        { id: 'approach_to_bikeway', from: { facility: 'bikeway_approach', direction: 'forward' },
            to: { facility: 'bikeway_eastbound', direction: 'forward' } },
        { id: 'bikeway_to_approach', from: { facility: 'bikeway_eastbound', direction: 'reverse' },
            to: { facility: 'bikeway_approach', direction: 'reverse' } },
        { id: 'road_out_to_extension', from: { facility: 'road_eastbound', direction: 'reverse' },
            to: { facility: 'road_out', direction: 'reverse' } },
    ],
    facility_adjacencies: [
        { id: 'bikeway_lanes', first: 'bikeway_eastbound', second: 'bikeway_westbound',
            side: 'right' },
        { id: 'road_side', first: 'road_shoulder', second: 'road_eastbound', side: 'right' },
    ],
    movements: [
        { id: 'cross_movement', from: 'cross_south', to: 'cross_north', path: 'cross_guide',
            priority: 0, direction: 'forward' },
    ],
    crossings: [
        { id: 'crossing_north', region: 'crossing_zone', movements: [ 'cross_movement' ] },
    ],
    permissions: [
        { id: 'bicycle_may_use_contraflow', kind: 'nominal_direction', holder: 'bicycle',
            target: 'bikeway_eastbound', effect: 'permit' },
        { id: 'scooter_must_use_contraflow', kind: 'nominal_direction', holder: 'scooter',
            target: 'bikeway_eastbound', effect: 'obligate' },
        { id: 'bicycle_holds_nominal_on_cross', kind: 'nominal_direction', holder: 'bicycle',
            target: 'cross_movement', effect: 'prohibit' },
        { id: 'bicycle_holds_outside', kind: 'lane_use', holder: 'bicycle',
            target: 'bikeway_eastbound', effect: 'obligate' },
        { id: 'scooter_may_not_pass', kind: 'overtake', holder: 'scooter',
            target: 'bikeway_east_exit', effect: 'prohibit' },
        { id: 'bicycle_may_cross', kind: 'crossing', holder: 'bicycle',
            target: 'crossing_north', effect: 'permit' },
        { id: 'bicycle_serves_stop', kind: 'stop_service', holder: 'bicycle',
            target: 'bus_stop_1', effect: 'obligate' },
    ],
    clearance_bands: [
        { id: 'narrow_close_pass', threshold_m: 0.75, violation: true,
            applies_to_modes: [ 'bicycle' ] },
        { id: 'motor_close_pass', threshold_m: 1.0, violation: true,
            applies_to_modes: [ 'scooter' ] },
        { id: 'study_band', threshold_m: 1.5, violation: false },
    ],
    maneuver_policy: {
        commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
        wrong_way: { min_time_saving_s: 10.0, max_opposing_density_per_km: 40.0,
            urgency: 0.5 },
    },
}"#;

/// A version-2 document whose two side-by-side bands follow a curved pair of
/// reference paths and share a curved boundary.
///
/// Each band is a half-annulus approximated by chords: `inner_band` spans
/// radius 3–4 m and `outer_band` radius 4–5 m, so the two share the mid arc at
/// radius 4 m. Their references are the concentric chordal polylines at radius
/// 3.5 m and 4.5 m. The longest collinear segment the bands share is one mid
/// arc chord, whose midpoint is not the straight `width_m / 2` offset a runtime
/// half-width approximation would report.
const CURVED: &str = r#"{
    schema_version: 2,
    id: 'curved_pair_compiled',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'inner_curve', points: [
            { x: 3.5, y: 0.0 }, { x: 2.4749, y: 2.4749 }, { x: 0.0, y: 3.5 },
            { x: -2.4749, y: 2.4749 }, { x: -3.5, y: 0.0 } ] },
        { id: 'outer_curve', points: [
            { x: 4.5, y: 0.0 }, { x: 3.1820, y: 3.1820 }, { x: 0.0, y: 4.5 },
            { x: -3.1820, y: 3.1820 }, { x: -4.5, y: 0.0 } ] },
    ],
    portals: [],
    regions: [
        { id: 'inner_band', points: [
            { x: 3.0, y: 0.0 }, { x: 2.1213, y: 2.1213 }, { x: 0.0, y: 3.0 },
            { x: -2.1213, y: 2.1213 }, { x: -3.0, y: 0.0 },
            { x: -4.0, y: 0.0 }, { x: -2.8284, y: 2.8284 }, { x: 0.0, y: 4.0 },
            { x: 2.8284, y: 2.8284 }, { x: 4.0, y: 0.0 } ] },
        { id: 'outer_band', points: [
            { x: 4.0, y: 0.0 }, { x: 2.8284, y: 2.8284 }, { x: 0.0, y: 4.0 },
            { x: -2.8284, y: 2.8284 }, { x: -4.0, y: 0.0 },
            { x: -5.0, y: 0.0 }, { x: -3.5355, y: 3.5355 }, { x: 0.0, y: 5.0 },
            { x: 3.5355, y: 3.5355 }, { x: 5.0, y: 0.0 } ] },
    ],
    mode_templates: [
        {
            id: 'cycle',
            body: { kind: 'capsule', length_m: { min: 1.6, max: 1.6 },
                radius_m: { min: 0.3, max: 0.3 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility' ] },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 4.0, max: 4.0 },
                max_accel_mps2: { min: 1.0, max: 1.0 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                lateral_clearance_m: { min: 0.1, max: 0.1 },
                compliance: { min: 1.0, max: 1.0 },
            },
        },
    ],
    facilities: [
        { id: 'inner_lane', region: 'inner_band', reference_path: 'inner_curve',
            width_m: 1.0, nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
        { id: 'outer_lane', region: 'outer_band', reference_path: 'outer_curve',
            width_m: 1.0, nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_adjacencies: [
        { id: 'curved_pair', first: 'inner_lane', second: 'outer_lane', side: 'right' },
    ],
}"#;

/// A version-2 document whose two bands touch only at the single corner
/// `(10, 10)`: a shared point is not a shared boundary of positive length, so
/// the adjacency yields no coordinate and validation rejects the document.
const CORNER: &str = r#"{
    schema_version: 2,
    id: 'corner_touch_compiled',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'left_guide', points: [ { x: 0.0, y: 5.0 }, { x: 10.0, y: 5.0 } ] },
        { id: 'right_guide', points: [ { x: 15.0, y: 10.0 }, { x: 15.0, y: 20.0 } ] },
    ],
    portals: [],
    regions: [
        { id: 'left_band', points: [
            { x: 0.0, y: 0.0 }, { x: 10.0, y: 0.0 },
            { x: 10.0, y: 10.0 }, { x: 0.0, y: 10.0 } ] },
        { id: 'right_band', points: [
            { x: 10.0, y: 10.0 }, { x: 20.0, y: 10.0 },
            { x: 20.0, y: 20.0 }, { x: 10.0, y: 20.0 } ] },
    ],
    mode_templates: [
        {
            id: 'cycle',
            body: { kind: 'capsule', length_m: { min: 1.6, max: 1.6 },
                radius_m: { min: 0.35, max: 0.35 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility' ] },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 4.0, max: 4.0 },
                max_accel_mps2: { min: 1.0, max: 1.0 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                lateral_clearance_m: { min: 0.3, max: 0.3 },
                compliance: { min: 1.0, max: 1.0 },
            },
        },
    ],
    facilities: [
        { id: 'left_lane', region: 'left_band', reference_path: 'left_guide',
            width_m: 3.0, nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
        { id: 'right_lane', region: 'right_band', reference_path: 'right_guide',
            width_m: 3.0, nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_adjacencies: [
        { id: 'corner_only', first: 'left_lane', second: 'right_lane', side: 'left' },
    ],
}"#;

/// A version-2 document whose Increment 2 policy names objects no fixture
/// declares: an adjacency to an undeclared facility, a band restricted to an
/// undeclared mode, and a lane-use statement about an undeclared facility.
/// `validate_v2` does not resolve these references yet, so the compiler must.
const DANGLING: &str = r#"{
    schema_version: 2,
    id: 'dangling_increment_2',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 20.0, y: 0.0 } ] } ],
    portals: [],
    regions: [ { id: 'band', points: [
        { x: 0.0, y: -1.5 }, { x: 20.0, y: -1.5 },
        { x: 20.0, y: 1.5 }, { x: 0.0, y: 1.5 } ] } ],
    mode_templates: [
        {
            id: 'cycle',
            body: { kind: 'capsule', length_m: { min: 1.6, max: 1.6 },
                radius_m: { min: 0.35, max: 0.35 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility' ] },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 4.0, max: 4.0 },
                max_accel_mps2: { min: 1.0, max: 1.0 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                lateral_clearance_m: { min: 0.3, max: 0.3 },
                compliance: { min: 1.0, max: 1.0 },
            },
        },
    ],
    facilities: [
        { id: 'lane', region: 'band', reference_path: 'guide', width_m: 3.0,
            nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_adjacencies: [
        { id: 'lane_to_nowhere', first: 'lane', second: 'missing_lane', side: 'left' },
    ],
    clearance_bands: [
        { id: 'only_band', threshold_m: 1.0, violation: false,
            applies_to_modes: [ 'missing_mode' ] },
    ],
    permissions: [
        { id: 'cycle_may_use_nowhere', kind: 'lane_use', holder: 'cycle',
            target: 'missing_lane', effect: 'permit' },
    ],
}"#;

/// A version-2 document that authors no Increment 2 policy: one capsule mode, a
/// one-way facility, and a forward connector, which is the Increment 1 shape.
const INCREMENT_1: &str = r#"{
    schema_version: 2,
    id: 'increment_1_compiled',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'west_centerline', points: [ { x: 0.0, y: 0.0 }, { x: 100.0, y: 0.0 } ] },
        { id: 'east_centerline', points: [ { x: 100.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] },
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
    mode_templates: [
        {
            id: 'cycle',
            body: { kind: 'capsule', length_m: { min: 1.6, max: 1.9 },
                radius_m: { min: 0.3, max: 0.4 } },
            motion: 'single_body_wheeled',
            tactics: [ 'follow', 'stop', 'yield' ],
            access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
                speed_policy: { limit_mps: null } },
            occupancy: 'operator_only',
            profiles: {
                speed_mps: { min: 3.5, max: 6.5 },
                max_accel_mps2: { min: 0.8, max: 1.5 },
                comfortable_brake_mps2: { min: 1.5, max: 3.0 },
                time_gap_s: { min: 0.8, max: 1.4 },
                steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                lateral_clearance_m: { min: 0.3, max: 0.3 },
                compliance: { min: 0.8, max: 1.0 },
            },
        },
    ],
    facilities: [
        { id: 'west_lane', region: 'west_band', reference_path: 'west_centerline',
            width_m: 3.0, nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: 8.0 } },
        { id: 'east_lane', region: 'east_band', reference_path: 'east_centerline',
            width_m: 3.0, nominal_direction: 'forward', access: { modes: [ 'cycle' ] },
            lateral_use: 'shared', speed_policy: { limit_mps: null } },
    ],
    facility_connectors: [
        { id: 'west_to_east', from: { facility: 'west_lane', direction: 'forward' },
            to: { facility: 'east_lane', direction: 'forward' } },
    ],
}"#;

/// A version-1 walking document, which has no version-2 objects at all.
const VERSION_1: &str = r#"{
    schema_version: 1,
    id: 'walking_guide_v1',
    coordinate_system: { x: 'east_m', y: 'north_m' },
    paths: [
        { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 60.0, y: 0.0 } ] },
    ],
    portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
    ],
}"#;

/// The compiled Increment 2 document.
fn increment_2() -> CompiledScenario {
    let source = parse_scenario_source_v2(INCREMENT_2).expect("the document parses");
    CompiledScenario::compile_v2(source).expect("the document validates and compiles")
}

/// The compiled Increment 1 version-2 document.
fn increment_1() -> CompiledScenario {
    let source = parse_scenario_source_v2(INCREMENT_1).expect("the document parses");
    CompiledScenario::compile_v2(source).expect("the document validates and compiles")
}

/// The dense id of an authored facility, permission, or band by name.
fn facility_id(scenario: &CompiledScenario, name: &str) -> FacilityId {
    let index = scenario
        .facilities()
        .iter()
        .position(|facility| facility.name() == name)
        .unwrap_or_else(|| panic!("facility '{name}' exists"));
    FacilityId::from_index(index)
}

fn mode_id(scenario: &CompiledScenario, name: &str) -> ModeTemplateId {
    let index = scenario
        .mode_templates()
        .iter()
        .position(|template| template.id() == name)
        .unwrap_or_else(|| panic!("mode template '{name}' exists"));
    ModeTemplateId::from_index(index)
}

fn policy(
    scenario: &CompiledScenario,
    mode: &str,
    facility: &str,
) -> tangle_model::FacilityTraversalPolicy {
    scenario
        .traversal_policy(
            mode_id(scenario, mode),
            facility_id(scenario, facility),
            None,
        )
        .unwrap_or_else(|| panic!("'{mode}' is eligible on '{facility}'"))
}

#[test]
fn resolves_increment_2_policy_ids_once_in_source_order() {
    let scenario = increment_2();

    assert_eq!(scenario.permissions().len(), 7);
    assert_eq!(
        scenario
            .permission(PermissionId::from_index(0))
            .map(|permission| (
                permission.name(),
                permission.kind(),
                permission.holder(),
                permission.target(),
                permission.effect(),
            )),
        Some((
            "bicycle_may_use_contraflow",
            PermissionKind::NominalDirection,
            mode_id(&scenario, "bicycle"),
            PermissionTarget::Facility(facility_id(&scenario, "bikeway_eastbound")),
            PermissionEffect::Permit,
        ))
    );
    assert_eq!(
        scenario
            .permission(PermissionId::from_index(2))
            .map(|permission| permission.target()),
        Some(PermissionTarget::Movement(MovementId::from_index(0)))
    );
    assert_eq!(
        scenario
            .permission(PermissionId::from_index(4))
            .map(|permission| (permission.kind(), permission.target())),
        Some((
            PermissionKind::Overtake,
            PermissionTarget::Facility(facility_id(&scenario, "bikeway_east_exit")),
        ))
    );
    assert_eq!(
        scenario
            .permission(PermissionId::from_index(5))
            .map(|permission| permission.target()),
        Some(PermissionTarget::Crossing(CrossingId::from_index(0)))
    );
    // A `stop_service` statement is shape-only until Increment 4: its target is
    // never resolved and it binds no traversal.
    assert_eq!(
        scenario
            .permission(PermissionId::from_index(6))
            .map(|permission| (permission.name(), permission.target(), permission.effect())),
        Some((
            "bicycle_serves_stop",
            PermissionTarget::Undeclared,
            PermissionEffect::Obligate
        ))
    );
    assert_eq!(
        scenario.permission_effect(
            PermissionKind::StopService,
            mode_id(&scenario, "bicycle"),
            PermissionTarget::Undeclared,
        ),
        Some(PermissionEffect::Obligate)
    );
    assert_eq!(
        scenario.permission_effect(
            PermissionKind::LaneUse,
            mode_id(&scenario, "passenger_car"),
            PermissionTarget::Facility(facility_id(&scenario, "bikeway_eastbound")),
        ),
        None
    );

    assert_eq!(scenario.facility_adjacencies().len(), 2);
    let adjacency = scenario
        .facility_adjacency(FacilityAdjacencyId::from_index(0))
        .expect("the adjacency exists");
    assert_eq!(adjacency.name(), "bikeway_lanes");
    assert_eq!(
        adjacency.first(),
        facility_id(&scenario, "bikeway_eastbound")
    );
    assert_eq!(
        adjacency.second(),
        facility_id(&scenario, "bikeway_westbound")
    );
    assert_eq!(adjacency.side(), AdjacencySide::Right);
    assert_eq!(
        scenario
            .facility_adjacency(FacilityAdjacencyId::from_index(1))
            .map(|adjacency| (
                adjacency.name(),
                adjacency.first(),
                adjacency.second(),
                adjacency.side(),
            )),
        Some((
            "road_side",
            facility_id(&scenario, "road_shoulder"),
            facility_id(&scenario, "road_eastbound"),
            AdjacencySide::Right,
        ))
    );
    assert_eq!(
        scenario.facility_adjacency(FacilityAdjacencyId::from_index(2)),
        None
    );

    let id_map = scenario.id_map();
    assert_eq!(
        id_map.permission_name(PermissionId::from_index(1)),
        Some("scooter_must_use_contraflow")
    );
    assert_eq!(
        id_map.facility_adjacency_name(FacilityAdjacencyId::from_index(0)),
        Some("bikeway_lanes")
    );
    assert_eq!(
        id_map.clearance_band_name(ClearanceBandId::from_index(2)),
        Some("study_band")
    );
    assert_eq!(
        id_map.facility_adjacency_name(FacilityAdjacencyId::from_index(1)),
        Some("road_side")
    );
    // Every dense id names one compiled object, in source order.
    assert_eq!(id_map.permissions().len(), scenario.permissions().len());
    assert_eq!(
        id_map.facility_adjacencies().len(),
        scenario.facility_adjacencies().len()
    );
    assert_eq!(
        id_map.clearance_bands().len(),
        scenario.clearance_bands().len()
    );
}

#[test]
fn compiles_the_lateral_policy_of_a_capable_mode_only() {
    let scenario = increment_2();

    let bicycle = scenario
        .mode_template(mode_id(&scenario, "bicycle"))
        .expect("the bicycle template compiles");
    let lateral = bicycle.lateral().expect("the bicycle has lateral motion");
    assert_eq!(lateral.target_clearance_m(), 0.75);
    assert_eq!(lateral.horizon_s(), 4.0);
    assert!(
        bicycle
            .tactics()
            .supports(tangle_model::TacticalCapability::Pass)
    );
    assert_eq!(
        bicycle
            .profile()
            .steering_rate_max_rad_s()
            .map(|range| range.max()),
        Some(0.9)
    );

    let car = scenario
        .mode_template(mode_id(&scenario, "passenger_car"))
        .expect("the car template compiles");
    assert_eq!(car.lateral(), None);
    assert_eq!(car.profile().steering_rate_max_rad_s(), None);

    // The compiled body envelope is the widest body of the mode, which is what
    // the usable interval subtracts.
    assert_eq!(bicycle.envelope_width_m(), 0.7);
    assert_eq!(bicycle.lateral_clearance_m(), 0.3);
    assert_eq!(car.envelope_width_m(), 2.0);
    assert_eq!(car.lateral_clearance_m(), 0.0);
}

/// A template authoring `lateral` and the `lateral_accel_max_mps2` parameter
/// compiles it into the behavior profile, the seam a bounded-steering stage
/// reads. `validate_v2` does not yet accept the parameter for a family that does
/// not require it, so the template compiler is driven directly here.
#[test]
fn compiles_the_lateral_acceleration_profile_parameter() {
    let source = parse_scenario_source_v2(
        r#"{
            schema_version: 2, id: 'lateral_profile',
            coordinate_system: { x: 'east_m', y: 'north_m' },
            paths: [], portals: [],
            mode_templates: [ {
                id: 'bicycle',
                body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
                    radius_m: { min: 0.35, max: 0.35 } },
                motion: 'single_body_wheeled',
                tactics: [ 'follow', 'pass' ],
                access: { facility_kinds: [ 'facility' ] },
                occupancy: 'operator_only',
                profiles: {
                    speed_mps: { min: 4.5, max: 4.5 },
                    max_accel_mps2: { min: 1.2, max: 1.2 },
                    comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                    time_gap_s: { min: 1.0, max: 1.0 },
                    steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                    lateral_accel_max_mps2: { min: 1.9, max: 1.9 },
                    lateral_clearance_m: { min: 0.3, max: 0.3 },
                    compliance: { min: 1.0, max: 1.0 },
                },
                lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
            } ],
        }"#,
    )
    .expect("the document parses");
    let template: &tangle_model::ModeTemplateSource = &source.mode_templates[0];
    let compiled: CompiledModeTemplate = compile_mode_template(template).expect("it compiles");

    assert_eq!(
        compiled.lateral().map(|lateral| lateral.horizon_s()),
        Some(4.0)
    );
    assert_eq!(
        compiled
            .profile()
            .lateral_accel_max_mps2()
            .map(|range| range.max()),
        Some(1.9)
    );
    assert!(
        compiled
            .tactics()
            .supports(tangle_model::TacticalCapability::ChooseLateralPosition)
    );
}

/// A lateral-capable box template carries all three authored lateral profile
/// parameters on its compiled bundle, exactly as a capsule does; a box that
/// authors no `lateral` carries none of them.
///
/// Validation requires the three parameters for any lateral-capable wheeled
/// body, so the compiled bundle must expose them whichever body it carries.
/// The template compiler is driven directly because a lateral box needs no
/// scenario-level policy to compile its bundle.
#[test]
fn a_lateral_box_carries_all_three_lateral_profile_parameters() {
    let box_template = |lateral: bool| {
        let lateral_field = if lateral {
            "lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },"
        } else {
            ""
        };
        let tactic = if lateral {
            "'follow', 'overtake'"
        } else {
            "'follow'"
        };
        let lateral_params = if lateral {
            "steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                    lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
                    lateral_clearance_m: { min: 0.3, max: 0.3 },"
        } else {
            ""
        };
        format!(
            r#"{{
                schema_version: 2, id: 'lateral_box',
                coordinate_system: {{ x: 'east_m', y: 'north_m' }},
                paths: [], portals: [],
                mode_templates: [ {{
                    id: 'passenger_car',
                    body: {{ kind: 'box', length_m: {{ min: 4.5, max: 4.5 }},
                        width_m: {{ min: 1.8, max: 1.8 }} }},
                    motion: 'single_body_wheeled',
                    tactics: [ {tactic} ],
                    access: {{ facility_kinds: [ 'facility' ] }},
                    occupancy: 'operator_only',
                    profiles: {{
                        speed_mps: {{ min: 9.0, max: 9.0 }},
                        max_accel_mps2: {{ min: 1.2, max: 1.2 }},
                        comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
                        time_gap_s: {{ min: 1.0, max: 1.0 }},
                        {lateral_params}
                        compliance: {{ min: 1.0, max: 1.0 }},
                    }},
                    {lateral_field}
                }} ],
            }}"#
        )
    };

    let source = parse_scenario_source_v2(&box_template(true)).expect("the document parses");
    let compiled = compile_mode_template(&source.mode_templates[0]).expect("it compiles");
    let profile = compiled.profile();
    assert_eq!(
        profile.steering_rate_max_rad_s().map(|range| range.max()),
        Some(0.9)
    );
    assert_eq!(
        profile.lateral_accel_max_mps2().map(|range| range.max()),
        Some(2.0)
    );
    assert_eq!(
        profile.lateral_clearance_m().map(|range| range.max()),
        Some(0.3)
    );
    // The template-level accessors the usable interval reads agree.
    assert_eq!(compiled.envelope_width_m(), 1.8);
    assert_eq!(compiled.lateral_clearance_m(), 0.3);

    let source = parse_scenario_source_v2(&box_template(false)).expect("the document parses");
    let compiled = compile_mode_template(&source.mode_templates[0]).expect("it compiles");
    let profile = compiled.profile();
    assert_eq!(profile.steering_rate_max_rad_s(), None);
    assert_eq!(profile.lateral_accel_max_mps2(), None);
    assert_eq!(profile.lateral_clearance_m(), None);
}

#[test]
fn keeps_nominal_permitted_and_physically_possible_directions_separate() {
    let scenario = increment_2();
    let eastbound = policy(&scenario, "bicycle", "bikeway_eastbound");

    assert_eq!(eastbound.nominal_directions(), DirectionSet::FORWARD);
    assert_eq!(eastbound.nominal_effect(), Some(PermissionEffect::Permit));
    assert_eq!(eastbound.permitted_directions(), DirectionSet::BOTH);
    assert_eq!(
        eastbound.physically_possible_directions(),
        DirectionSet::BOTH
    );
    assert_eq!(eastbound.traversable_directions(), DirectionSet::BOTH);
    assert!(eastbound.permits(MovementDirection::Reverse));

    // The obligating scope-taker contract: the scooter must travel against the
    // nominal direction.
    let scooter = policy(&scenario, "scooter", "bikeway_eastbound");
    assert_eq!(scooter.nominal_effect(), Some(PermissionEffect::Obligate));
    assert_eq!(scooter.permitted_directions(), DirectionSet::REVERSE);

    // No statement: only the nominal direction, with the mode's own access
    // restriction intersected in.
    let westbound = policy(&scenario, "bicycle", "bikeway_westbound");
    assert_eq!(westbound.nominal_directions(), DirectionSet::FORWARD);
    assert_eq!(westbound.nominal_effect(), None);
    assert_eq!(westbound.permitted_directions(), DirectionSet::FORWARD);
    // The band has no connector of its own, so both directions are physically
    // possible only because the adjacency continues onto traversals the
    // connector graph makes possible.
    assert_eq!(
        westbound.physically_possible_directions(),
        DirectionSet::BOTH
    );
    assert_eq!(westbound.traversable_directions(), DirectionSet::FORWARD);

    // An `either` facility has both directions nominal, so both are permitted.
    let turnaround = policy(&scenario, "bicycle", "bikeway_turnaround");
    assert_eq!(turnaround.nominal_directions(), DirectionSet::BOTH);
    assert_eq!(turnaround.permitted_directions(), DirectionSet::BOTH);
    assert_eq!(
        turnaround.physically_possible_directions(),
        DirectionSet::NONE
    );
    assert_eq!(turnaround.traversable_directions(), DirectionSet::NONE);

    // A `reverse` facility leaves the reverse direction nominal and permitted,
    // and only the reverse traversal is connected, so that is all that is
    // routable.
    let road = policy(&scenario, "passenger_car", "road_eastbound");
    assert_eq!(road.nominal_directions(), DirectionSet::REVERSE);
    assert_eq!(road.permitted_directions(), DirectionSet::REVERSE);
    assert_eq!(road.physically_possible_directions(), DirectionSet::REVERSE);
    assert_eq!(road.traversable_directions(), DirectionSet::REVERSE);
    assert_eq!(road.lateral_use(), LateralUse::Centered);
    assert_eq!(road.passing_side(), None);

    // A mode the facility does not permit is not eligible.
    assert_eq!(
        scenario.traversal_policy(
            mode_id(&scenario, "scooter"),
            facility_id(&scenario, "bikeway_turnaround"),
            None,
        ),
        None
    );
    // An undeclared facility is not eligible either.
    assert_eq!(
        scenario.traversal_policy(
            mode_id(&scenario, "bicycle"),
            FacilityId::from_index(99),
            None,
        ),
        None
    );
}

#[test]
fn uses_the_eligible_body_envelope_and_clearance_for_the_usable_interval() {
    let scenario = increment_2();

    // Bicycle: 3.0 m band, 0.7 m envelope, 0.3 m clearance.
    let bicycle = policy(&scenario, "bicycle", "bikeway_eastbound").usable_interval();
    assert!((bicycle.d_min() + 0.85).abs() < 1e-12);
    assert!((bicycle.d_max() - 0.85).abs() < 1e-12);
    assert!(!bicycle.is_empty());
    assert!(bicycle.contains(0.0));

    // Scooter: the same band, a 0.7 m envelope, and 0.2 m clearance.
    let scooter = policy(&scenario, "scooter", "bikeway_eastbound").usable_interval();
    assert!((scooter.d_max() - 0.95).abs() < 1e-12);

    // The car: a 3.5 m centered band, a 2.0 m envelope, and no declared
    // clearance, which is the Increment 1 default of zero.
    let car = policy(&scenario, "passenger_car", "road_eastbound").usable_interval();
    assert!((car.d_min() + 0.75).abs() < 1e-12);
    assert!((car.d_max() - 0.75).abs() < 1e-12);
}

#[test]
fn compiles_lateral_and_longitudinal_transition_targets() {
    let scenario = increment_2();
    let eastbound = facility_id(&scenario, "bikeway_eastbound");
    let westbound = facility_id(&scenario, "bikeway_westbound");
    let exit = facility_id(&scenario, "bikeway_east_exit");

    // The connectors that leave the traversal's end, unchanged from Increment 1.
    let forward = scenario
        .transitions(eastbound, MovementDirection::Forward)
        .expect("the facility is declared");
    assert_eq!(forward.longitudinal(), [FacilityConnectorId::from_index(0)]);
    assert_eq!(forward.lateral().len(), 1);
    assert_eq!(forward.lateral()[0].target().facility(), westbound);
    assert_eq!(
        forward.lateral()[0].target().direction(),
        MovementDirection::Reverse
    );
    // The eastbound band travels +x, so the band at -y lies on its right.
    assert_eq!(forward.lateral()[0].side(), AdjacencySide::Right);

    let reverse = scenario
        .transitions(eastbound, MovementDirection::Reverse)
        .expect("the facility is declared");
    assert_eq!(reverse.longitudinal(), [FacilityConnectorId::from_index(3)]);
    // Traveling the other way, the same crossing continues onto the other
    // facility's forward direction and lies on the other side in the agent's
    // own travel frame.
    assert_eq!(reverse.lateral()[0].target().facility(), westbound);
    assert_eq!(
        reverse.lateral()[0].target().direction(),
        MovementDirection::Forward
    );
    assert_eq!(reverse.lateral()[0].side(), AdjacencySide::Left);

    // The adjacency is undirected, so the westbound band reaches the eastbound
    // one with the side mapped into its own travel frame.
    let westbound_forward = scenario
        .transitions(westbound, MovementDirection::Forward)
        .expect("the facility is declared");
    assert_eq!(westbound_forward.longitudinal(), []);
    assert_eq!(
        westbound_forward.lateral()[0].target().facility(),
        eastbound
    );
    assert_eq!(
        westbound_forward.lateral()[0].target().direction(),
        MovementDirection::Reverse
    );
    assert_eq!(westbound_forward.lateral()[0].side(), AdjacencySide::Right);
    let westbound_reverse = scenario
        .transitions(westbound, MovementDirection::Reverse)
        .expect("the facility is declared");
    assert_eq!(westbound_reverse.lateral()[0].side(), AdjacencySide::Left);

    assert!(
        scenario
            .transitions(exit, MovementDirection::Reverse)
            .is_some()
    );
    assert_eq!(
        scenario
            .transitions(exit, MovementDirection::Reverse)
            .expect("the facility is declared")
            .lateral(),
        []
    );
    // A facility beside another one but joined by no adjacency transitions
    // nowhere: proximity is never inferred.
    let turnaround = facility_id(&scenario, "bikeway_turnaround");
    for direction in [MovementDirection::Forward, MovementDirection::Reverse] {
        let transitions = scenario
            .transitions(turnaround, direction)
            .expect("the facility is declared");
        assert_eq!(transitions.lateral(), []);
        assert_eq!(transitions.longitudinal(), []);
    }
    assert_eq!(
        scenario.transitions(FacilityId::from_index(99), MovementDirection::Forward),
        None
    );

    // The connector graph makes both directions physically possible on the
    // joined pair, which is what a lateral crossing into it would continue.
    assert_eq!(
        scenario
            .facility(exit)
            .expect("the facility exists")
            .physically_possible_directions(),
        [MovementDirection::Forward, MovementDirection::Reverse]
    );
}

#[test]
fn compiles_the_shared_boundary_of_a_straight_parallel_pair() {
    let scenario = increment_2();
    let eastbound = facility_id(&scenario, "bikeway_eastbound");
    let westbound = facility_id(&scenario, "bikeway_westbound");
    let adjacency = scenario
        .facility_adjacency(FacilityAdjacencyId::from_index(0))
        .expect("bikeway_lanes exists");

    // The two 3 m bands meet on `y = -1.5` from `x = 0` to `x = 120`, so the
    // longest shared segment's midpoint is the middle of that edge.
    assert_eq!(adjacency.shared_boundary_midpoint(), DVec2::new(60.0, -1.5));
    // The boundary is half a band width to the right of each band's own
    // reference: the eastbound reference is `y = 0`, the westbound `y = -3`.
    for band in [eastbound, westbound] {
        assert_eq!(
            adjacency.shared_boundary_offset(band, MovementDirection::Forward),
            Some(-1.5)
        );
        assert_eq!(
            adjacency.shared_boundary_offset(band, MovementDirection::Reverse),
            Some(1.5)
        );
    }
    // The offset sign agrees with the compiled crossing side in the agent's own
    // travel frame: left of travel is positive `d`, right of travel negative.
    for band in [eastbound, westbound] {
        for direction in [MovementDirection::Forward, MovementDirection::Reverse] {
            let side = adjacency
                .transition(band, direction)
                .expect("the band is one side of the adjacency")
                .side();
            let offset = adjacency
                .shared_boundary_offset(band, direction)
                .expect("the band is one side of the adjacency");
            assert_eq!(
                side,
                if offset > 0.0 {
                    AdjacencySide::Left
                } else {
                    AdjacencySide::Right
                }
            );
        }
    }
    // A facility that is not one of the two bands has no boundary offset.
    assert_eq!(
        adjacency.shared_boundary_offset(
            facility_id(&scenario, "road_eastbound"),
            MovementDirection::Forward
        ),
        None
    );
}

#[test]
fn compiles_the_shared_boundary_of_a_curved_pair() {
    let source = parse_scenario_source_v2(CURVED).expect("the document parses");
    let scenario =
        CompiledScenario::compile_v2(source).expect("the document validates and compiles");
    let inner = facility_id(&scenario, "inner_lane");
    let outer = facility_id(&scenario, "outer_lane");
    let adjacency = scenario
        .facility_adjacency(FacilityAdjacencyId::from_index(0))
        .expect("curved_pair exists");

    // The longest shared mid-arc chord runs from `(-4, 0)` to `(-2.8284,
    // 2.8284)`, so the boundary sits at its midpoint.
    let midpoint = adjacency.shared_boundary_midpoint();
    let chord_midpoint = (DVec2::new(-4.0, 0.0) + DVec2::new(-2.8284, 2.8284)) * 0.5;
    assert!((midpoint - chord_midpoint).length() < 1e-12);

    // Each band's boundary offset is the midpoint's projection onto that band's
    // own reference in its travel frame. The chordal references are not parallel
    // to the shared chord, so the distance is the curved 0.462 m, not the
    // straight `width_m / 2` half width of 0.5 m a runtime approximation reports.
    for (band, name) in [(inner, "inner_lane"), (outer, "outer_lane")] {
        let reference = scenario
            .facility(band)
            .unwrap_or_else(|| panic!("'{name}' exists"))
            .reference()
            .expect("the band declares a reference path")
            .geometry();
        let expected = reference.project(midpoint).d();
        assert!((expected.abs() - 0.4619).abs() < 1e-3);
        assert!((expected.abs() - 0.5).abs() > 0.01);
        let forward = adjacency
            .shared_boundary_offset(band, MovementDirection::Forward)
            .expect("the band is one side of the adjacency");
        let reverse = adjacency
            .shared_boundary_offset(band, MovementDirection::Reverse)
            .expect("the band is one side of the adjacency");
        assert!((forward - expected).abs() < 1e-12);
        assert!((reverse + expected).abs() < 1e-12);
    }
}

#[test]
fn a_corner_only_touch_yields_no_boundary_and_is_rejected() {
    let source = parse_scenario_source_v2(CORNER).expect("the document parses");
    let diagnostics = CompiledScenario::compile_v2(source)
        .expect_err("a corner-only touch shares no boundary, so the document is rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::FacilityAdjacencyDisjoint),
        "expected the disjoint-adjacency diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn an_adjacency_possibility_comes_from_the_adjacent_facility_only() {
    let scenario = increment_2();
    let shoulder = facility_id(&scenario, "road_shoulder");
    let road = facility_id(&scenario, "road_eastbound");

    let shoulder_policy = policy(&scenario, "passenger_car", "road_shoulder");
    assert_eq!(shoulder_policy.nominal_directions(), DirectionSet::BOTH);
    assert_eq!(shoulder_policy.permitted_directions(), DirectionSet::BOTH);
    // The shoulder has no connector at all: its reverse traversal is physically
    // possible only because the road's reverse traversal is, and its forward
    // traversal continues onto a direction the road does not connect.
    assert_eq!(
        shoulder_policy.physically_possible_directions(),
        DirectionSet::REVERSE
    );
    assert_eq!(
        shoulder_policy.traversable_directions(),
        DirectionSet::REVERSE
    );

    let forward = scenario
        .transitions(shoulder, MovementDirection::Forward)
        .expect("the facility is declared");
    assert_eq!(forward.lateral().len(), 1);
    assert_eq!(forward.lateral()[0].target().facility(), road);
    assert_eq!(
        forward.lateral()[0].target().direction(),
        MovementDirection::Forward
    );
    // The shoulder faces the road's centerline, so the road lies on the side
    // its own forward direction names.
    assert_eq!(forward.lateral()[0].side(), AdjacencySide::Right);
    assert_eq!(
        scenario
            .transitions(shoulder, MovementDirection::Reverse)
            .expect("the facility is declared")
            .lateral()[0]
            .side(),
        AdjacencySide::Left
    );
    assert_eq!(
        scenario
            .transitions(road, MovementDirection::Forward)
            .expect("the facility is declared")
            .lateral()[0]
            .target()
            .facility(),
        shoulder
    );
}

#[test]
fn keeps_a_crossing_onto_an_unpermitted_direction_as_a_forbidden_boundary() {
    let scenario = increment_2();
    let shoulder = facility_id(&scenario, "road_shoulder");
    let road = policy(&scenario, "passenger_car", "road_eastbound");

    // The road is a contraflow facility for this car: only its reverse
    // traversal is permitted, so the shoulder's forward crossing is kept as a
    // target and flagged by the destination's permitted set, which is what
    // makes it a forbidden boundary rather than a missing adjacency.
    let forward_target = scenario
        .transitions(shoulder, MovementDirection::Forward)
        .expect("the facility is declared")
        .lateral()[0];
    assert_eq!(
        forward_target.target().direction(),
        MovementDirection::Forward
    );
    assert!(!road.permits(forward_target.target().direction()));

    let reverse_target = scenario
        .transitions(shoulder, MovementDirection::Reverse)
        .expect("the facility is declared")
        .lateral()[0];
    assert!(road.permits(reverse_target.target().direction()));
    assert!(
        road.physically_possible_directions()
            .contains(reverse_target.target().direction())
    );
}

#[test]
fn resolves_the_applicable_pass_lane_use_and_crossing_policy() {
    let scenario = increment_2();

    let eastbound = policy(&scenario, "bicycle", "bikeway_eastbound");
    assert_eq!(eastbound.passing_side(), Some(PassingSide::Left));
    assert_eq!(eastbound.lane_use(), Some(PermissionEffect::Obligate));
    assert_eq!(eastbound.overtake(), None);

    let exit = policy(&scenario, "scooter", "bikeway_east_exit");
    assert_eq!(exit.overtake(), Some(PermissionEffect::Prohibit));
    assert_eq!(exit.lane_use(), None);
    // A facility authoring no `lateral_policy` offers no lateral target.
    assert_eq!(exit.passing_side(), None);

    assert_eq!(
        scenario.crossing_permission(mode_id(&scenario, "bicycle"), CrossingId::from_index(0)),
        Some(PermissionEffect::Permit)
    );
    assert_eq!(
        scenario.crossing_permission(
            mode_id(&scenario, "passenger_car"),
            CrossingId::from_index(0)
        ),
        None
    );
}

#[test]
fn a_movement_targeted_statement_decides_the_traversal_that_carries_it() {
    let scenario = increment_2();
    let bicycle = mode_id(&scenario, "bicycle");
    let eastbound = facility_id(&scenario, "bikeway_eastbound");
    let movement = MovementId::from_index(0);

    let without = scenario
        .traversal_policy(bicycle, eastbound, None)
        .expect("the mode is eligible");
    assert_eq!(without.movement(), None);
    assert_eq!(without.nominal_effect(), Some(PermissionEffect::Permit));
    assert_eq!(without.permitted_directions(), DirectionSet::BOTH);

    // The traversal carries the movement, so the narrower object decides.
    let carrying = scenario
        .traversal_policy(bicycle, eastbound, Some(movement))
        .expect("the mode is eligible");
    assert_eq!(carrying.movement(), Some(movement));
    assert_eq!(carrying.nominal_effect(), Some(PermissionEffect::Prohibit));
    assert_eq!(carrying.permitted_directions(), DirectionSet::FORWARD);
    assert_eq!(
        scenario.permission_effect(
            PermissionKind::NominalDirection,
            bicycle,
            PermissionTarget::Movement(movement),
        ),
        Some(PermissionEffect::Prohibit)
    );
}

#[test]
fn compiles_clearance_bands_in_declaration_order() {
    let scenario = increment_2();

    let bands = scenario.clearance_bands();
    assert_eq!(bands.len(), 3);
    assert_eq!(
        bands
            .iter()
            .map(|band| (band.name(), band.threshold_m(), band.violation()))
            .collect::<Vec<_>>(),
        [
            ("narrow_close_pass", 0.75, true),
            ("motor_close_pass", 1.0, true),
            ("study_band", 1.5, false),
        ]
    );
    assert_eq!(
        bands[0].applies_to_modes(),
        Some([mode_id(&scenario, "bicycle")].as_slice())
    );
    assert!(bands[0].applies_to(mode_id(&scenario, "bicycle")));
    assert!(!bands[0].applies_to(mode_id(&scenario, "scooter")));
    // An absent list applies to every mode pair.
    assert_eq!(bands[2].applies_to_modes(), None);
    assert!(bands[2].applies_to(mode_id(&scenario, "passenger_car")));
    assert_eq!(
        bands[1].applies_to_modes(),
        Some([mode_id(&scenario, "scooter")].as_slice())
    );
    assert_eq!(bands[0].id(), ClearanceBandId::from_index(0));
    assert_eq!(
        scenario.clearance_band(ClearanceBandId::from_index(3)),
        None
    );
}

#[test]
fn compiles_the_scenario_scoped_maneuver_policy_without_defaulting() {
    let scenario = increment_2();

    let commit = scenario.commit_policy().expect("the commit policy exists");
    assert_eq!(commit.min_predicted_clearance_m, 0.25);
    assert_eq!(commit.hold_timeout_s, 2.0);
    let wrong_way = scenario
        .wrong_way_policy()
        .expect("the wrong-way policy exists");
    assert_eq!(wrong_way.min_time_saving_s, 10.0);
    assert_eq!(wrong_way.max_opposing_density_per_km, 40.0);
    assert_eq!(wrong_way.urgency, 0.5);
}

#[test]
fn an_increment_1_document_compiles_to_the_increment_1_behavior() {
    let scenario = increment_1();

    assert!(scenario.facility_adjacencies().is_empty());
    assert!(scenario.permissions().is_empty());
    assert!(scenario.clearance_bands().is_empty());
    assert_eq!(scenario.maneuver_policy(), None);
    assert_eq!(scenario.commit_policy(), None);
    assert_eq!(scenario.wrong_way_policy(), None);

    let west = scenario
        .facility(facility_id(&scenario, "west_lane"))
        .expect("the west lane exists");
    assert_eq!(west.passing_side(), None);
    assert_eq!(west.lateral_use(), LateralUse::Shared);
    assert_eq!(
        scenario
            .mode_template(mode_id(&scenario, "cycle"))
            .expect("the cycle template compiles")
            .lateral(),
        None
    );

    // No permission binds the mode, so only the nominal direction is permitted;
    // no adjacency exists, so the reachable targets are the Increment 1
    // connector targets alone.
    let lane = policy(&scenario, "cycle", "west_lane");
    assert_eq!(lane.nominal_effect(), None);
    assert_eq!(lane.permitted_directions(), DirectionSet::FORWARD);
    assert_eq!(lane.lane_use(), None);
    assert_eq!(lane.overtake(), None);
    assert_eq!(lane.passing_side(), None);
    let transitions = scenario
        .transitions(
            facility_id(&scenario, "west_lane"),
            MovementDirection::Forward,
        )
        .expect("the facility is declared");
    assert_eq!(
        transitions.longitudinal(),
        [FacilityConnectorId::from_index(0)]
    );
    assert_eq!(transitions.lateral(), []);
    assert_eq!(
        transitions.longitudinal(),
        west.outgoing_connectors(),
        "the traversal's connector targets are the Increment 1 connector list"
    );
    assert_eq!(
        scenario
            .facility_connector(FacilityConnectorId::from_index(0))
            .map(|connector| (
                connector.name(),
                connector.from().facility(),
                connector.from().direction(),
                connector.to().facility(),
            )),
        Some((
            "west_to_east",
            facility_id(&scenario, "west_lane"),
            MovementDirection::Forward,
            facility_id(&scenario, "east_lane"),
        ))
    );
}

#[test]
fn resolves_a_dangling_increment_2_reference_without_panicking() {
    let source = parse_scenario_source_v2(DANGLING).expect("the document parses");
    // A reference no declared object supplies is validation's to reject
    // (TAS-086). Until that rule lands the compiler resolves it without
    // panicking: an adjacency to an undeclared facility is dropped, a band mode
    // no template supplies is dropped from the band's list, and a statement
    // whose target does not resolve matches no traversal.
    match CompiledScenario::compile_v2(source) {
        Ok(scenario) => {
            assert!(scenario.facility_adjacencies().is_empty());
            assert_eq!(scenario.clearance_bands().len(), 1);
            assert_eq!(
                scenario.clearance_bands()[0].applies_to_modes(),
                Some([].as_slice())
            );
            assert!(!scenario.clearance_bands()[0].applies_to(mode_id(&scenario, "cycle")));
            assert_eq!(
                scenario
                    .permission(PermissionId::from_index(0))
                    .map(|permission| permission.target()),
                Some(PermissionTarget::Undeclared)
            );
            let lane = policy(&scenario, "cycle", "lane");
            assert_eq!(lane.lane_use(), None);
            assert_eq!(lane.permitted_directions(), DirectionSet::FORWARD);
        }
        // A validator that rejects the reference outright is the other correct
        // outcome; panicking is not.
        Err(diagnostics) => assert!(!diagnostics.is_empty()),
    }
}

#[test]
fn a_version_1_document_compiles_to_the_increment_1_behavior() {
    let source = parse_scenario_source(VERSION_1).expect("the document parses");
    let scenario = CompiledScenario::compile(source).expect("the document compiles");

    assert!(scenario.mode_templates().is_empty());
    assert!(scenario.facilities().is_empty());
    assert!(scenario.facility_connectors().is_empty());
    assert!(scenario.facility_adjacencies().is_empty());
    assert!(scenario.permissions().is_empty());
    assert!(scenario.clearance_bands().is_empty());
    assert_eq!(scenario.maneuver_policy(), None);
    assert_eq!(
        scenario.transitions(FacilityId::from_index(0), MovementDirection::Forward),
        None
    );
}
