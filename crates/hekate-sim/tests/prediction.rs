//! TAS-090: analytic straight-line fixtures for the maneuver-corridor
//! predictor.
//!
//! A bounded candidate maneuver is predicted over its horizon against exact
//! body shapes collected by the ordinary broad phase, and the front, rear,
//! side, and swept clearance facts are asserted against hand-computed
//! straight-line geometry. The fixtures cover an approaching rear, a slower
//! front, a side-by-side body, a crossing sweep caught between samples, an
//! empty corridor, a curved boundary, and an exact tie, plus insertion-order
//! independence and the conservative minimum that gates feasibility.

use glam::DVec2;
use hekate_model::{CompiledReferencePath, CompiledScenario, FacilityId, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, BodyShape, BoundedSteering, LateralCorridor, LimitingObject, ManeuverInputs,
    PredictedBody, PredictionVerdict, SteeringLimits, body_clearance_m, predict_maneuver_corridor,
};

/// A version-2 scenario with one `bikeway` facility of width 3.0 m on a
/// straight reference, so the predictor can be driven from compiled geometry.
const COMPILED_FACILITY: &str = r#"
{
  schema_version: 2,
  id: 'prediction_fixture',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
  portals: [
    { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
    { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
  ],
  boundaries: [
    { id: 'world', points: [
      { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
      { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
    ] },
  ],
  regions: [
    { id: 'band', points: [
      { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
      { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
    ] },
  ],
  facilities: [
    { id: 'bikeway', region: 'band', reference_path: 'guide',
      width_m: 3.0, nominal_direction: 'forward',
      access: { modes: [ 'rider' ] }, lateral_use: 'shared',
      lateral_policy: { passing_side: 'left' },
      speed_policy: { limit_mps: null } },
  ],
  movements: [
    { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
      direction: 'forward' },
  ],
  mode_templates: [
    {
      id: 'rider',
      body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
        radius_m: { min: 0.35, max: 0.35 } },
      motion: 'single_body_wheeled',
      tactics: [ 'follow', 'stop', 'yield', 'pass' ],
      access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
        speed_policy: { limit_mps: null } },
      occupancy: 'operator_only',
      profiles: {
        speed_mps: { min: 6.0, max: 6.0 },
        max_accel_mps2: { min: 1.2, max: 1.2 },
        comfortable_brake_mps2: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.0, max: 1.0 },
        steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
        lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
        lateral_clearance_m: { min: 0.3, max: 0.3 },
        compliance: { min: 1.0, max: 1.0 },
      },
      lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
    },
  ],
  permissions: [],
  maneuver_policy: {
    commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
  },
  demand: [],
}
"#;

/// A straight reference along the world x axis, so the left normal is `+y`.
fn straight() -> CompiledReferencePath {
    CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(400.0, 0.0)])
}

/// The fixture's bounded-steering limits and a symmetric usable corridor.
fn steering(corridor_half_m: f64) -> BoundedSteering {
    BoundedSteering {
        limits: SteeringLimits {
            heading_rate_max_rad_s: 0.9,
            lateral_accel_max_mps2: 2.0,
        },
        corridor: LateralCorridor {
            d_min: -corridor_half_m,
            d_max: corridor_half_m,
        },
    }
}

/// A 4 m by 2 m box envelope, the fixture agent.
fn car(centre: DVec2, heading_rad: f64) -> BodyShape {
    BodyShape::Box {
        centre,
        heading_rad,
        length_m: 4.0,
        width_m: 2.0,
    }
}

/// The standard straight-corridor request: a car at `x = 50` travelling
/// forward at 10 m/s on a 7 m band, clearing 0.5 m, over a 2 s horizon.
fn inputs<'a>(geometry: &'a CompiledReferencePath, target_offset_m: f64) -> ManeuverInputs<'a> {
    ManeuverInputs {
        geometry,
        facility_width_m: 7.0,
        direction: 1.0,
        body: car(DVec2::new(50.0, 0.0), 0.0),
        speed_mps: 10.0,
        target_offset_m,
        steering: steering(2.0),
        target_clearance_m: 0.5,
        horizon_s: 2.0,
        cadence_s: 0.1,
        subdivisions: 4,
    }
}

