//! TAS-125: analytic and high-resolution clearance reference cases for the
//! maneuver-corridor predictor.
//!
//! Four case families fix reference minima for the front, rear, side, and swept
//! clearance facts of the predictor, each at a declared horizon, the Standard
//! preset's cadence, and the production subdivision:
//!
//! - a **straight constant-velocity** maneuver holds offset zero on a straight
//!   reference, so the corridor is exactly `x = 50 + 10 t` and every minimum is
//!   closed-form;
//! - a **bounded lateral shift** steers 1.5 m across a 7 m band, and its
//!   reference integrates the same bounded motion at `1/4096 s`, 26 times finer
//!   than the production fine step, so the reference is the converged minimum
//!   of the same integral;
//! - a **curved reference** holds an offset 0.6 m outside a 1000 m arc, where
//!   the band edge is the route-relative constant-width boundary and the
//!   reference value is sandwiched by the aligned offset and the chord and
//!   heading-lag drift bounds;
//! - **body-shape pairs** cover `box`/`box`, `box`/`circle`, `circle`/`circle`,
//!   and `circle`/`box`, including a pair that penetrates, so every exact
//!   clearance convention is exercised.
//!
//! The reference is independent of the predictor's own fold: it re-integrates
//! the bounded step itself and folds `body_clearance_m` and
//! `tick_minimum_clearance_m` over the finer corridor, or it evaluates a closed
//! form. Every case asserts that the predictor reproduces the reference, that
//! each reported value is finite, and that a repeated run is bit-identical.
//!
//! Out of scope here: the production prediction against the *executed* minimum
//! at each fidelity preset, and the coarse-endpoint falsification probe, both of
//! which are TAS-126's slice.

use glam::DVec2;
use hekate_model::CompiledReferencePath;
use hekate_sim::{
    AgentId, BodyShape, BoundedSteering, ClearanceFact, DEFAULT_SUBDIVISIONS, LateralCorridor,
    LimitingObject, ManeuverInputs, ManeuverPrediction, PredictedBody, PredictionVerdict,
    SteeringLimits, SteeringRequest, SweptBody, body_clearance_m, bounded_steering_step,
    predict_maneuver_corridor, tick_minimum_clearance_m,
};

/// The reference cases' declared maneuver-prediction horizon in seconds: the
/// horizon a version-2 mode template authors for its lateral policy.
const CASE_HORIZON_S: f64 = 4.0;

/// The Standard fidelity preset's physics step in seconds: the coarsest cadence
/// the benchmark matrix judges an interaction cell at, and the cadence the
/// production predictor subdivides into [`DEFAULT_SUBDIVISIONS`] fine steps.
const MATRIX_CADENCE_S: f64 = 0.050;

/// The tolerance a hand-computed minimum is asserted at.
const EXACT_TOLERANCE_M: f64 = 1e-9;

/// The high-resolution reference step in seconds, `1/4096`: 26 times finer than
/// the production fine step `MATRIX_CADENCE_S / DEFAULT_SUBDIVISIONS` = 6.25 ms.
const REFERENCE_STEP_S: f64 = 1.0 / 4096.0;

/// The lateral separation of the shift case's side body, in metres.
const SHIFT_SIDE_LATERAL_M: f64 = 4.0;

/// The tolerance the bounded lateral shift case's predictor minimum is asserted
/// against its high-resolution reference at, in metres.
///
/// The predictor integrates at the production fine step, 6.25 ms, and the
/// reference at 0.244 ms, so the two differ by the production step's own
/// discretisation of the rate-clamped heading. The coarser step freezes the
/// envelope's heading across the step, which reads up to
/// `(length / 2) * (heading rate cap) * step = 2 * 0.2 rad/s * 6.25 ms = 0.0025
/// m` deeper than the reference, while its rate-clamp lag widens the gap by at
/// most `lateral_accel_max_mps2 * step = 0.0125 m/s` of lateral rate. The
/// benchmark matrix's own bound on a predicted minimum, `T-O1`, is 0.10 m.
const SHIFT_CONVERGENCE_TOLERANCE_M: f64 = 3e-3;

