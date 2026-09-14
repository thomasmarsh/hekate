//! Pure finite-horizon prediction of a lateral maneuver corridor and its
//! front, rear, side, and swept clearances.
//!
//! `PHASE_2_PLAN.md` "Continuous lateral motion" requires gap acceptance to
//! check predicted front, rear, side, and swept-envelope clearance over the
//! maneuver horizon, and `docs/schema-v2-contract.md` *Increment 2 additions*
//! fixes the exact vocabulary: the **usable corridor** (a reachability set the
//! tactical stage checks before it prepares), the **predicted corridor** (the
//! swept region the candidate maneuver's envelope would occupy over the
//! horizon), and four **clearance facts**, each a signed surface clearance in
//! metres with the same convention as
//! [`body_clearance_m`](crate::body_clearance_m) and
//! [`CONTACT_EPSILON_M`](crate::CONTACT_EPSILON_M).
//!
//! This module owns that prediction. It is a pure, deterministic query: it
//! reads compiled geometry, the current bodies and velocities, one bounded
//! candidate motion, a body envelope, a target clearance, and a horizon, and it
//! returns feasibility plus the limiting object and the four facts. It never
//! mutates state, chooses a tactic, or commits anything.
//!
//! # Model card
//!
//! ## Inputs
//!
//! [`ManeuverInputs`] carries the compiled reference geometry and the facility
//! band width, the agent's travel direction, its current world body envelope
//! ([`BodyShape`]), its longitudinal speed, the candidate maneuver's target
//! signed offset (in the agent's own travel frame), the compiled bounded
//! steering envelope ([`BoundedSteering`](crate::BoundedSteering)), the mode's
//! target clearance, the horizon, the run's lateral-decision cadence, and a
//! finite-subdivision count. The candidate bodies are a separate slice of
//! [`PredictedBody`], each a stable [`AgentId`], an exact [`BodyShape`], and a
//! world velocity in metres per second.
//!
//! ## Candidate motion and the predicted corridor
//!
//! The candidate motion is integrated with
//! [`bounded_steering_step`](crate::bounded_steering_step) at a fine step of
//! `cadence_s / subdivisions`, starting from the current pose and steering
//! toward the target offset. Each integrated pose is one [`CorridorSample`];
//! the sequence is the predicted corridor, in world and route coordinates, and
//! it covers the whole transition, not only the two endpoint lane centres. A
//! step that would leave the compiled usable corridor ends the integration: the
//! candidate is infeasible on the boundary, and the corridor holds the feasible
//! poses up to that point.
//!
//! ## Body classes
//!
//! A body's **progress interval** is its current longitudinal footprint in
//! route coordinates: its projected arc length plus and minus its own
//! half-extent along the local reference tangent. A body whose interval is
//! strictly ahead of the agent's is a **front** body, one strictly behind is a
//! **rear** body, and one whose interval overlaps the agent's is a **side**
//! body — the side-by-side case. Classification reads the current instant; each
//! fact then takes the minimum clearance over the whole horizon.
//!
//! ## Clearance facts
//!
//! For each body the minimum signed clearance over the horizon is the least of
//! the exact clearances at every corridor sample and the swept-interval minimum
//! [`tick_minimum_clearance_m`](crate::tick_minimum_clearance_m) between the
//! agent's swept body and the body's linear sweep across each fine step, so a
//! crossing that happens between two samples is still read. The **band edge**
//! fact is the least distance from the agent's outer envelope to either side of
//! the facility band over the corridor, computed in the facility's own
//! route-relative frame (`width_m / 2 - |d| - envelope half-extent along the
//! reference normal`), so it applies on a curved reference exactly as the
//! compiled constant-width band defines it.
//!
//! `front` is the least front-body clearance, `rear` the least rear-body
//! clearance, `side` the least side-body clearance, and `swept` the least of
//! every body and the band edge. Because the classes partition the bodies,
//! `swept` is never greater than any of the other three, and it is the value the
//! abort decision reads. A class with no body reports no fact (`None`); the band
//! edge is always a fact, so `swept` is always present.
//!
//! ## Feasibility
//!
//! The candidate is feasible when every fact stays at or above the target
//! clearance and the bounded motion never left the usable corridor. Otherwise
//! the verdict names the limiting boundary or agent: the fact's own limiting
//! object when a clearance is below target, and the band edge when the corridor
//! was exceeded and no fact is lower.
//!
//! ## Candidate collection and determinism
//!
//! Candidates come from the ordinary [`BroadPhase`](crate::BroadPhase): the
//! grid is rebuilt from the candidate bodies in any input order and queried with
//! the axis-aligned box of the whole predicted corridor, widened by the largest
//! candidate displacement over the horizon (the query widens again by the
//! largest circumradius), so no body that could reach the corridor is missed.
//! The query returns ascending [`AgentId`] order, and the facts are folded over
//! that order with a strict `<`, so the reported limiting object and every value
//! are a pure function of the bodies and not of insertion, declaration, or
//! discovery order; an exact tie is won by the lower [`AgentId`], and the band
//! edge only wins a strict minimum.
//!
//! ## Conservatism
//!
//! Feasibility is defined by the same fine subdivision the prediction reports,
//! so the result can never be feasible while a minimum it computed is below
//! target. The subdivision reads a whole interval with an exact swept query
//! rather than sampling its endpoints, so a crossing between two samples is
//! caught, and a conservative result rejects a marginal gap rather than
//! accepting one.