/// The empty corridor is feasible, tracks the reference, and reports the band
/// edge as its swept fact.
#[test]
fn the_empty_corridor_is_feasible_and_tracks_the_reference() {
    let geometry = straight();
    let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[]);
    assert!(prediction.is_feasible());
    assert_eq!(prediction.verdict, PredictionVerdict::Feasible);
    assert_eq!(prediction.clears.swept.object, LimitingObject::BandEdge);
    // Band half width 3.5, envelope half extent 1.0 at offset 0: 2.5 m clear.
    assert!((prediction.clears.swept.clearance_m - 2.5).abs() < 1e-9);
    assert!(prediction.clears.front.is_none());
    assert!(prediction.clears.rear.is_none());
    assert!(prediction.clears.side.is_none());

    // The corridor advances forward along the reference at the sampled speed.
    let first = prediction.corridor.first().expect("a first sample");
    let last = prediction.corridor.last().expect("a last sample");
    assert_eq!(first.time_s, 0.0);
    assert!((first.d_m).abs() < 1e-9);
    assert!(last.time_s >= 1.9 && last.time_s <= 2.0);
    assert!(last.s_m > first.s_m);
    assert!((last.d_m).abs() < 1e-3, "straight travel keeps the offset");
}

/// A slower body ahead is a front fact, and the minimum is the closing gap.
#[test]
fn a_slower_front_body_reports_the_closing_gap() {
    let geometry = straight();
    // Lead box centre at 64 m, footprint [62, 66] ahead of the agent [48, 52].
    let lead = PredictedBody {
        id: AgentId::from_index(0),
        shape: car(DVec2::new(64.0, 0.0), 0.0),
        velocity_mps: DVec2::new(6.0, 0.0),
    };
    let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[lead]);
    let front = prediction.clears.front.expect("a body is ahead");
    assert_eq!(front.object, LimitingObject::Agent(AgentId::from_index(0)));
    // Start gap 10 m, closing 4 m/s for 2 s: least clearance 2 m.
    assert!(
        (front.clearance_m - 2.0).abs() < 1e-9,
        "{}",
        front.clearance_m
    );
    assert!(front.closing_speed_mps > 0.0);
    assert_eq!(front.time_s, 2.0);
    assert!(prediction.clears.rear.is_none());
    assert!(prediction.clears.side.is_none());
    assert!(prediction.is_feasible());
}

/// A faster body behind is a rear fact with a positive closing speed.
#[test]
fn an_approaching_rear_body_reports_the_closing_gap() {
    let geometry = straight();
    let follower = PredictedBody {
        id: AgentId::from_index(0),
        shape: BodyShape::Circle {
            centre: DVec2::new(36.0, 0.0),
            radius_m: 1.0,
        },
        velocity_mps: DVec2::new(20.0, 0.0),
    };
    let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[follower]);
    let rear = prediction.clears.rear.expect("a body is behind");
    assert_eq!(rear.object, LimitingObject::Agent(AgentId::from_index(0)));
    assert!(rear.closing_speed_mps > 0.0, "{}", rear.closing_speed_mps);
    assert!(prediction.clears.front.is_none());
    assert!(prediction.clears.side.is_none());
    // The follower closes through the agent's rear, so the maneuver is not
    // feasible over the horizon.
    assert!(!prediction.is_feasible());
}

/// A body abreast is a side fact, and a zero-clearance pass is below target.
#[test]
fn a_side_by_side_body_reports_a_side_fact() {
    let geometry = straight();
    let alongside = PredictedBody {
        id: AgentId::from_index(0),
        shape: car(DVec2::new(50.0, 2.0), 0.0),
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[alongside]);
    let side = prediction.clears.side.expect("a body is alongside");
    assert_eq!(side.object, LimitingObject::Agent(AgentId::from_index(0)));
    // Centres 2 m apart, half widths 1 m each: zero surface clearance.
    assert!(side.clearance_m.abs() < 1e-9, "{}", side.clearance_m);
    assert_eq!(prediction.clears.swept.object, side.object);
    assert!(prediction.clears.front.is_none());
    assert!(prediction.clears.rear.is_none());
    assert!(
        !prediction.is_feasible(),
        "a touching side body is below target"
    );
}

