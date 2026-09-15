//! TAS-090: analytic straight-line fixtures for the maneuver-corridor
//! predictor, and TAS-126: the production prediction against the executed
//! clearance at each fidelity preset.
//!
//! A bounded candidate maneuver is predicted over its horizon against exact
//! body shapes collected by the ordinary broad phase, and the front, rear,
//! side, and swept clearance facts are asserted against hand-computed
//! straight-line geometry. The fixtures cover an approaching rear, a slower
//! front, a side-by-side body, a crossing sweep caught between samples, an
//! empty corridor, a curved boundary, and an exact tie, plus insertion-order
//! independence and the conservative minimum that gates feasibility.
//!
//! The TAS-126 section below compares the production prediction with the
//! clearance the production integrator executes at a preset's own step, under
//! the matrix's `T-O1` bound and fixed horizon, and falsifies an endpoint-only
//! executed fold with a coarse near-pass probe.

use glam::DVec2;
use hekate_model::{CompiledReferencePath, CompiledScenario, FacilityId, parse_scenario_source_v2};
use hekate_sim::{
    AgentId, BodyShape, BoundedSteering, DEFAULT_SUBDIVISIONS, LateralCorridor, LimitingObject,
    ManeuverInputs, PredictedBody, PredictionVerdict, SteeringLimits, SteeringRequest, SweptBody,
    body_clearance_m, bounded_steering_step, predict_maneuver_corridor, tick_minimum_clearance_m,
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

// ---------------------------------------------------------------------------
// TAS-126: production prediction against executed clearance
// ---------------------------------------------------------------------------
//
// The matrix judges the overtaking cell `CC-OVERTAKE` on
// `T-O1 = |predicted - executed minimum clearance| <= 0.10 m`, declared against
// the fine-step executed clearance. These cases run the production predictor at
// a preset's own cadence (`docs/benchmark-matrix.md` §2) over the matrix's fixed
// maneuver-prediction horizon, integrate the same bounded motion through
// `bounded_steering_step` at the preset's step — the integrator the kernel's
// `advance_physics` executes a committed maneuver with — and fold the executed
// corridor at its sampled poses and across its exact swept intervals. The
// reference is the same executed motion integrated at `1/4096 s`.

/// The matrix's `T-O1` bound on `|predicted - executed minimum clearance|` in
/// metres. It is the declared tolerance these cases meet, never widen.
const T_O1_M: f64 = 0.10;

/// The three Phase 2 fidelity presets and their frozen physics steps in
/// seconds, from `docs/benchmark-matrix.md` §2: `(preset, step_s)`.
const PRESETS: [(&str, f64); 3] = [("Fast", 0.100), ("Standard", 0.050), ("Fine", 0.020)];

/// The matrix's fixed maneuver-prediction horizon in seconds: the horizon a
/// version-2 mode template authors for its lateral policy.
const EXECUTED_HORIZON_S: f64 = 4.0;

/// The step of the fine-step executed clearance `T-O1` is declared from, 26
/// times finer than the production fine step, so the reference is converged
/// against every preset.
const REFERENCE_STEP_S: f64 = 1.0 / 4096.0;

/// A circle envelope.
fn circle(centre: DVec2, radius_m: f64) -> BodyShape {
    BodyShape::Circle { centre, radius_m }
}

/// The comparison request of one case at one preset: the case's own target
/// offset, at the preset's cadence and the production subdivision, over the
/// matrix's fixed horizon.
fn matrix_inputs<'a>(
    geometry: &'a CompiledReferencePath,
    target_offset_m: f64,
    step_s: f64,
) -> ManeuverInputs<'a> {
    ManeuverInputs {
        horizon_s: EXECUTED_HORIZON_S,
        cadence_s: step_s,
        subdivisions: DEFAULT_SUBDIVISIONS,
        ..inputs(geometry, target_offset_m)
    }
}

/// One pose of an executed corridor: the world pose the production integrator
/// lands on at `time_s`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ExecutedSample {
    time_s: f64,
    position: DVec2,
    heading_rad: f64,
}