use glam::DVec2;
use tangle_model::CompiledReferencePath;

use crate::agent::AgentId;
use crate::index::BroadPhase;
use crate::metrics::tick_minimum_clearance_m;
use crate::query::{Aabb, BodyShape, body_clearance_m, body_contact_normal};
use crate::steering::{BoundedSteering, SteeringRequest, bounded_steering_step};
use crate::swept::SweptBody;

/// The largest number of fine steps one prediction integrates, a guard against
/// a degenerate cadence. The run's cadence and horizon keep a real maneuver far
/// below it.
pub const MAX_PREDICTION_STEPS: usize = 100_000;

/// The subdivision the fixtures and a default caller use per decision cadence,
/// matching the "subdivided finely for the clearance minimum" contract
/// language.
pub const DEFAULT_SUBDIVISIONS: usize = 8;

/// One world body a candidate maneuver must clear: a stable identifier, its
/// exact shape, and its world velocity over the horizon.
///
/// The bodies are the *other* agents, in any order; the querying agent's own
/// body is not one of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedBody {
    /// Stable identifier used for ordering and as the reported limiting object.
    pub id: AgentId,
    /// Exact body shape at the current instant.
    pub shape: BodyShape,
    /// World velocity in metres per second, held constant over the horizon.
    pub velocity_mps: DVec2,
}

/// The compiled geometry and bounded candidate motion of one lateral maneuver.
///
/// See the module card for how each field enters the prediction.
#[derive(Debug, Clone, Copy)]
pub struct ManeuverInputs<'a> {
    /// The compiled reference the maneuver rides.
    pub geometry: &'a CompiledReferencePath,
    /// Full facility band width across the reference, in metres.
    pub facility_width_m: f64,
    /// The agent's longitudinal travel sign: `1.0` forward along the reference,
    /// `-1.0` reverse.
    pub direction: f64,
    /// The agent's current world body envelope.
    pub body: BodyShape,
    /// The agent's current longitudinal speed in metres per second.
    pub speed_mps: f64,
    /// The candidate maneuver's target signed offset in metres, in the agent's
    /// own travel frame.
    pub target_offset_m: f64,
    /// The compiled bounded-steering envelope of the agent.
    pub steering: BoundedSteering,
    /// The mode's target clearance in metres.
    pub target_clearance_m: f64,
    /// The prediction horizon in seconds.
    pub horizon_s: f64,
    /// The run's lateral-decision cadence in seconds.
    pub cadence_s: f64,
    /// Fine subdivisions per cadence interval; at least one.
    pub subdivisions: usize,
}

/// The object a clearance minimum is measured against: a candidate body's
/// [`AgentId`] or the facility band edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitingObject {
    /// The candidate body with this stable identifier.
    Agent(AgentId),
    /// The facility band boundary.
    BandEdge,
}