/// A body crossing between two corridor samples is caught by the swept read,
/// even at a single subdivision, so the endpoints alone do not decide.
#[test]
fn a_crossing_sweep_is_caught_between_samples() {
    let geometry = straight();
    let crossing = PredictedBody {
        id: AgentId::from_index(0),
        shape: BodyShape::Circle {
            centre: DVec2::new(50.0, 5.0),
            radius_m: 0.5,
        },
        velocity_mps: DVec2::new(0.0, -10.0),
    };
    // The agent holds station; one cadence of one second covers the whole pass.
    let hold = ManeuverInputs {
        speed_mps: 0.0,
        cadence_s: 1.0,
        subdivisions: 1,
        ..inputs(&geometry, 0.0)
    };
    let prediction = predict_maneuver_corridor(hold, &[crossing]);

    // The endpoints are clear, so only the swept interval read sees the pass.
    let agent = car(DVec2::new(50.0, 0.0), 0.0);
    let start = body_clearance_m(&agent, &crossing.shape);
    let end = body_clearance_m(
        &agent,
        &BodyShape::Circle {
            centre: crossing.shape.centre() + DVec2::new(0.0, -20.0),
            radius_m: 0.5,
        },
    );
    assert!(start > 0.0 && end > 0.0, "start {start} end {end}");
    assert!(
        prediction.clears.swept.clearance_m < 0.0,
        "the swept minimum caught the crossing: {}",
        prediction.clears.swept.clearance_m
    );
    assert_eq!(
        prediction.clears.swept.object,
        LimitingObject::Agent(AgentId::from_index(0))
    );
    assert!(!prediction.is_feasible());
}

/// On a curved reference the band edge is the compiled constant-width boundary
/// in route coordinates, and it binds when the envelope nears it.
#[test]
fn a_curved_boundary_reports_the_route_relative_band_edge() {
    // A gentle arc of radius 1000 m; the tangent at the start is +x.
    let geometry = CompiledReferencePath::arc(
        DVec2::new(0.0, 1000.0),
        1000.0,
        -0.5 * std::f64::consts::PI,
        0.05,
    );
    let body = car(geometry.point_at(0.0, 0.6), geometry.heading_at(0.0));
    let curved = ManeuverInputs {
        facility_width_m: 3.5,
        body,
        // Hold the offset so the envelope stays aligned with the reference.
        target_offset_m: 0.6,
        target_clearance_m: 0.5,
        horizon_s: 0.1,
        ..inputs(&geometry, 0.0)
    };
    let prediction = predict_maneuver_corridor(curved, &[]);
    // Band half width 1.75, offset 0.6, envelope half extent 1.0: 0.15 m,
    // which is below the target, so the curved boundary limits the maneuver.
    assert_eq!(
        prediction.verdict,
        PredictionVerdict::Infeasible {
            limiting: LimitingObject::BandEdge
        }
    );
    assert!(
        prediction.clears.swept.clearance_m <= 0.15 + 1e-9
            && prediction.clears.swept.clearance_m > 0.10,
        "swept {}",
        prediction.clears.swept.clearance_m
    );
}