/// The corridor the production execution covers: [`bounded_steering_step`] at
/// `step_s`, from the prediction's own request and held over the horizon.
///
/// This is the coarse side of the comparison: the production prediction
/// subdivides the same cadence by [`DEFAULT_SUBDIVISIONS`], so the two integrate
/// the same bounded motion at different steps.
fn executed_corridor(request: ManeuverInputs<'_>, step_s: f64) -> Vec<ExecutedSample> {
    let heading_rad = match request.body {
        BodyShape::Box { heading_rad, .. } => heading_rad,
        BodyShape::Circle { .. } => 0.0,
    };
    let start = request.body.centre();
    let mut samples = vec![ExecutedSample {
        time_s: 0.0,
        position: start,
        heading_rad,
    }];
    let mut pose = SteeringRequest {
        position: start,
        heading_rad,
        speed_mps: request.speed_mps,
        target_offset_m: request.target_offset_m,
        direction: request.direction,
    };
    let steps = (request.horizon_s / step_s).ceil() as usize;
    for index in 0..steps {
        let step = bounded_steering_step(request.geometry, pose, request.steering, step_s)
            .expect("every executed step of a case stays inside the usable corridor");
        pose.position = step.position;
        pose.heading_rad = step.heading_rad;
        samples.push(ExecutedSample {
            time_s: (index + 1) as f64 * step_s,
            position: step.position,
            heading_rad: step.heading_rad,
        });
    }
    samples
}

/// The body at a world pose: the envelope re-centred and, for a box,
/// re-oriented. This is the predictor's own posing convention.
fn posed_body(body: &BodyShape, position: DVec2, heading_rad: f64) -> BodyShape {
    match *body {
        BodyShape::Circle { radius_m, .. } => BodyShape::Circle {
            centre: position,
            radius_m,
        },
        BodyShape::Box {
            length_m, width_m, ..
        } => BodyShape::Box {
            centre: position,
            heading_rad,
            length_m,
            width_m,
        },
    }
}

/// A shape translated by `offset_m`, keeping its orientation.
fn translated(body: &BodyShape, offset_m: DVec2) -> BodyShape {
    match *body {
        BodyShape::Circle { centre, radius_m } => BodyShape::Circle {
            centre: centre + offset_m,
            radius_m,
        },
        BodyShape::Box {
            centre,
            heading_rad,
            length_m,
            width_m,
        } => BodyShape::Box {
            centre: centre + offset_m,
            heading_rad,
            length_m,
            width_m,
        },
    }
}

/// The envelope's radius along a unit axis: a circle's radius, or a box's
/// projection radius `(length_m / 2) |cos| + (width_m / 2) |sin|` against the
/// axis.
fn projection_radius_m(body: &BodyShape, axis: DVec2) -> f64 {
    match *body {
        BodyShape::Circle { radius_m, .. } => radius_m,
        BodyShape::Box {
            heading_rad,
            length_m,
            width_m,
            ..
        } => {
            let (sin, cos) = heading_rad.sin_cos();
            let forward = DVec2::new(cos, sin);
            let left = DVec2::new(-sin, cos);
            (length_m * 0.5) * forward.dot(axis).abs() + (width_m * 0.5) * left.dot(axis).abs()
        }
    }
}

/// The least signed clearance of one body over a corridor: the exact clearance
/// at every sample and, when `swept`, the exact swept minimum across every step,
/// so a crossing inside one executed step is read rather than missed.
fn body_minimum_over(
    request: ManeuverInputs<'_>,
    samples: &[ExecutedSample],
    body: &PredictedBody,
    swept: bool,
) -> f64 {
    let mut minimum_m = f64::INFINITY;
    for sample in samples {
        let agent = posed_body(&request.body, sample.position, sample.heading_rad);
        let other = translated(&body.shape, body.velocity_mps * sample.time_s);
        minimum_m = minimum_m.min(body_clearance_m(&agent, &other));
    }
    if swept {
        for pair in samples.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let agent = SweptBody {
                shape: posed_body(&request.body, start.position, start.heading_rad),
                displacement_m: end.position - start.position,
            };
            let other = SweptBody {
                shape: translated(&body.shape, body.velocity_mps * start.time_s),
                displacement_m: body.velocity_mps * (end.time_s - start.time_s),
            };
            minimum_m = minimum_m.min(tick_minimum_clearance_m(&agent, &other));
        }
    }
    minimum_m
}

/// The least route-relative band-edge clearance over a corridor, from the
/// facility's compiled constant width.
fn band_edge_minimum_over(request: ManeuverInputs<'_>, samples: &[ExecutedSample]) -> f64 {
    let half_width_m = request.facility_width_m * 0.5;
    let mut minimum_m = f64::INFINITY;
    for sample in samples {
        let coordinate = request.geometry.project(sample.position);
        let normal = request.geometry.normal_at(coordinate.s());
        let envelope = posed_body(&request.body, sample.position, sample.heading_rad);
        let clearance_m =
            half_width_m - coordinate.d().abs() - projection_radius_m(&envelope, normal);
        minimum_m = minimum_m.min(clearance_m);
    }
    minimum_m
}