/// The tolerance the curved reference case's predictor minimum is asserted
/// against its high-resolution reference at, in metres.
///
/// The offset a step loses to the reference's curvature is its chord-versus-arc
/// error `(v dt)^2 / (2 R)`, and the production step accumulates
/// `v^2 dt H / (2 R) = 100 * 6.25 ms * 1 s / 2000 = 3.1e-4 m` of it against the
/// reference's own `1.2e-5 m`.
const CURVED_CONVERGENCE_TOLERANCE_M: f64 = 5e-4;

/// The straight reference: 400 m of world `+x`, so the left normal is `+y`.
fn straight() -> CompiledReferencePath {
    CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(400.0, 0.0)])
}

/// The cases' bounded-steering envelope: the fixture's compiled limits
/// (0.9 rad/s, 2.0 m/s²) and a symmetric ±2 m usable corridor.
fn steering() -> BoundedSteering {
    BoundedSteering {
        limits: SteeringLimits {
            heading_rate_max_rad_s: 0.9,
            lateral_accel_max_mps2: 2.0,
        },
        corridor: LateralCorridor {
            d_min: -2.0,
            d_max: 2.0,
        },
    }
}

/// A 4 m by 2 m box envelope.
fn car(centre: DVec2, heading_rad: f64) -> BodyShape {
    BodyShape::Box {
        centre,
        heading_rad,
        length_m: 4.0,
        width_m: 2.0,
    }
}

/// A circle envelope.
fn circle(centre: DVec2, radius_m: f64) -> BodyShape {
    BodyShape::Circle { centre, radius_m }
}

/// The base request of a case: a 4 m by 2 m car at `x = 50` travelling forward
/// at 10 m/s on a 7 m band, clearing 0.5 m, over the declared horizon at the
/// matrix cadence and the production subdivision.
fn inputs<'a>(geometry: &'a CompiledReferencePath, target_offset_m: f64) -> ManeuverInputs<'a> {
    ManeuverInputs {
        geometry,
        facility_width_m: 7.0,
        direction: 1.0,
        body: car(DVec2::new(50.0, 0.0), 0.0),
        speed_mps: 10.0,
        target_offset_m,
        steering: steering(),
        target_clearance_m: 0.5,
        horizon_s: CASE_HORIZON_S,
        cadence_s: MATRIX_CADENCE_S,
        subdivisions: DEFAULT_SUBDIVISIONS,
    }
}

/// The clearance class a case's body belongs to, named by the case so the
/// reference folds each class without a second classifier. The predictor's own
/// class for the body is asserted separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceClass {
    Front,
    Rear,
    Side,
}

/// One body of a case: the predictor's candidate body plus the clearance class
/// the case constructs it for.
#[derive(Debug, Clone, Copy)]
struct CaseBody {
    body: PredictedBody,
    class: ReferenceClass,
}

/// A body ahead of the agent on the same traversal.
fn front(id: usize, shape: BodyShape, velocity_mps: DVec2) -> CaseBody {
    CaseBody {
        body: PredictedBody {
            id: AgentId::from_index(id),
            shape,
            velocity_mps,
        },
        class: ReferenceClass::Front,
    }
}

/// A body behind the agent on the same traversal.
fn rear(id: usize, shape: BodyShape, velocity_mps: DVec2) -> CaseBody {
    CaseBody {
        body: PredictedBody {
            id: AgentId::from_index(id),
            shape,
            velocity_mps,
        },
        class: ReferenceClass::Rear,
    }
}

/// A body abreast of the agent, its progress interval overlapping the agent's.
fn side(id: usize, shape: BodyShape, velocity_mps: DVec2) -> CaseBody {
    CaseBody {
        body: PredictedBody {
            id: AgentId::from_index(id),
            shape,
            velocity_mps,
        },
        class: ReferenceClass::Side,
    }
}

/// The predictor's own view of a case's bodies.
fn predicted(case: &[CaseBody; 3]) -> Vec<PredictedBody> {
    case.iter().map(|entry| entry.body).collect()
}

/// One high-resolution reference sample: an integrated world pose at `time_s`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReferenceSample {
    time_s: f64,
    position: DVec2,
    heading_rad: f64,
}

/// The reference minima of the four clearance classes, in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReferenceMinima {
    front: Option<f64>,
    rear: Option<f64>,
    side: Option<f64>,
    swept: f64,
}