/// The whole corridor is read, so a body in the current flow and a body at the
/// destination offset are both facts, not only the two endpoint lane centres.
#[test]
fn the_corridor_covers_the_current_and_destination_flows() {
    let geometry = straight();
    // The agent changes from offset 0 to offset 1.5 within the band.
    // Current flow: a same-speed body behind it in its own lane.
    let current = PredictedBody {
        id: AgentId::from_index(0),
        shape: BodyShape::Circle {
            centre: DVec2::new(36.0, 0.0),
            radius_m: 1.0,
        },
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    // Destination flow: a same-speed body ahead in the target lane.
    let destination = PredictedBody {
        id: AgentId::from_index(1),
        shape: car(DVec2::new(60.0, 1.5), 0.0),
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    let prediction = predict_maneuver_corridor(inputs(&geometry, 1.5), &[current, destination]);
    let front = prediction
        .clears
        .front
        .expect("the destination body is ahead");
    assert_eq!(front.object, LimitingObject::Agent(AgentId::from_index(1)));
    let rear = prediction.clears.rear.expect("the current body is behind");
    assert_eq!(rear.object, LimitingObject::Agent(AgentId::from_index(0)));
    assert!(prediction.is_feasible());
    // The corridor actually advances toward the destination offset.
    let last = prediction.corridor.last().expect("a last sample");
    assert!(
        last.d_m > 0.5,
        "the corridor moves toward the target lane: {}",
        last.d_m
    );
}

/// A target outside the usable corridor is infeasible and names the band edge.
#[test]
fn a_target_outside_the_usable_corridor_names_the_band_edge() {
    let geometry = straight();
    let prediction = predict_maneuver_corridor(inputs(&geometry, 5.0), &[]);
    assert_eq!(
        prediction.verdict,
        PredictionVerdict::Infeasible {
            limiting: LimitingObject::BandEdge
        }
    );
}

/// Two bodies at exactly equal clearance tie on the lower stable `AgentId`.
#[test]
fn an_exact_tie_keeps_the_lower_agent_id() {
    let geometry = straight();
    let left = PredictedBody {
        id: AgentId::from_index(1),
        shape: BodyShape::Circle {
            centre: DVec2::new(50.0, 2.0),
            radius_m: 0.5,
        },
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    let right = PredictedBody {
        id: AgentId::from_index(0),
        shape: BodyShape::Circle {
            centre: DVec2::new(50.0, -2.0),
            radius_m: 0.5,
        },
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[left, right]);
    assert_eq!(
        prediction.clears.swept.object,
        LimitingObject::Agent(AgentId::from_index(0))
    );
    assert!((prediction.clears.swept.clearance_m - 0.5).abs() < 1e-9);
}

/// The result is a pure function of the body *set*, not of insertion order.
#[test]
fn the_result_is_independent_of_body_insertion_order() {
    let geometry = straight();
    let ahead = PredictedBody {
        id: AgentId::from_index(0),
        shape: car(DVec2::new(60.0, 0.0), 0.0),
        velocity_mps: DVec2::new(8.0, 0.0),
    };
    let behind = PredictedBody {
        id: AgentId::from_index(1),
        shape: BodyShape::Circle {
            centre: DVec2::new(44.0, 0.0),
            radius_m: 0.5,
        },
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    let forward = predict_maneuver_corridor(inputs(&geometry, 0.0), &[ahead, behind]);
    let reversed = predict_maneuver_corridor(inputs(&geometry, 0.0), &[behind, ahead]);
    assert_eq!(forward, reversed);
}

/// Feasible is exactly the reported minimum clearing the target: a finer
/// subdivision may only lower the minimum, so it can never turn a below-target
/// minimum into a feasible verdict.
#[test]
fn feasibility_tracks_the_reported_minimum_across_subdivisions() {
    let geometry = straight();
    // A side body 0.25 m clear, with a 0.3 m target: below target at any
    // subdivision.
    let alongside = PredictedBody {
        id: AgentId::from_index(0),
        shape: BodyShape::Circle {
            centre: DVec2::new(50.0, 1.75),
            radius_m: 0.5,
        },
        velocity_mps: DVec2::new(10.0, 0.0),
    };
    for subdivisions in [1, 4, 64] {
        let prediction = predict_maneuver_corridor(
            ManeuverInputs {
                target_clearance_m: 0.3,
                subdivisions,
                ..inputs(&geometry, 0.0)
            },
            &[alongside],
        );
        assert!(
            (prediction.clears.swept.clearance_m - 0.25).abs() < 1e-9,
            "subdivisions {subdivisions}: {}",
            prediction.clears.swept.clearance_m
        );
        assert!(!prediction.is_feasible());
        assert_eq!(
            prediction.verdict,
            PredictionVerdict::Infeasible {
                limiting: LimitingObject::Agent(AgentId::from_index(0))
            }
        );
    }
}

/// The predictor reads compiled geometry: a band width and reference taken from
/// a `CompiledFacility`, not a hand-built pair.
#[test]
fn the_predictor_reads_a_compiled_facility_reference_and_width() {
    let source = parse_scenario_source_v2(COMPILED_FACILITY).expect("the document is version 2");
    let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
    let facility = scenario
        .facility(FacilityId::from_index(0))
        .expect("the facility compiles");
    let geometry = facility
        .reference()
        .expect("the facility has a reference")
        .geometry();
    let request = ManeuverInputs {
        facility_width_m: facility.width_m(),
        target_clearance_m: 0.4,
        horizon_s: 0.5,
        ..inputs(geometry, 0.0)
    };
    let prediction = predict_maneuver_corridor(request, &[]);
    // Width 3.0, envelope half extent 1.0 at offset 0: 0.5 m of band clearance.
    assert!((prediction.clears.swept.clearance_m - 0.5).abs() < 1e-9);
    assert!(prediction.is_feasible());
}