/// One candidate body of a comparison, with the clearance class and the body
/// pair the case constructs it for.
struct ExecutedBody {
    body: PredictedBody,
    class: &'static str,
    pair: &'static str,
}

/// The least clearance over one corridor: every case body and the band edge.
/// With `swept` false it is the endpoint-only fold an endpoint-only
/// implementation would report.
fn corridor_minimum(
    request: ManeuverInputs<'_>,
    samples: &[ExecutedSample],
    bodies: &[ExecutedBody],
    swept: bool,
) -> f64 {
    let mut minimum_m = band_edge_minimum_over(request, samples);
    for entry in bodies {
        minimum_m = minimum_m.min(body_minimum_over(request, samples, &entry.body, swept));
    }
    minimum_m
}

/// One `T-O1` comparison row: the production predicted minimum against the
/// executed minimum at one preset, with the fine-step reference the declared
/// tolerance derives from. `state` is the maneuver state the prediction reports
/// for the case: the verdict, naming the limiting boundary or agent when the
/// case is infeasible.
#[derive(Debug, Clone)]
struct ClearanceComparison {
    preset: &'static str,
    pair: &'static str,
    class: &'static str,
    state: String,
    predicted_m: f64,
    executed_m: f64,
    reference_m: f64,
}

impl ClearanceComparison {
    /// The `T-O1` quantity: `|predicted - executed|` in metres.
    fn error_m(&self) -> f64 {
        (self.predicted_m - self.executed_m).abs()
    }

    /// Assert the row holds the declared `T-O1` bound on
    /// `|predicted - executed|`, reporting the predicted, executed, reference,
    /// error, preset, pair, and maneuver state on failure.
    fn assert_within_t_o1(&self) {
        assert!(
            self.error_m() <= T_O1_M,
            "T-O1 exceeded: predicted={} executed={} reference={} error={} preset={} pair={} \
             state={}",
            self.predicted_m,
            self.executed_m,
            self.reference_m,
            self.error_m(),
            self.preset,
            self.pair,
            self.state,
        );
    }

    /// Assert the executed minimum is within `T-O1` of the fine-step executed
    /// reference the tolerance is declared from, reporting the same quantities.
    fn assert_reference_frames(&self) {
        let reference_error_m = (self.executed_m - self.reference_m).abs();
        assert!(
            reference_error_m <= T_O1_M,
            "T-O1 reference: predicted={} executed={} reference={} error={} preset={} pair={} \
             state={}",
            self.predicted_m,
            self.executed_m,
            self.reference_m,
            reference_error_m,
            self.preset,
            self.pair,
            self.state,
        );
    }
}

/// The predicted bodies of a case, in declaration order.
fn predicted_bodies(bodies: &[ExecutedBody]) -> Vec<PredictedBody> {
    bodies.iter().map(|entry| entry.body).collect()
}

/// Compare one case's production prediction at one preset against the executed
/// sampled-and-swept minimum and the fine-step reference: one row per reported
/// clearance class, plus the swept minimum the abort decision reads.
fn compare_at_preset(
    preset: &'static str,
    step_s: f64,
    request: ManeuverInputs<'_>,
    bodies: &[ExecutedBody],
) -> Vec<ClearanceComparison> {
    let predicted = predict_maneuver_corridor(request, &predicted_bodies(bodies));
    let executed = executed_corridor(request, step_s);
    let reference = executed_corridor(request, REFERENCE_STEP_S);
    let state = format!("{:?}", predicted.verdict);

    let mut rows = Vec::new();
    for (class, fact) in [
        ("front", predicted.clears.front),
        ("rear", predicted.clears.rear),
        ("side", predicted.clears.side),
    ] {
        let Some(entry) = bodies.iter().find(|entry| entry.class == class) else {
            continue;
        };
        let fact =
            fact.unwrap_or_else(|| panic!("{preset}/{class}: the prediction reports this class"));
        assert_eq!(
            fact.object,
            LimitingObject::Agent(entry.body.id),
            "{preset}/{class}: the predicted limiting body is the case's body"
        );
        rows.push(ClearanceComparison {
            preset,
            pair: entry.pair,
            class,
            state: state.clone(),
            predicted_m: fact.clearance_m,
            executed_m: body_minimum_over(request, &executed, &entry.body, true),
            reference_m: body_minimum_over(request, &reference, &entry.body, true),
        });
    }
    rows.push(ClearanceComparison {
        preset,
        pair: "every body and the band edge",
        class: "swept",
        state,
        predicted_m: predicted.clears.swept.clearance_m,
        executed_m: corridor_minimum(request, &executed, bodies, true),
        reference_m: corridor_minimum(request, &reference, bodies, true),
    });
    rows
}