/// One predicted clearance fact: the minimum signed clearance in metres over
/// the horizon, the object that produced it, the time of the minimum in
/// seconds from now, and the closing speed along the minimum's axis.
///
/// The closing speed is the rate at which the signed clearance is falling; it
/// is positive when the gap is closing and negative when it is opening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearanceFact {
    /// Minimum signed surface clearance in metres.
    pub clearance_m: f64,
    /// The body or band edge that produced the minimum.
    pub object: LimitingObject,
    /// Time of the minimum in seconds from now, in `[0, horizon_s]`.
    pub time_s: f64,
    /// Closing speed along the minimum's axis in metres per second.
    pub closing_speed_mps: f64,
}

/// The four predicted clearance facts of one candidate maneuver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedClearances {
    /// Least clearance to a body ahead on the same traversal, or `None` when
    /// no body is ahead.
    pub front: Option<ClearanceFact>,
    /// Least clearance to a body behind on the same traversal, or `None` when
    /// no body is behind.
    pub rear: Option<ClearanceFact>,
    /// Least clearance to a side-by-side body whose progress interval overlaps
    /// the agent's, or `None` when no body does.
    pub side: Option<ClearanceFact>,
    /// Least clearance over the whole sweep, against every candidate body and
    /// the facility band edge.
    pub swept: ClearanceFact,
}

/// One sampled pose of the predicted corridor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorridorSample {
    /// Time of the sample in seconds from now.
    pub time_s: f64,
    /// World position in metres.
    pub position: DVec2,
    /// World heading in radians.
    pub heading_rad: f64,
    /// Reference arc length in metres.
    pub s_m: f64,
    /// Signed lateral offset in metres, in the agent's own travel frame.
    pub d_m: f64,
}

/// The feasibility verdict of one candidate maneuver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionVerdict {
    /// Every fact stayed at or above the target clearance and the bounded
    /// motion stayed inside the usable corridor.
    Feasible,
    /// The maneuver is rejected, limited by this boundary or agent.
    Infeasible {
        /// The band edge or body that limits the maneuver.
        limiting: LimitingObject,
    },
}

/// The full result of one maneuver prediction.
#[derive(Debug, Clone, PartialEq)]
pub struct ManeuverPrediction {
    /// The predicted corridor: the candidate poses in time order, at least the
    /// current pose.
    pub corridor: Vec<CorridorSample>,
    /// The front, rear, side, and swept clearance facts.
    pub clears: PredictedClearances,
    /// Whether the maneuver is feasible and, when not, its limiting object.
    pub verdict: PredictionVerdict,
}

impl ManeuverPrediction {
    /// Whether the maneuver is feasible.
    pub fn is_feasible(&self) -> bool {
        matches!(self.verdict, PredictionVerdict::Feasible)
    }
}

/// Predict the corridor and clearances of one candidate lateral maneuver.
///
/// Pure and deterministic: the result is a function of `inputs` and the set of
/// `bodies`, independent of the order the bodies are supplied in. See the
/// module card for the model and the conservatism guarantee.
pub fn predict_maneuver_corridor(
    inputs: ManeuverInputs<'_>,
    bodies: &[PredictedBody],
) -> ManeuverPrediction {
    let (corridor, corridor_exceeded) = integrate_corridor(&inputs);
    let bounds = corridor_bounds(&inputs, &corridor, bodies);
    let candidates = collect_candidates(bodies, bounds);

    let mut front: Option<ClearanceFact> = None;
    let mut rear: Option<ClearanceFact> = None;
    let mut side: Option<ClearanceFact> = None;
    let mut swept: Option<ClearanceFact> = None;

    for candidate in &candidates {
        let fact = body_fact(&inputs, &corridor, candidate);
        // Fold each class with a strict `<`, so an exact tie keeps the first
        // candidate in ascending `AgentId` order.
        match classify_body(&inputs, candidate) {
            BodyClass::Front => keep_least(&mut front, fact),
            BodyClass::Rear => keep_least(&mut rear, fact),
            BodyClass::Side => keep_least(&mut side, fact),
        }
        keep_least(&mut swept, fact);
    }

    // The band edge is always a fact, and it only wins a strict minimum, so a
    // body at exactly the band-edge clearance keeps its identifier.
    let edge = band_edge_fact(&inputs, &corridor);
    keep_least(&mut swept, Some(edge));
    let swept = swept.expect("the band edge always supplies a swept fact");

    let verdict = if swept.clearance_m < inputs.target_clearance_m {
        PredictionVerdict::Infeasible {
            limiting: swept.object,
        }
    } else if corridor_exceeded {
        PredictionVerdict::Infeasible {
            limiting: edge.object,
        }
    } else {
        PredictionVerdict::Feasible
    };

    ManeuverPrediction {
        corridor,
        clears: PredictedClearances {
            front,
            rear,
            side,
            swept,
        },
        verdict,
    }
}