impl ReferenceMinima {
    /// The three classified minima with their class names.
    fn classes(&self) -> [(&'static str, Option<f64>); 3] {
        [
            ("front", self.front),
            ("rear", self.rear),
            ("side", self.side),
        ]
    }

    /// Every reference minimum is finite.
    fn assert_finite(&self) {
        for (class, value) in self.classes() {
            assert!(
                value.is_none_or(f64::is_finite),
                "the {class} reference is not finite: {value:?}"
            );
        }
        assert!(
            self.swept.is_finite(),
            "the swept reference is not finite: {}",
            self.swept
        );
    }
}

/// The high-resolution reference of one case: the fine-step corridor and the
/// minima folded over it.
struct Reference {
    samples: Vec<ReferenceSample>,
    minima: ReferenceMinima,
}

/// Fold the high-resolution reference of one case: the bounded candidate motion
/// integrated at [`REFERENCE_STEP_S`], the exact clearance at every sample and
/// the exact swept minimum across every step for each body, and the
/// route-relative band edge from the band's constant width.
///
/// The integration is repeated to assert it is deterministic too.
fn reference(request: ManeuverInputs<'_>, case: &[CaseBody; 3]) -> Reference {
    let samples = reference_corridor(request);
    assert!(
        reference_corridor(request) == samples,
        "the reference integration is deterministic"
    );

    let mut minima = ReferenceMinima {
        front: None,
        rear: None,
        side: None,
        swept: f64::INFINITY,
    };
    for entry in case {
        let clearance_m = reference_body_minimum(request, &samples, &entry.body);
        let slot = match entry.class {
            ReferenceClass::Front => &mut minima.front,
            ReferenceClass::Rear => &mut minima.rear,
            ReferenceClass::Side => &mut minima.side,
        };
        keep_least(slot, clearance_m);
        minima.swept = minima.swept.min(clearance_m);
    }
    minima.swept = minima
        .swept
        .min(reference_band_edge_minimum(request, &samples));
    minima.assert_finite();
    Reference { samples, minima }
}

/// Integrate the bounded candidate motion at the reference step, starting from
/// the envelope's own centre and heading: a circle's heading is `0.0`, the
/// predictor's own convention.
fn reference_corridor(request: ManeuverInputs<'_>) -> Vec<ReferenceSample> {
    let heading_rad = match request.body {
        BodyShape::Box { heading_rad, .. } => heading_rad,
        BodyShape::Circle { .. } => 0.0,
    };
    let centre = request.body.centre();
    let mut samples = vec![ReferenceSample {
        time_s: 0.0,
        position: centre,
        heading_rad,
    }];
    let mut pose = SteeringRequest {
        position: centre,
        heading_rad,
        speed_mps: request.speed_mps,
        target_offset_m: request.target_offset_m,
        direction: request.direction,
    };
    let steps = (request.horizon_s / REFERENCE_STEP_S).ceil() as usize;
    for index in 0..steps {
        let step =
            bounded_steering_step(request.geometry, pose, request.steering, REFERENCE_STEP_S)
                .expect("every case's bounded candidate stays inside the usable corridor");
        pose.position = step.position;
        pose.heading_rad = step.heading_rad;
        samples.push(ReferenceSample {
            time_s: (index + 1) as f64 * REFERENCE_STEP_S,
            position: step.position,
            heading_rad: step.heading_rad,
        });
    }
    samples
}

/// The least signed clearance of one body over the reference corridor: the
/// exact clearance at every sample and the exact swept minimum across every
/// step, so a crossing inside a step is still read.
fn reference_body_minimum(
    request: ManeuverInputs<'_>,
    samples: &[ReferenceSample],
    body: &PredictedBody,
) -> f64 {
    let mut minimum_m = f64::INFINITY;
    for sample in samples {
        let agent = posed(&request.body, sample.position, sample.heading_rad);
        let other = translated(&body.shape, body.velocity_mps * sample.time_s);
        minimum_m = minimum_m.min(body_clearance_m(&agent, &other));
    }
    for pair in samples.windows(2) {
        let (start, end) = (&pair[0], &pair[1]);
        let agent = SweptBody {
            shape: posed(&request.body, start.position, start.heading_rad),
            displacement_m: end.position - start.position,
        };
        let other = SweptBody {
            shape: translated(&body.shape, body.velocity_mps * start.time_s),
            displacement_m: body.velocity_mps * (end.time_s - start.time_s),
        };
        minimum_m = minimum_m.min(tick_minimum_clearance_m(&agent, &other));
    }
    minimum_m
}

/// The least route-relative band-edge clearance over the reference corridor,
/// from the band's constant width: `width_m / 2 - |d| -` the envelope's radius
/// onto the reference normal at the sample's own arc length.
fn reference_band_edge_minimum(request: ManeuverInputs<'_>, samples: &[ReferenceSample]) -> f64 {
    let half_width_m = request.facility_width_m * 0.5;
    let mut minimum_m = f64::INFINITY;
    for sample in samples {
        let coordinate = request.geometry.project(sample.position);
        let normal = request.geometry.normal_at(coordinate.s());
        let envelope = posed(&request.body, sample.position, sample.heading_rad);
        let clearance_m =
            half_width_m - coordinate.d().abs() - projection_radius_m(&envelope, normal);
        minimum_m = minimum_m.min(clearance_m);
    }
    minimum_m
}

/// The agent envelope at a world pose: a box re-centred and re-oriented, a
/// circle re-centred. This is the predictor's own posing convention.
fn posed(body: &BodyShape, position: DVec2, heading_rad: f64) -> BodyShape {
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

/// Keep the strictly least of an optional slot.
fn keep_least(slot: &mut Option<f64>, value: f64) {
    if slot.is_none_or(|current| value < current) {
        *slot = Some(value);
    }
}

/// Predict one case twice: the second run must reproduce the first exactly, and
/// every reported value must be finite.
fn deterministic_prediction(
    request: ManeuverInputs<'_>,
    bodies: &[PredictedBody],
) -> ManeuverPrediction {
    let first = predict_maneuver_corridor(request, bodies);
    let second = predict_maneuver_corridor(request, bodies);
    assert_eq!(first, second, "the prediction is deterministic");
    for sample in &first.corridor {
        assert!(
            sample.time_s.is_finite()
                && sample.position.is_finite()
                && sample.heading_rad.is_finite()
                && sample.s_m.is_finite()
                && sample.d_m.is_finite(),
            "a corridor sample is finite: {sample:?}"
        );
    }
    for (class, fact) in reported(&first) {
        assert!(
            fact.clearance_m.is_finite()
                && fact.time_s.is_finite()
                && fact.closing_speed_mps.is_finite(),
            "the {class} fact is finite: {fact:?}"
        );
    }
    first
}

/// The reported facts with their class names.
fn reported(prediction: &ManeuverPrediction) -> Vec<(&'static str, ClearanceFact)> {
    let mut facts = Vec::new();
    for (class, fact) in [
        ("front", prediction.clears.front),
        ("rear", prediction.clears.rear),
        ("side", prediction.clears.side),
    ] {
        if let Some(fact) = fact {
            facts.push((class, fact));
        }
    }
    facts.push(("swept", prediction.clears.swept));
    facts
}

/// The clearance of a reported fact, when the case reports that class.
fn minimum(fact: Option<ClearanceFact>) -> Option<f64> {
    fact.map(|fact| fact.clearance_m)
}

/// Assert one reported minimum, or reference minimum, against an expected value.
fn assert_minimum(label: &str, reported_m: Option<f64>, expected_m: f64, tolerance_m: f64) {
    let reported_m = reported_m.unwrap_or_else(|| panic!("{label}: the case reports this minimum"));
    assert!(
        (reported_m - expected_m).abs() <= tolerance_m,
        "{label}: {reported_m} against the expected {expected_m}, error {} beyond {tolerance_m}",
        reported_m - expected_m
    );
}

/// Assert a reported fact names the expected candidate body.
fn assert_object(label: &str, fact: Option<ClearanceFact>, id: usize) {
    let fact = fact.unwrap_or_else(|| panic!("{label}: the case reports this class"));
    assert_eq!(
        fact.object,
        LimitingObject::Agent(AgentId::from_index(id)),
        "{label}: the limiting object"
    );
}

/// Case A: the straight constant-velocity maneuver holds offset zero, so the
/// corridor is exactly `x = 50 + 10 t` and the band edge is the constant
/// `3.5 - 1.0 = 2.5 m`. The least of the bodies and the edge is the side box at
/// `2.6 - 2.0 = 0.6 m`, which is exactly the 0.5 m target clearance plus
/// margin, so the maneuver stays feasible.
#[test]
fn a_straight_constant_velocity_case_has_hand_computable_minima() {
    let geometry = straight();
    let case = [
        // A lead box 26 m ahead, closing at 10 - 8 = 2 m/s: 26 - 8 = 18 m.
        front(0, car(DVec2::new(80.0, 0.5), 0.0), DVec2::new(8.0, 0.0)),
        // A follower 13 m behind, closing at 12 - 10 = 2 m/s: 13 - 8 = 5 m.
        rear(1, circle(DVec2::new(34.0, 0.0), 1.0), DVec2::new(12.0, 0.0)),
        // A box abreast at 2.6 m lateral separation, at the agent's own speed:
        // 2.6 - 1.0 - 1.0 = 0.6 m, held over the whole horizon.
        side(2, car(DVec2::new(50.0, 2.6), 0.0), DVec2::new(10.0, 0.0)),
    ];
    let request = inputs(&geometry, 0.0);
    let prediction = deterministic_prediction(request, &predicted(&case));
    let reference = reference(request, &case);

    assert_minimum(
        "front",
        minimum(prediction.clears.front),
        18.0,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "front reference",
        reference.minima.front,
        18.0,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "rear",
        minimum(prediction.clears.rear),
        5.0,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "rear reference",
        reference.minima.rear,
        5.0,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "side",
        minimum(prediction.clears.side),
        0.6,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "side reference",
        reference.minima.side,
        0.6,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "swept",
        minimum(Some(prediction.clears.swept)),
        0.6,
        EXACT_TOLERANCE_M,
    );
    assert_minimum(
        "swept reference",
        Some(reference.minima.swept),
        0.6,
        EXACT_TOLERANCE_M,
    );
    assert_object("front", prediction.clears.front, 0);
    assert_object("rear", prediction.clears.rear, 1);
    assert_object("side", prediction.clears.side, 2);
    assert_object("swept", Some(prediction.clears.swept), 2);
    assert_eq!(prediction.verdict, PredictionVerdict::Feasible);

    // The least clearance of a closing pair is at the end of the horizon, and
    // the side pair keeps formation, so its minimum is the current instant.
    let front = prediction.clears.front.expect("a body is ahead");
    let rear = prediction.clears.rear.expect("a body is behind");
    let side = prediction.clears.side.expect("a body is alongside");
    assert!(
        (front.time_s - CASE_HORIZON_S).abs() <= EXACT_TOLERANCE_M
            && (rear.time_s - CASE_HORIZON_S).abs() <= EXACT_TOLERANCE_M
            && side.time_s.abs() <= EXACT_TOLERANCE_M,
        "times: front {} rear {} side {}",
        front.time_s,
        rear.time_s,
        side.time_s
    );
    assert!(
        (front.closing_speed_mps - 2.0).abs() <= EXACT_TOLERANCE_M
            && (rear.closing_speed_mps - 2.0).abs() <= EXACT_TOLERANCE_M
            && side.closing_speed_mps.abs() <= EXACT_TOLERANCE_M,
        "closing speeds: front {} rear {} side {}",
        front.closing_speed_mps,
        rear.closing_speed_mps,
        side.closing_speed_mps
    );
}

/// Case B: the bounded lateral shift steers 1.5 m toward the side body on a
/// 7 m band. The shift is monotone, so the least side clearance is the lateral
/// gap at the deepest offset the bounded motion reaches, which gives the side
/// reference a closed form beside the fine-step reference; the front and rear
/// gaps close with the rotation slack of the agent's own heading.
#[test]
fn a_bounded_lateral_shift_case_matches_a_high_resolution_reference() {
    let geometry = straight();
    let case = [
        // A lead box 26 m ahead, closing at 2 m/s: 26 - 8 = 18 m before the
        // agent's own heading rotation is read, which only shortens the gap.
        front(0, car(DVec2::new(80.0, 0.0), 0.0), DVec2::new(8.0, 0.0)),
        // A follower 13 m behind, closing at 2 m/s: 13 - 8 = 5 m likewise.
        rear(1, circle(DVec2::new(34.0, 0.0), 1.0), DVec2::new(12.0, 0.0)),
        // A box abreast at 4 m, which the shift closes on: the start gap is
        // `4.0 - 2.0 = 2.0 m` and the least gap is at the deepest offset.
        side(
            2,
            car(DVec2::new(50.0, SHIFT_SIDE_LATERAL_M), 0.0),
            DVec2::new(10.0, 0.0),
        ),
    ];
    let request = inputs(&geometry, 1.5);
    let prediction = deterministic_prediction(request, &predicted(&case));
    let reference = reference(request, &case);

    // The heading rotates by at most the desired error at offset zero,
    // `asin(1.5 / 2 / 10) = 0.0751 rad`. That both tilts the longitudinal
    // advance, `(1 - cos 0.0751) v H = 0.112 m` less progress, which widens the
    // same-lane gap, and rotates the envelope,
    // `(4.0 / 2) sin(0.0751) + (2.0 / 2)(1 - cos(0.0751)) = 0.153 m` more radius,
    // which narrows it, so the closed-form in-lane gap frames the reference
    // within that slack.
    const ROTATION_SLACK_M: f64 = 0.16;
    let front = reference.minima.front.expect("a front reference");
    let rear = reference.minima.rear.expect("a rear reference");
    assert!(
        (front - 18.0).abs() <= ROTATION_SLACK_M,
        "front reference {front} outside 18.0 ± {ROTATION_SLACK_M}"
    );
    assert!(
        (rear - 5.0).abs() <= ROTATION_SLACK_M,
        "rear reference {rear} outside 5.0 ± {ROTATION_SLACK_M}"
    );

    // The side body's reference minimum is hand-computable at the deepest
    // reference sample: the shift closes on the body, so the two boxes still
    // overlap along the reference tangent and the exact clearance is the lateral
    // gap between the rotated agent envelope and the body. The reference's swept
    // fold freezes the envelope's heading across each step, so it reads up to
    // the envelope's growth over one step, `(4.0 / 2) |heading rate| step =
    // 2 * 0.2 rad/s / 4096 s = 9.8e-5 m`, on either side of that sampled gap.
    const FROZEN_SWEEP_SLACK_M: f64 = 1e-4;
    let deepest = reference
        .samples
        .iter()
        .max_by(|first, second| {
            let first_d = request.geometry.project(first.position).d();
            let second_d = request.geometry.project(second.position).d();
            first_d.total_cmp(&second_d)
        })
        .expect("the reference corridor has samples");
    let deepest_d = request.geometry.project(deepest.position).d();
    let deepest_s = request.geometry.project(deepest.position).s();
    let envelope_radius_m = projection_radius_m(
        &posed(&request.body, deepest.position, deepest.heading_rad),
        request.geometry.normal_at(deepest_s),
    );
    let closed_form_m = (SHIFT_SIDE_LATERAL_M - deepest_d) - (1.0 + envelope_radius_m);
    assert_minimum(
        "side reference",
        reference.minima.side,
        closed_form_m,
        FROZEN_SWEEP_SLACK_M,
    );
    assert!(
        deepest_d > 1.2 && deepest_d < 1.5,
        "the shift reaches {deepest_d} of the 1.5 m target offset"
    );

    // The predictor reproduces the reference on every class, and the swept
    // minimum is the side body the shift closes on.
    assert_minimum(
        "front convergence",
        minimum(prediction.clears.front),
        front,
        SHIFT_CONVERGENCE_TOLERANCE_M,
    );
    assert_minimum(
        "rear convergence",
        minimum(prediction.clears.rear),
        rear,
        SHIFT_CONVERGENCE_TOLERANCE_M,
    );
    assert_minimum(
        "side convergence",
        minimum(prediction.clears.side),
        reference.minima.side.expect("a side reference"),
        SHIFT_CONVERGENCE_TOLERANCE_M,
    );
    assert_minimum(
        "swept convergence",
        minimum(Some(prediction.clears.swept)),
        reference.minima.swept,
        SHIFT_CONVERGENCE_TOLERANCE_M,
    );
    assert_object("side", prediction.clears.side, 2);
    assert_object("swept", Some(prediction.clears.swept), 2);
    assert_eq!(prediction.verdict, PredictionVerdict::Feasible);
}

/// Case C: the curved reference holds an offset 0.6 m outside a 1000 m arc, so
/// the band edge is the route-relative constant-width boundary and it binds.
/// The reference integrates the bounded motion at the reference step; the value
/// is sandwiched by the aligned offset `1.75 - 0.6 - 1.0 = 0.15 m` above and the
/// chord and heading-lag drift bounds below.
#[test]
fn a_curved_reference_case_matches_a_high_resolution_reference() {
    let radius_m = 1000.0;
    let horizon_s = 1.0;
    let geometry = CompiledReferencePath::arc(
        DVec2::new(0.0, radius_m),
        radius_m,
        -0.5 * std::f64::consts::PI,
        0.2,
    );
    // The agent rides 0.6 m outside the curve at 30 m of arc length, so a rear
    // body has reference behind it and the outward drift closes on the outer
    // band edge.
    let body = car(geometry.point_at(30.0, -0.6), geometry.heading_at(30.0));
    let request = ManeuverInputs {
        facility_width_m: 3.5,
        body,
        // Hold the offset: the candidate steers back to its current offset.
        target_offset_m: -0.6,
        horizon_s,
        ..inputs(&geometry, 0.0)
    };
    let body_velocity = |s_m: f64| DVec2::from_angle(geometry.heading_at(s_m)) * 10.0;
    let case = [
        front(
            0,
            car(geometry.point_at(50.0, -0.6), geometry.heading_at(50.0)),
            body_velocity(50.0),
        ),
        rear(
            1,
            car(geometry.point_at(15.0, -0.6), geometry.heading_at(15.0)),
            body_velocity(15.0),
        ),
        side(
            2,
            car(geometry.point_at(30.0, 2.8), geometry.heading_at(30.0)),
            body_velocity(30.0),
        ),
    ];
    let prediction = deterministic_prediction(request, &predicted(&case));
    let reference = reference(request, &case);

    // The aligned band edge is 0.15 m; the minimum can only fall below it. The
    // outward chord drift of one horizon is at most
    // `(v t)^2 / (2 R) = 10^2 / 2000 = 0.05 m` of offset, and the heading lag
    // against the rotated tangent is at most `v t / R = 0.01 rad`, which adds at
    // most `(4.0 / 2) sin(0.01) + (2.0 / 2)(1 - cos(0.01)) = 0.020 m` to the
    // envelope's radius along the normal.
    const CHORD_DRIFT_M: f64 = 0.05;
    const HEADING_LAG_M: f64 = 0.02;
    let band_edge = reference.minima.swept;
    let band_edges_m = 0.15 - CHORD_DRIFT_M - HEADING_LAG_M..=0.15 + EXACT_TOLERANCE_M;
    assert!(
        band_edges_m.contains(&band_edge),
        "the curved band edge {band_edge} outside {band_edges_m:?}"
    );
    assert_minimum(
        "curved band edge convergence",
        minimum(Some(prediction.clears.swept)),
        band_edge,
        CURVED_CONVERGENCE_TOLERANCE_M,
    );
    for (class, reported_m, reference_m) in [
        (
            "curved front convergence",
            minimum(prediction.clears.front),
            reference.minima.front,
        ),
        (
            "curved rear convergence",
            minimum(prediction.clears.rear),
            reference.minima.rear,
        ),
        (
            "curved side convergence",
            minimum(prediction.clears.side),
            reference.minima.side,
        ),
    ] {
        assert_minimum(
            class,
            reported_m,
            reference_m.expect("the case reports this reference minimum"),
            CURVED_CONVERGENCE_TOLERANCE_M,
        );
    }
    assert_object("front", prediction.clears.front, 0);
    assert_object("rear", prediction.clears.rear, 1);
    assert_object("side", prediction.clears.side, 2);
    assert_eq!(
        prediction.clears.swept.object,
        LimitingObject::BandEdge,
        "the curved band edge is the swept object"
    );
    assert_eq!(
        prediction.verdict,
        PredictionVerdict::Infeasible {
            limiting: LimitingObject::BandEdge
        }
    );
}

/// Case D1: a box agent against a box lead, a circle follower, and a circle
/// abreast at exactly the target clearance. The maneuver holds offset zero, so
/// every minimum is closed-form and the pair is exactly feasible.
#[test]
fn a_box_agent_shape_pair_case_has_hand_computable_minima() {
    let geometry = straight();
    let case = [
        // A box lead 26 m ahead, closing at 2 m/s: 26 - 8 = 18 m.
        front(0, car(DVec2::new(80.0, 0.5), 0.0), DVec2::new(8.0, 0.0)),
        // A circle follower 12.5 m behind, closing at 2 m/s: 12.5 - 8 = 4.5 m.
        rear(1, circle(DVec2::new(35.0, 0.0), 0.5), DVec2::new(12.0, 0.0)),
        // A circle abreast: 2.0 - 1.0 - 0.5 = 0.5 m, exactly the target.
        side(2, circle(DVec2::new(50.0, 2.0), 0.5), DVec2::new(10.0, 0.0)),
    ];
    let request = inputs(&geometry, 0.0);
    let prediction = deterministic_prediction(request, &predicted(&case));
    let reference = reference(request, &case);

    for (class, reported_m, expected_m) in [
        ("front", minimum(prediction.clears.front), 18.0),
        ("rear", minimum(prediction.clears.rear), 4.5),
        ("side", minimum(prediction.clears.side), 0.5),
        ("swept", minimum(Some(prediction.clears.swept)), 0.5),
    ] {
        assert_minimum(class, reported_m, expected_m, EXACT_TOLERANCE_M);
    }
    for (class, reference_m, expected_m) in [
        ("front reference", reference.minima.front, 18.0),
        ("rear reference", reference.minima.rear, 4.5),
        ("side reference", reference.minima.side, 0.5),
        ("swept reference", Some(reference.minima.swept), 0.5),
    ] {
        assert_minimum(class, reference_m, expected_m, EXACT_TOLERANCE_M);
    }
    assert_object("front", prediction.clears.front, 0);
    assert_object("rear", prediction.clears.rear, 1);
    assert_object("side", prediction.clears.side, 2);
    // The side pair is exactly at the target, and feasibility requires only
    // that no fact is below it.
    assert_eq!(prediction.verdict, PredictionVerdict::Feasible);
}

/// Case D2: a circle agent against a circle lead, a box follower, and a box
/// abreast that the agent penetrates. The maneuver holds offset zero, so every
/// minimum is closed-form, and the negative side clearance fixes the swept fact
/// and the verdict.
#[test]
fn a_circle_agent_shape_pair_case_has_hand_computable_minima() {
    let geometry = straight();
    let case = [
        // A circle lead 23 m ahead, closing at 8 - 4 = 4 m/s: 23 - 16 = 7 m.
        front(0, circle(DVec2::new(75.0, 0.0), 1.0), DVec2::new(4.0, 0.0)),
        // A box follower 17 m behind, closing at 4 m/s: 17 - 16 = 1 m.
        rear(1, car(DVec2::new(30.0, 0.0), 0.0), DVec2::new(12.0, 0.0)),
        // A box abreast whose near face is 0.9 m from the agent's centre, which
        // is 1.0 m inside its own surface: -0.1 m of penetration.
        side(2, car(DVec2::new(50.0, 1.9), 0.0), DVec2::new(8.0, 0.0)),
    ];
    let request = ManeuverInputs {
        body: circle(DVec2::new(50.0, 0.0), 1.0),
        speed_mps: 8.0,
        ..inputs(&geometry, 0.0)
    };
    let prediction = deterministic_prediction(request, &predicted(&case));
    let reference = reference(request, &case);

    for (class, reported_m, expected_m) in [
        ("front", minimum(prediction.clears.front), 7.0),
        ("rear", minimum(prediction.clears.rear), 1.0),
        ("side", minimum(prediction.clears.side), -0.1),
        ("swept", minimum(Some(prediction.clears.swept)), -0.1),
    ] {
        assert_minimum(class, reported_m, expected_m, EXACT_TOLERANCE_M);
    }
    for (class, reference_m, expected_m) in [
        ("front reference", reference.minima.front, 7.0),
        ("rear reference", reference.minima.rear, 1.0),
        ("side reference", reference.minima.side, -0.1),
        ("swept reference", Some(reference.minima.swept), -0.1),
    ] {
        assert_minimum(class, reference_m, expected_m, EXACT_TOLERANCE_M);
    }
    assert_object("side", prediction.clears.side, 2);
    assert_object("swept", Some(prediction.clears.swept), 2);
    assert_eq!(
        prediction.verdict,
        PredictionVerdict::Infeasible {
            limiting: LimitingObject::Agent(AgentId::from_index(2))
        }
    );
}