/// Every row of one case holds `T-O1` and is framed by the fine-step reference.
fn assert_t_o1(rows: &[ClearanceComparison]) {
    for row in rows {
        row.assert_within_t_o1();
        row.assert_reference_frames();
    }
}

/// Case E1: the straight constant-velocity maneuver holds offset zero, so the
/// predicted and executed motions are the same straight line and every class
/// agrees at every preset; the closed-form minima frame both.
#[test]
fn the_straight_case_holds_t_o1_at_every_preset() {
    let geometry = straight();
    let bodies = [
        ExecutedBody {
            body: PredictedBody {
                id: AgentId::from_index(0),
                shape: car(DVec2::new(80.0, 0.5), 0.0),
                velocity_mps: DVec2::new(8.0, 0.0),
            },
            class: "front",
            pair: "box/box",
        },
        ExecutedBody {
            body: PredictedBody {
                id: AgentId::from_index(1),
                shape: circle(DVec2::new(34.0, 0.0), 1.0),
                velocity_mps: DVec2::new(12.0, 0.0),
            },
            class: "rear",
            pair: "box/circle",
        },
        ExecutedBody {
            body: PredictedBody {
                id: AgentId::from_index(2),
                shape: car(DVec2::new(50.0, 2.6), 0.0),
                velocity_mps: DVec2::new(10.0, 0.0),
            },
            class: "side",
            pair: "box/box",
        },
    ];
    for (preset, step_s) in PRESETS {
        let rows = compare_at_preset(
            preset,
            step_s,
            matrix_inputs(&geometry, 0.0, step_s),
            &bodies,
        );
        assert_t_o1(&rows);
        for row in &rows {
            assert!(
                row.predicted_m.is_finite()
                    && row.executed_m.is_finite()
                    && row.reference_m.is_finite(),
                "{preset}/{}: every value is finite: {row:?}",
                row.class
            );
        }
        // The three classified minima are the closed-form gaps, at every step.
        for (class, expected_m) in [("front", 18.0), ("rear", 5.0), ("side", 0.6)] {
            let row = rows
                .iter()
                .find(|row| row.class == class)
                .expect("the case reports this class");
            assert!(
                (row.predicted_m - expected_m).abs() < 1e-9
                    && (row.executed_m - expected_m).abs() < 1e-9,
                "{preset}/{class}: predicted {} executed {} against {expected_m}",
                row.predicted_m,
                row.executed_m
            );
        }
    }
}

/// Case E2: the bounded lateral shift closes on a side body while the agent's
/// heading rotates under the compiled limits, so the preset step is the only
/// source of error between the production prediction and the executed minimum;
/// the shift stays inside the corridor at every preset.
#[test]
fn a_bounded_lateral_shift_holds_t_o1_at_every_preset() {
    let geometry = straight();
    let target_offset_m = 1.5;
    let bodies = [
        ExecutedBody {
            body: PredictedBody {
                id: AgentId::from_index(0),
                shape: car(DVec2::new(80.0, 0.0), 0.0),
                velocity_mps: DVec2::new(8.0, 0.0),
            },
            class: "front",
            pair: "box/box",
        },
        ExecutedBody {
            body: PredictedBody {
                id: AgentId::from_index(1),
                shape: circle(DVec2::new(34.0, 0.0), 1.0),
                velocity_mps: DVec2::new(12.0, 0.0),
            },
            class: "rear",
            pair: "box/circle",
        },
        ExecutedBody {
            body: PredictedBody {
                id: AgentId::from_index(2),
                shape: car(DVec2::new(50.0, 4.0), 0.0),
                velocity_mps: DVec2::new(10.0, 0.0),
            },
            class: "side",
            pair: "box/box",
        },
    ];
    for (preset, step_s) in PRESETS {
        let request = matrix_inputs(&geometry, target_offset_m, step_s);
        let rows = compare_at_preset(preset, step_s, request, &bodies);
        assert_t_o1(&rows);
        // The executed shift reaches the same target and stays inside the
        // usable corridor at every step.
        let executed = executed_corridor(request, step_s);
        let deepest = executed
            .iter()
            .map(|sample| request.geometry.project(sample.position).d())
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            deepest > 1.2 && deepest < 1.5,
            "{preset}: the executed shift reaches {deepest} of the {target_offset_m} m target"
        );
    }
}