/// The classification of one candidate body by its current progress footprint
/// relative to the agent's.
enum BodyClass {
    Front,
    Rear,
    Side,
}

/// Integrate the candidate motion at the fine step, returning the sampled
/// corridor and whether a bounded step was rejected by the usable corridor.
fn integrate_corridor(inputs: &ManeuverInputs<'_>) -> (Vec<CorridorSample>, bool) {
    let start_position = inputs.body.centre();
    let start_heading = match inputs.body {
        BodyShape::Box { heading_rad, .. } => heading_rad,
        BodyShape::Circle { .. } => 0.0,
    };

    let mut corridor = Vec::new();
    corridor.push(sample_of(
        inputs.geometry,
        inputs.direction,
        0.0,
        start_position,
        start_heading,
    ));

    let fine_step = fine_step_s(inputs);
    let steps = ((inputs.horizon_s / fine_step).ceil() as usize).min(MAX_PREDICTION_STEPS);
    if steps == 0 {
        return (corridor, false);
    }

    let mut request = SteeringRequest {
        position: start_position,
        heading_rad: start_heading,
        speed_mps: inputs.speed_mps,
        target_offset_m: inputs.target_offset_m,
        direction: inputs.direction,
    };

    for index in 0..steps {
        match bounded_steering_step(inputs.geometry, request, inputs.steering, fine_step) {
            Some(step) => {
                request.position = step.position;
                request.heading_rad = step.heading_rad;
                corridor.push(sample_of(
                    inputs.geometry,
                    inputs.direction,
                    (index + 1) as f64 * fine_step,
                    step.position,
                    step.heading_rad,
                ));
            }
            None => return (corridor, true),
        }
    }
    (corridor, false)
}

/// The fine integration step in seconds: the cadence divided by the
/// subdivision count, falling back to the horizon when either is degenerate.
fn fine_step_s(inputs: &ManeuverInputs<'_>) -> f64 {
    let subdivisions = inputs.subdivisions.max(1) as f64;
    let step = inputs.cadence_s / subdivisions;
    if step.is_finite() && step > 0.0 {
        step
    } else {
        inputs.horizon_s.max(f64::MIN_POSITIVE)
    }
}

/// One corridor sample at `time_s` for a world pose.
fn sample_of(
    geometry: &CompiledReferencePath,
    direction: f64,
    time_s: f64,
    position: DVec2,
    heading_rad: f64,
) -> CorridorSample {
    let coordinate = geometry.project(position);
    CorridorSample {
        time_s,
        position,
        heading_rad,
        s_m: coordinate.s(),
        d_m: coordinate.d() * if direction < 0.0 { -1.0 } else { 1.0 },
    }
}

/// The axis-aligned box of the whole predicted corridor, widened by the largest
/// candidate displacement over the horizon.
///
/// The candidate query widens again by the largest indexed circumradius, so the
/// box and that widening together bound every body that could reach the
/// corridor, and no candidate is missed.
fn corridor_bounds(
    inputs: &ManeuverInputs<'_>,
    corridor: &[CorridorSample],
    bodies: &[PredictedBody],
) -> Aabb {
    let mut min = DVec2::splat(f64::INFINITY);
    let mut max = DVec2::splat(f64::NEG_INFINITY);
    for sample in corridor {
        let bounds = pose_body(&inputs.body, sample.position, sample.heading_rad).bounds();
        min = min.min(bounds.min);
        max = max.max(bounds.max);
    }
    if !min.x.is_finite() {
        // No samples: fall back to the current body bounds.
        let bounds = inputs.body.bounds();
        min = bounds.min;
        max = bounds.max;
    }
    let max_displacement = bodies
        .iter()
        .map(|candidate| candidate.velocity_mps.length() * inputs.horizon_s)
        .fold(0.0, f64::max);
    Aabb::new(min, max).expand(max_displacement)
}