/// Case E3: the coarse-endpoint probe. A body passes the holding agent at
/// 200 m/s, so at the coarsest preset's 0.100 s step the whole near-pass falls
/// inside one executed step: the body's own endpoints are 2.85 m clear and the
/// band edge 2.5 m, while the exact swept minimum inside the step is 1.0 m. An
/// endpoint-only executed fold therefore reports 2.5 m against a predicted
/// 1.0 m and breaks `T-O1`, while the sampled-and-swept fold the production
/// prediction uses agrees with the prediction and with the fine-step reference.
/// The probe is repeated at the Standard preset, the coarsest preset an
/// overtaking cell is judged at.
#[test]
fn a_coarse_endpoint_only_fold_is_falsified() {
    let geometry = straight();
    // The probe deliberately compresses the near-pass into one executed step.
    // The body keeps a 2.5 m lateral offset, so the closest approach is a
    // positive 1.0 m that the exact swept read resolves, rather than the
    // overlap boundary a pass straight through the agent's centre would read.
    let bodies = [ExecutedBody {
        body: PredictedBody {
            id: AgentId::from_index(0),
            shape: circle(DVec2::new(45.0, 2.5), 0.5),
            velocity_mps: DVec2::new(200.0, 0.0),
        },
        class: "rear",
        pair: "box/circle",
    }];
    for (preset, step_s) in [("Fast", 0.100), ("Standard", 0.050)] {
        // The agent holds station, so the executed corridor is a single pose and
        // the whole near-pass belongs to the first executed step.
        let request = ManeuverInputs {
            speed_mps: 0.0,
            target_offset_m: 0.0,
            ..matrix_inputs(&geometry, 0.0, step_s)
        };
        let predicted = predict_maneuver_corridor(request, &predicted_bodies(&bodies));
        let executed = executed_corridor(request, step_s);
        let reference = executed_corridor(request, REFERENCE_STEP_S);
        let state = format!("{:?}", predicted.verdict);
        let reference_m = corridor_minimum(request, &reference, &bodies, true);

        let row = |executed_m: f64| ClearanceComparison {
            preset,
            pair: "box/circle",
            class: "swept",
            state: state.clone(),
            predicted_m: predicted.clears.swept.clearance_m,
            executed_m,
            reference_m,
        };
        let endpoint_only = row(corridor_minimum(request, &executed, &bodies, false));
        let sampled_and_swept = row(corridor_minimum(request, &executed, &bodies, true));
        let endpoint_body_m = body_minimum_over(request, &executed, &bodies[0].body, false);

        // The executed endpoints are clear, so the endpoint-only fold reports
        // the clearance at the step's edges and misses the near-pass inside it.
        assert!(
            endpoint_only.executed_m > sampled_and_swept.executed_m,
            "{preset}: the endpoint-only fold must miss the inside-step minimum: endpoints={} \
             swept={}",
            endpoint_only.executed_m,
            sampled_and_swept.executed_m
        );
        // The body's own endpoint fold misses the near-pass by more than the
        // whole tolerance, whatever the band edge contributes.
        assert!(
            (endpoint_body_m - sampled_and_swept.executed_m).abs() > T_O1_M,
            "{preset}: the body's endpoint fold must miss the near-pass: endpoints={endpoint_body_m} \
             swept={}",
            sampled_and_swept.executed_m
        );
        assert!(
            sampled_and_swept.executed_m > 0.0,
            "{preset}: no executed endpoint or step penetrates: {endpoint_only:?} \
             {sampled_and_swept:?}"
        );
        // An endpoint-only implementation misses the inside-step minimum, so its
        // reported executed clearance breaks T-O1 against both the prediction and
        // the fine-step reference ...
        assert!(
            endpoint_only.error_m() > T_O1_M,
            "{preset}: an endpoint-only fold must be falsified: {endpoint_only:?}"
        );
        assert!(
            (endpoint_only.reference_m - endpoint_only.executed_m).abs() > T_O1_M,
            "{preset}: the endpoint-only fold must break T-O1 against the reference: \
             {endpoint_only:?}"
        );
        // ... while the production sampled-and-swept fold holds it.
        sampled_and_swept.assert_within_t_o1();
        sampled_and_swept.assert_reference_frames();
        for value in [
            sampled_and_swept.predicted_m,
            endpoint_only.executed_m,
            sampled_and_swept.executed_m,
            reference_m,
        ] {
            assert!(
                value.is_finite(),
                "{preset}: every value is finite: {value}"
            );
        }
    }
}