/// The candidate bodies the corridor could reach, in ascending [`AgentId`]
/// order, by the ordinary broad phase.
fn collect_candidates(bodies: &[PredictedBody], bounds: Aabb) -> Vec<&PredictedBody> {
    if bodies.is_empty() {
        return Vec::new();
    }
    let shapes: Vec<(AgentId, BodyShape)> = bodies
        .iter()
        .map(|candidate| (candidate.id, candidate.shape))
        .collect();
    let mut phase = BroadPhase::default();
    phase.rebuild(&shapes);

    let mut ids = Vec::new();
    phase.candidates_overlapping(bounds, &mut ids);

    let mut ordered: Vec<&PredictedBody> = bodies.iter().collect();
    ordered.sort_by_key(|candidate| candidate.id);
    let mut found = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(position) = ordered.binary_search_by_key(&id, |candidate| candidate.id) {
            found.push(ordered[position]);
        }
    }
    found
}

/// The class of one candidate from its current progress footprint relative to
/// the agent's.
fn classify_body(inputs: &ManeuverInputs<'_>, candidate: &PredictedBody) -> BodyClass {
    let agent = progress_footprint(inputs.geometry, &inputs.body, inputs.body.centre());
    let other = progress_footprint(inputs.geometry, &candidate.shape, candidate.shape.centre());
    if other.0 > agent.1 {
        BodyClass::Front
    } else if other.1 < agent.0 {
        BodyClass::Rear
    } else {
        BodyClass::Side
    }
}

/// The `(low, high)` progress footprint of a body at a world position: its
/// projected arc length plus and minus its half-extent along the local
/// reference tangent.
fn progress_footprint(
    geometry: &CompiledReferencePath,
    body: &BodyShape,
    position: DVec2,
) -> (f64, f64) {
    let s = geometry.project(position).s();
    let along = half_extent_along(body, geometry.tangent_at(s));
    (s - along, s + along)
}

/// The minimum clearance fact of one candidate body over the horizon.
///
/// The minimum is the least of the exact clearances at every corridor sample
/// and the swept-interval minimum over every fine step, so a crossing between
/// two samples is read rather than missed.
fn body_fact(
    inputs: &ManeuverInputs<'_>,
    corridor: &[CorridorSample],
    candidate: &PredictedBody,
) -> Option<ClearanceFact> {
    let mut best: Option<ClearanceFact> = None;
    // Exact clearance at every sample, with the agent's real heading.
    for sample in corridor {
        let agent = pose_body(&inputs.body, sample.position, sample.heading_rad);
        let body = candidate
            .shape
            .translated(candidate.velocity_mps * sample.time_s);
        let clearance_m = body_clearance_m(&agent, &body);
        keep_least_strict(
            &mut best,
            ClearanceFact {
                clearance_m,
                object: LimitingObject::Agent(candidate.id),
                time_s: sample.time_s,
                closing_speed_mps: closing_speed(
                    &agent,
                    &body,
                    sample.heading_rad,
                    inputs.speed_mps,
                    candidate.velocity_mps,
                ),
            },
        );
    }
    // Swept minimum across every fine step, so a crossing between samples is
    // caught even at a single subdivision.
    for pair in corridor.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let agent_swept = SweptBody {
            shape: pose_body(&inputs.body, start.position, start.heading_rad),
            displacement_m: end.position - start.position,
        };
        let body_swept = SweptBody {
            shape: candidate
                .shape
                .translated(candidate.velocity_mps * start.time_s),
            displacement_m: candidate.velocity_mps * (end.time_s - start.time_s),
        };
        let clearance_m = tick_minimum_clearance_m(&agent_swept, &body_swept);
        keep_least_strict(
            &mut best,
            ClearanceFact {
                clearance_m,
                object: LimitingObject::Agent(candidate.id),
                time_s: start.time_s,
                closing_speed_mps: closing_speed(
                    &agent_swept.shape,
                    &body_swept.shape,
                    start.heading_rad,
                    inputs.speed_mps,
                    candidate.velocity_mps,
                ),
            },
        );
    }
    best
}

/// The band-edge clearance fact over the corridor: the least distance from the
/// agent's outer envelope to either side of the facility band.
fn band_edge_fact(inputs: &ManeuverInputs<'_>, corridor: &[CorridorSample]) -> ClearanceFact {
    let half_width = inputs.facility_width_m * 0.5;
    let mut best: Option<ClearanceFact> = None;
    for sample in corridor {
        let coordinate = inputs.geometry.project(sample.position);
        let normal = inputs.geometry.normal_at(coordinate.s());
        let envelope = pose_body(&inputs.body, sample.position, sample.heading_rad);
        let clearance_m = half_width - coordinate.d().abs() - half_extent_along(&envelope, normal);
        let agent_velocity = DVec2::from_angle(sample.heading_rad) * inputs.speed_mps;
        let closing_speed_mps = if coordinate.d() < 0.0 {
            -agent_velocity.dot(normal)
        } else {
            agent_velocity.dot(normal)
        };
        keep_least_strict(
            &mut best,
            ClearanceFact {
                clearance_m,
                object: LimitingObject::BandEdge,
                time_s: sample.time_s,
                closing_speed_mps,
            },
        );
    }
    best.expect("the corridor always holds at least the current pose")
}

/// The closing speed of one body pair: the rate the signed clearance is
/// falling, positive when the gap is closing.
fn closing_speed(
    agent: &BodyShape,
    body: &BodyShape,
    heading_rad: f64,
    speed_mps: f64,
    body_velocity: DVec2,
) -> f64 {
    let normal = body_contact_normal(agent, body);
    let agent_velocity = DVec2::from_angle(heading_rad) * speed_mps;
    -(body_velocity - agent_velocity).dot(normal)
}

/// Keep the fact with the strictly least clearance, so an exact tie keeps the
/// earlier fact.
fn keep_least_strict(slot: &mut Option<ClearanceFact>, fact: ClearanceFact) {
    if slot.is_none_or(|current| fact.clearance_m < current.clearance_m) {
        *slot = Some(fact);
    }
}

/// Keep the least fact from an optional candidate in `slot`.
fn keep_least(slot: &mut Option<ClearanceFact>, fact: Option<ClearanceFact>) {
    if let Some(fact) = fact {
        keep_least_strict(slot, fact);
    }
}

/// The body shape at a world pose: the envelope re-centred and, for a box,
/// re-oriented.
fn pose_body(body: &BodyShape, position: DVec2, heading_rad: f64) -> BodyShape {
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

/// The half-extent of a body's envelope along a unit axis: the radius of a
/// circle, or the projection radius of a box onto the axis.
fn half_extent_along(body: &BodyShape, axis: DVec2) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steering::{LateralCorridor, SteeringLimits};

    fn straight() -> CompiledReferencePath {
        CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(400.0, 0.0)])
    }

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

    fn inputs<'a>(geometry: &'a CompiledReferencePath, target_offset_m: f64) -> ManeuverInputs<'a> {
        ManeuverInputs {
            geometry,
            facility_width_m: 7.0,
            direction: 1.0,
            body: BodyShape::Box {
                centre: DVec2::new(50.0, 0.0),
                heading_rad: 0.0,
                length_m: 4.0,
                width_m: 2.0,
            },
            speed_mps: 10.0,
            target_offset_m,
            steering: steering(2.0),
            target_clearance_m: 0.5,
            horizon_s: 2.0,
            cadence_s: 0.1,
            subdivisions: 4,
        }
    }

    #[test]
    fn an_empty_corridor_is_feasible_and_reports_the_band_edge() {
        let geometry = straight();
        let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[]);
        assert!(prediction.is_feasible());
        assert_eq!(prediction.clears.swept.object, LimitingObject::BandEdge);
        // Band half width 3.5, envelope half extent 1.0, offset 0: clearance 2.5.
        assert!((prediction.clears.swept.clearance_m - 2.5).abs() < 1e-9);
        assert!(prediction.clears.front.is_none());
        assert!(prediction.clears.rear.is_none());
        assert!(prediction.clears.side.is_none());
        assert!(prediction.corridor.len() > 1);
    }

    #[test]
    fn a_target_outside_the_usable_corridor_is_infeasible_on_the_band_edge() {
        let geometry = straight();
        let prediction = predict_maneuver_corridor(inputs(&geometry, 5.0), &[]);
        assert_eq!(
            prediction.verdict,
            PredictionVerdict::Infeasible {
                limiting: LimitingObject::BandEdge
            }
        );
    }

    #[test]
    fn a_slower_front_body_reports_a_front_fact() {
        let geometry = straight();
        // A box 14 m ahead, slower, so the agent closes on it.
        let lead = PredictedBody {
            id: AgentId::from_index(0),
            shape: BodyShape::Box {
                centre: DVec2::new(64.0, 0.0),
                heading_rad: 0.0,
                length_m: 4.0,
                width_m: 2.0,
            },
            velocity_mps: DVec2::new(6.0, 0.0),
        };
        let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[lead]);
        let front = prediction.clears.front.expect("a body is ahead");
        assert_eq!(front.object, LimitingObject::Agent(AgentId::from_index(0)));
        // Start gap: 64 - 2 - 50 - 2 = 10 m; the agent closes 4 m/s for 2 s, so
        // the least front clearance is 10 - 8 = 2 m.
        assert!(
            (front.clearance_m - 2.0).abs() < 1e-9,
            "{}",
            front.clearance_m
        );
        assert!(prediction.clears.rear.is_none());
        assert!(prediction.clears.side.is_none());
        assert!(prediction.is_feasible());
    }

    #[test]
    fn an_approaching_rear_body_reports_a_rear_fact() {
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
        assert!(
            rear.closing_speed_mps > 0.0,
            "a faster follower closes: {}",
            rear.closing_speed_mps
        );
        assert!(prediction.clears.front.is_none());
    }

    #[test]
    fn a_side_by_side_body_reports_a_side_fact() {
        let geometry = straight();
        let alongside = PredictedBody {
            id: AgentId::from_index(0),
            shape: BodyShape::Box {
                centre: DVec2::new(50.0, 2.0),
                heading_rad: 0.0,
                length_m: 4.0,
                width_m: 2.0,
            },
            velocity_mps: DVec2::new(10.0, 0.0),
        };
        let prediction = predict_maneuver_corridor(inputs(&geometry, 0.0), &[alongside]);
        let side = prediction.clears.side.expect("a body is alongside");
        assert_eq!(side.object, LimitingObject::Agent(AgentId::from_index(0)));
        // Centres 2 m apart, half widths 1 m each: zero surface clearance.
        assert!(side.clearance_m.abs() < 1e-9, "{}", side.clearance_m);
        assert!(prediction.clears.front.is_none());
        assert!(prediction.clears.rear.is_none());
    }

    #[test]
    fn a_crossing_between_samples_is_caught_by_the_swept_minimum() {
        let geometry = straight();
        // The agent holds station; a body crosses its lane through the middle
        // of the first fine step and is clear at both endpoints.
        let crossing = PredictedBody {
            id: AgentId::from_index(0),
            shape: BodyShape::Circle {
                centre: DVec2::new(50.0, 5.0),
                radius_m: 0.5,
            },
            velocity_mps: DVec2::new(0.0, -10.0),
        };
        let hold = ManeuverInputs {
            speed_mps: 0.0,
            cadence_s: 1.0,
            subdivisions: 1,
            ..inputs(&geometry, 0.0)
        };
        let prediction = predict_maneuver_corridor(hold, &[crossing]);
        // Endpoint clearances are positive, but the swept read sees the pass.
        let start = body_clearance_m(
            &pose_body(&hold.body, DVec2::new(50.0, 0.0), 0.0),
            &crossing.shape,
        );
        let end = body_clearance_m(
            &pose_body(&hold.body, DVec2::new(50.0, 0.0), 0.0),
            &crossing.shape.translated(DVec2::new(0.0, -20.0)),
        );
        assert!(start > 0.0 && end > 0.0, "endpoints are clear");
        assert!(
            prediction.clears.swept.clearance_m < 0.0,
            "the swept minimum caught the crossing: {}",
            prediction.clears.swept.clearance_m
        );
        assert!(!prediction.is_feasible());
        assert_eq!(
            prediction.clears.swept.object,
            LimitingObject::Agent(AgentId::from_index(0))
        );
    }

    #[test]
    fn a_curved_boundary_reports_the_route_relative_band_edge() {
        // A gentle arc of radius 1000 m: the reference curves away, and the
        // band edge is the compiled constant-width boundary in route space.
        let geometry = CompiledReferencePath::arc(
            DVec2::new(0.0, 1000.0),
            1000.0,
            -0.5 * std::f64::consts::PI,
            0.05,
        );
        let body = BodyShape::Box {
            centre: geometry.point_at(0.0, 0.6),
            heading_rad: geometry.heading_at(0.0),
            length_m: 4.0,
            width_m: 2.0,
        };
        let curved = ManeuverInputs {
            facility_width_m: 3.5,
            body,
            target_offset_m: 0.6,
            target_clearance_m: 0.5,
            horizon_s: 0.1,
            ..inputs(&geometry, 0.0)
        };
        let prediction = predict_maneuver_corridor(curved, &[]);
        // Band half width 1.75, offset 0.6, envelope half extent 1.0: 0.15 m,
        // below the 0.5 m target, so the curved boundary binds. Holding the
        // offset keeps the envelope near-aligned; a small heading lag keeps the
        // reported minimum at or just below the analytic value.
        assert!(!prediction.is_feasible());
        assert_eq!(
            prediction.verdict,
            PredictionVerdict::Infeasible {
                limiting: LimitingObject::BandEdge
            }
        );
        assert!(
            prediction.clears.swept.clearance_m <= 0.15 + 1e-9
                && prediction.clears.swept.clearance_m > 0.10,
            "swept {} verdict {:?}",
            prediction.clears.swept.clearance_m,
            prediction.verdict
        );
    }

    #[test]
    fn the_result_is_independent_of_body_insertion_order() {
        let geometry = straight();
        let ahead = PredictedBody {
            id: AgentId::from_index(0),
            shape: BodyShape::Box {
                centre: DVec2::new(60.0, 0.0),
                heading_rad: 0.0,
                length_m: 4.0,
                width_m: 2.0,
            },
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

    #[test]
    fn an_exact_tie_keeps_the_lower_agent_id() {
        let geometry = straight();
        // Two identical side-by-side bodies mirrored across the reference, at
        // the same clearance: the lower id is the limiting object. They arrive
        // in ascending order already reversed.
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

    #[test]
    fn a_marginal_gap_is_rejected_conservatively_at_every_subdivision() {
        let geometry = straight();
        // A side body half a metre away, and a target clearance just above it.
        let alongside = PredictedBody {
            id: AgentId::from_index(0),
            shape: BodyShape::Circle {
                centre: DVec2::new(50.0, 1.75),
                radius_m: 0.5,
            },
            velocity_mps: DVec2::new(10.0, 0.0),
        };
        let coarse = predict_maneuver_corridor(
            ManeuverInputs {
                target_clearance_m: 0.3,
                subdivisions: 1,
                ..inputs(&geometry, 0.0)
            },
            &[alongside],
        );
        let fine = predict_maneuver_corridor(
            ManeuverInputs {
                target_clearance_m: 0.3,
                subdivisions: 64,
                ..inputs(&geometry, 0.0)
            },
            &[alongside],
        );
        // Clearance is 1.75 - 1.0 - 0.5 = 0.25 m, below the 0.3 m target, so
        // neither subdivision may report feasible.
        assert!((coarse.clears.swept.clearance_m - 0.25).abs() < 1e-9);
        assert!(!coarse.is_feasible());
        assert!(!fine.is_feasible());
        assert_eq!(coarse.clears.swept.object, fine.clears.swept.object);
    }
}
