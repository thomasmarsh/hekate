//! Documented pedestrian waypoint controller with local collision avoidance.
//!
//! # Model card
//!
//! This is the Phase 1 pedestrian model: a documented, replaceable microscopic
//! waypoint controller, not a calibrated scientific claim. Its two ingredients
//! are pure-pursuit-style waypoint seeking (R. Craig Coulter, "Implementation
//! of the Pure Pursuit Path Tracking Algorithm", 1992) and a bounded repulsive
//! interaction in the spirit of the social-force model (Dirk Helbing and Péter
//! Molnár, "Social Force Model for Pedestrian Dynamics", 1995). It is a
//! bounded-steering approximation of that interaction, not a reimplementation
//! of the model's exponential force law, and no parameter here is calibrated
//! against empirical trajectories. The vehicle model card lives with the IDM
//! controller in [`crate::control`]; Increment 3's explicit, replaceable
//! controller interfaces and the vehicle/pedestrian model-card pair are
//! delivered by the increment's later slice.
//!
//! ## State
//!
//! A pedestrian is a circle of radius `radius_m` whose state is a world
//! position in metres, a world heading in radians in `(-pi, pi]` (the direction
//! of travel), and a speed in metres per second. Its route progress is the arc
//! length of the projection of its position onto the compiled route path,
//! measured from the route entry along the direction of travel: `arc` for a
//! route entering at the path start, `length - arc` for a route entering at the
//! path end. The projection is clamped to the path extent, so progress lies in
//! `[0, length]`; a pedestrian that walks past the exit reports exactly
//! `length` and despawns with [`crate::DespawnReason::ExitedPath`].
//!
//! ## Waypoints
//!
//! The waypoints of a route are derived once per run from the compiled route,
//! in travel order:
//!
//! - one waypoint per named waiting area and crossing on the route, at the
//!   projection of that zone region's polygon centroid onto the route path;
//! - the route exit last, at the far end of the route path.
//!
//! Waypoints are ordered by route progress; equal progress is broken by kind
//! (waiting areas before crossings) and then by dense id, so the order is total
//! and deterministic. The route entry is the admission point, not a waypoint,
//! so the first target is the first zone ahead, or the exit for a route that
//! names no zone. A pedestrian's cursor advances past every waypoint whose
//! progress it has reached, so the cursor is monotone and the target is always
//! ahead of the pedestrian.
//!
//! ## Steering
//!
//! The desired direction is the unit vector to the current waypoint plus one
//! bounded lateral interaction term per nearby body ahead:
//!
//! ```text
//! d = unit(waypoint - p) + sum_i w_i * (g_r * lateral_away_i + g_l * right * h_i)
//! w_i = clamp((personal_space - clearance_i) / personal_space, 0, 1)
//! h_i = 1 when the nearest surface point of body i is strictly ahead, else 0
//! ```
//!
//! where `clearance_i` is the surface-to-surface clearance to body `i`,
//! `lateral_away_i` is the component of the unit vector away from body `i` that
//! is perpendicular to the current heading, and `right` is the unit vector to
//! the pedestrian's right. The interaction never fights the waypoint seek
//! head-on, because only the perpendicular component of "away" is used; it is
//! therefore a *deflection* toward the free side, with the fixed right-hand
//! term resolving the exactly head-on case. That right-hand term is the `h_i`
//! factor: it applies only to a body strictly ahead, so a pedestrian is never
//! steered into a body beside it, and a body behind the current heading is
//! ignored, so a queue cannot push itself forward.
//!
//! ## Bounds
//!
//! Heading, speed, and speed change are bounded every step:
//!
//! - `|Δheading| <= min(max_turn_rate, max_lateral_accel / max(v, v_floor)) * dt`,
//!   so the implied lateral acceleration `v * |Δheading| / dt` never exceeds
//!   [`MAX_LATERAL_ACCEL_MPS2`];
//! - speed stays in `[0, v0]` with `v0` the sampled desired walking speed;
//! - `Δspeed <= MAX_ACCEL_MPS2 * dt` and `Δspeed >= -MAX_DECEL_MPS2 * dt`,
//!   except for the documented emergency backstop below;
//! - the speed target is reduced while turning hard
//!   (`turn_slowdown_gain`), down to `min_turn_speed_fraction * v0`, so a
//!   pedestrian corners slower than it walks straight but never stalls.
//!
//! ## Emergency backstop
//!
//! The bounds above describe the steering model. On top of them the kernel
//! applies one hard spacing cap: for every nearby body whose nearest surface
//! point lies along the commanded heading, the step may not close more than the
//! available surface clearance, using the body's full speed as the worst-case
//! closing contribution and a small contact margin:
//!
//! ```text
//! step * closing_direction <= clearance_i - v_i * dt - contact_margin
//! ```
//!
//! where `closing_direction` is the component of the commanded heading toward
//! body `i`'s nearest surface point, so the bound applies to the closing part of
//! the step and a step that grazes past a body is not needlessly stopped. The
//! cap is evaluated along the *commanded* heading, so it holds jointly with the
//! turn: a step never both turns and closes on a body. Surface distance to a
//! convex body decreases by at most that closing component, so on top of the
//! body's own worst-case translation this bounds the closure per step: a
//! pedestrian never tunnels through a body, never teleports, and never makes the
//! first contact with a body ahead — a pedestrian steps into another body only
//! when that body is itself already moving onto the pedestrian, which a vehicle
//! can still do when its movement carries no `yield` rule or it is already
//! committed to the crossing (see [`crate::Simulation`]).
//! The cap only ever *reduces* the step, and it never applies to motion away
//! from a body, so a squeezed pedestrian can always turn out and step clear, and
//! cannot deadlock on a body. When the cap demands a deceleration beyond
//! [`MAX_DECEL_MPS2`] the step is counted in `Simulation::pedestrian_cap_steps`,
//! the assertion seam for that documented exception.
//!
//! The bound covers the neighbour's *translation*, using its full speed as the
//! worst case. It does not cover a body whose orientation changes within the
//! step, which can sweep a corner across a neighbouring circle without moving
//! its centre: in this kernel a vehicle's heading follows its path and can snap
//! at a polyline vertex. Exact swept queries that close that gap are the broad
//! phase and collision-query work of a later increment.
//!
//! ## Neighbour selection and tie-breaks
//!
//! The kernel scans every live agent in ascending [`AgentId`] order, so the
//! interaction terms are summed in a stable order and no hash map is iterated
//! for state-affecting logic. The spacing cap is the minimum over candidate
//! bodies, and an exact tie is won by the lowest agent id because the scan
//! replaces the incumbent only for a strictly smaller cap. A body whose nearest
//! surface point coincides exactly with the pedestrian (a perfect overlap, an
//! unreachable state) has no radial direction: the lower id is pushed left and
//! the higher id right, so the pair separates deterministically.
//!
//! ## Determinism
//!
//! The controller is a pure function of world state; it draws nothing from any
//! random stream, so adding it cannot reshuffle the named streams of any other
//! mode and the same seed reproduces every run exactly.
//!
//! ## Deferred
//!
//! This controller slows and steers around other bodies but encodes no signal
//! rule of its own. Whether a pedestrian waits for a forbidding crossing signal
//! is [`crate::PedestrianComplianceDecision`], which only reduces the commanded
//! speed and never bypasses this controller's bounds. Vehicle yielding to an
//! occupied crossing is Increment 3 slice D and lives with the vehicle rule in
//! [`crate::Simulation`]: a vehicle whose movement carries a `yield` rule
//! brakes for the crossing it crosses while a pedestrian occupies it, so a
//! pedestrian is overlapped only by a vehicle that is not obliged or is already
//! committed.

use glam::DVec2;
use tangle_model::{
    CompiledPath, CompiledPedestrianRoute, CompiledScenario, CrossingId, WaitingAreaId,
};

use crate::agent::AgentId;
use crate::profile::PedestrianProfile;

/// Radius in metres within which a pedestrian considers another body.
pub const SENSE_RADIUS_M: f64 = 4.0;

/// Comfort spacing in metres a pedestrian tries to keep beyond touching a body.
pub const PERSONAL_SPACE_M: f64 = 0.5;

/// Maximum heading change rate in radians per second.
pub const MAX_TURN_RATE_RAD_S: f64 = 2.0;

/// Maximum lateral acceleration in m/s² implied by a bounded turn.
///
/// The turn-rate bound is `min(MAX_TURN_RATE_RAD_S, MAX_LATERAL_ACCEL_MPS2 /
/// max(speed, MIN_SPEED_FOR_LATERAL_BOUND_MPS))`, so a fast pedestrian corners
/// within this lateral bound and a slow one within the turn-rate bound.
pub const MAX_LATERAL_ACCEL_MPS2: f64 = 2.0;

/// Speed floor in m/s used in the lateral-acceleration turn bound, so a
/// standstill does not make the bound unbounded.
pub const MIN_SPEED_FOR_LATERAL_BOUND_MPS: f64 = 0.5;

/// Maximum speed increase in m/s².
pub const MAX_ACCEL_MPS2: f64 = 1.5;

/// Maximum speed decrease in m/s² for the steering model. The spacing cap can
/// demand more; see the emergency backstop in the module model card.
pub const MAX_DECEL_MPS2: f64 = 2.0;

/// Weight of the perpendicular escape component of a nearby body's deflection.
pub const AVOID_RADIAL_GAIN: f64 = 1.0;

/// Weight of the fixed right-hand deflection that resolves a head-on encounter.
pub const AVOID_LATERAL_GAIN: f64 = 0.5;

/// Fraction of the desired speed given up at a 90° heading error or more.
pub const TURN_SLOWDOWN_GAIN: f64 = 0.75;

/// Lowest fraction of the desired speed a hard turn may command.
pub const MIN_TURN_SPEED_FRACTION: f64 = 0.25;

/// Surface clearance in metres the spacing cap leaves after a step.
pub const CONTACT_MARGIN_M: f64 = 0.05;

/// The zone a waypoint was derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedestrianZone {
    /// A waiting area the route stages at.
    WaitingArea(WaitingAreaId),
    /// A crossing the route traverses.
    Crossing(CrossingId),
}

impl PedestrianZone {
    /// Dense identifier of the zone, whichever kind it is.
    pub const fn id(self) -> u32 {
        match self {
            Self::WaitingArea(area) => area.get(),
            Self::Crossing(crossing) => crossing.get(),
        }
    }

    /// Ordering rank of the zone kind; waiting areas precede crossings when
    /// two zones project to the same route progress.
    const fn rank(self) -> u8 {
        match self {
            Self::WaitingArea(_) => 0,
            Self::Crossing(_) => 1,
        }
    }
}

/// One waypoint of a compiled pedestrian route, in travel order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PedestrianWaypoint {
    /// Arc-length position along the route path in metres.
    arc_m: f64,
    /// World position of the waypoint in metres.
    position: DVec2,
    /// Zone this waypoint was derived from; `None` for the route exit.
    zone: Option<PedestrianZone>,
}

impl PedestrianWaypoint {
    /// Arc-length position along the route path in metres.
    pub fn arc_m(self) -> f64 {
        self.arc_m
    }

    /// World position of the waypoint in metres.
    pub fn position(self) -> DVec2 {
        self.position
    }

    /// Zone this waypoint was derived from; `None` for the route exit.
    pub fn zone(self) -> Option<PedestrianZone> {
        self.zone
    }
}

/// One nearby body a pedestrian must avoid, as the kernel reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Conflict {
    /// Stable identifier of the nearby agent, for the documented tie-break.
    pub(crate) agent: AgentId,
    /// Vector from the pedestrian's centre to the nearest point on the
    /// neighbour's body surface.
    pub(crate) to_surface: DVec2,
    /// Signed surface-to-surface clearance in metres; negative means the
    /// bodies already overlap.
    pub(crate) clearance_m: f64,
    /// Neighbour speed in metres per second, the worst-case closing
    /// contribution to one step.
    pub(crate) speed_mps: f64,
}

/// The controller's view of the pedestrian it is steering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PedestrianState {
    /// Stable identifier of this pedestrian, for tie-breaks.
    pub(crate) agent: AgentId,
    /// Current world position in metres.
    pub(crate) position: DVec2,
    /// Current world heading in radians, the direction of travel.
    pub(crate) heading_rad: f64,
    /// Current speed in metres per second.
    pub(crate) speed_mps: f64,
    /// Current waypoint target in metres.
    pub(crate) target: DVec2,
}

/// One step of pedestrian steering: heading, speed target, and safety cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Steering {
    /// Commanded heading in radians after the turn-rate and lateral bounds.
    pub(crate) heading_rad: f64,
    /// Speed target in m/s before the per-step speed-change bound.
    pub(crate) speed_target_mps: f64,
    /// Hard spacing cap in m/s for this step, if a body ahead bounds it.
    pub(crate) spacing_cap_mps: Option<f64>,
}

/// Derive the travel-order waypoints of one compiled pedestrian route.
///
/// `direction` is the route's travel sense along its path: `1.0` when the route
/// enters at the path start, `-1.0` when it enters at the path end. The result
/// is empty only when the route's path is not compiled, which validation
/// rejects upstream.
pub(crate) fn plan_route(
    scenario: &CompiledScenario,
    route: &CompiledPedestrianRoute,
    direction: f64,
) -> Vec<PedestrianWaypoint> {
    let Some(path) = scenario.path(route.path()) else {
        return Vec::new();
    };
    let length = path.length();

    // (progress, kind rank, dense id, waypoint): the sort key order is the
    // documented tie-break, so the ordering is total and deterministic.
    let mut zones: Vec<(f64, u8, u32, PedestrianWaypoint)> = Vec::new();
    for area in route.waiting_areas() {
        let zone = PedestrianZone::WaitingArea(*area);
        if let Some(waypoint) = zone_waypoint(scenario, path, zone) {
            zones.push((
                waypoint_progress_m(waypoint, direction, length),
                zone.rank(),
                zone.id(),
                waypoint,
            ));
        }
    }
    for crossing in route.crossings() {
        let zone = PedestrianZone::Crossing(*crossing);
        if let Some(waypoint) = zone_waypoint(scenario, path, zone) {
            zones.push((
                waypoint_progress_m(waypoint, direction, length),
                zone.rank(),
                zone.id(),
                waypoint,
            ));
        }
    }
    zones.sort_by(|first, second| {
        first
            .0
            .total_cmp(&second.0)
            .then_with(|| (first.1, first.2).cmp(&(second.1, second.2)))
    });

    let mut waypoints: Vec<PedestrianWaypoint> = zones
        .into_iter()
        .map(|(_, _, _, waypoint)| waypoint)
        .collect();
    let exit_arc_m = if direction < 0.0 { 0.0 } else { length };
    waypoints.push(PedestrianWaypoint {
        arc_m: exit_arc_m,
        position: path.position_at(exit_arc_m),
        zone: None,
    });
    waypoints
}

/// One zone's waypoint: its region centroid projected onto the route path.
fn zone_waypoint(
    scenario: &CompiledScenario,
    path: &CompiledPath,
    zone: PedestrianZone,
) -> Option<PedestrianWaypoint> {
    let region = match zone {
        PedestrianZone::WaitingArea(area) => scenario.waiting_area(area)?.region(),
        PedestrianZone::Crossing(crossing) => scenario.crossing(crossing)?.region(),
    };
    let centroid = scenario.region(region)?.polygon().centroid();
    let arc_m = closest_arc(path, centroid);
    Some(PedestrianWaypoint {
        arc_m,
        position: path.position_at(arc_m),
        zone: Some(zone),
    })
}

/// Arc length of the point on `path` closest to `point`.
///
/// Ties are broken by the earliest segment because the scan replaces the
/// incumbent only for a strictly smaller distance.
pub(crate) fn closest_arc(path: &CompiledPath, point: DVec2) -> f64 {
    let mut best_arc_m = 0.0;
    let mut best_distance_m = f64::INFINITY;
    let mut segment_start_m = 0.0;
    for pair in path.points().windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let segment = end - start;
        let length = segment.length();
        let fraction = if length > 0.0 {
            ((point - start).dot(segment) / (length * length)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let distance_m = (point - (start + segment * fraction)).length();
        if distance_m < best_distance_m {
            best_distance_m = distance_m;
            best_arc_m = segment_start_m + length * fraction;
        }
        segment_start_m += length;
    }
    best_arc_m
}

/// Route progress of a waypoint in metres, measured from the route entry along
/// the direction of travel.
pub(crate) fn waypoint_progress_m(
    waypoint: PedestrianWaypoint,
    direction: f64,
    path_length_m: f64,
) -> f64 {
    route_progress_m(waypoint.arc_m(), direction, path_length_m)
}

/// Route progress in metres of a path arc length, measured from the route entry
/// along the direction of travel.
pub(crate) fn route_progress_m(arc_m: f64, direction: f64, path_length_m: f64) -> f64 {
    if direction < 0.0 {
        path_length_m - arc_m
    } else {
        arc_m
    }
}

/// Commanded heading and speed target for one step of pedestrian steering.
///
/// Pure in its inputs: the state, the sampled profile, the reported conflicts
/// in ascending agent-id order, and the step. See the module model card for the
/// equations, bounds, and tie-breaks.
pub(crate) fn steer(
    profile: &PedestrianProfile,
    state: &PedestrianState,
    conflicts: &[Conflict],
    dt: f64,
) -> Steering {
    let forward = DVec2::from_angle(state.heading_rad);
    let right = DVec2::new(forward.y, -forward.x);

    // Deflection pass: a bounded lateral interaction toward the free side.
    let mut desired = unit(state.target - state.position).unwrap_or(forward);
    for conflict in conflicts {
        let along = conflict.to_surface.dot(forward);
        // A body behind the current heading is the follower's concern, so a
        // pedestrian never reacts to it and a queue cannot push itself forward.
        if along < 0.0 || conflict.clearance_m >= PERSONAL_SPACE_M {
            continue;
        }
        let weight = ((PERSONAL_SPACE_M - conflict.clearance_m) / PERSONAL_SPACE_M).clamp(0.0, 1.0);
        let away = match unit(conflict.to_surface) {
            Some(toward) => -toward,
            // Degenerate perfect overlap: the ids break the tie, pushing the
            // lower id left and the higher id right.
            None if conflict.agent > state.agent => -right,
            None => right,
        };
        let lateral = away - forward * away.dot(forward);
        let mut deflection = lateral * AVOID_RADIAL_GAIN;
        // The fixed right-hand term resolves an exactly head-on encounter. It
        // never applies to a body abeam or behind, so a pedestrian is never
        // steered into a body beside it.
        if along > 0.0 {
            deflection += right * AVOID_LATERAL_GAIN;
        }
        desired += deflection * weight;
    }

    let desired_heading_rad = match unit(desired) {
        Some(direction) => direction.y.atan2(direction.x),
        None => state.heading_rad,
    };
    let error_rad = wrap_pi(desired_heading_rad - state.heading_rad);
    let turn_rate_rad_s = MAX_TURN_RATE_RAD_S
        .min(MAX_LATERAL_ACCEL_MPS2 / state.speed_mps.max(MIN_SPEED_FOR_LATERAL_BOUND_MPS));
    let limit_rad = turn_rate_rad_s * dt;
    let turn_fraction = (error_rad.abs() / std::f64::consts::FRAC_PI_2).min(1.0);
    let turn_scale = (1.0 - TURN_SLOWDOWN_GAIN * turn_fraction).clamp(MIN_TURN_SPEED_FRACTION, 1.0);
    let heading_rad = state.heading_rad + error_rad.clamp(-limit_rad, limit_rad);

    // Safety pass: the cap is evaluated along the commanded heading, the
    // direction the step will actually take, so a step can never close more
    // than the available clearance even when it is also a turn. The bound
    // applies to the closing *component* of the step, so a step that grazes
    // past a body is not needlessly stopped.
    let commanded = DVec2::from_angle(heading_rad);
    let mut spacing_cap_mps: Option<f64> = None;
    for conflict in conflicts {
        let Some(toward) = unit(conflict.to_surface) else {
            continue;
        };
        let closing = toward.dot(commanded);
        if closing <= 0.0 {
            continue;
        }
        let allowance_m = conflict.clearance_m - conflict.speed_mps * dt - CONTACT_MARGIN_M;
        let cap_mps = (allowance_m.max(0.0) / dt) / closing;
        if spacing_cap_mps.is_none_or(|incumbent| cap_mps < incumbent) {
            spacing_cap_mps = Some(cap_mps);
        }
    }

    Steering {
        // The heading is kept in `(-pi, pi]`, so it stays a canonical world
        // angle however long a run is; the turn itself is unaffected because
        // the steering error is wrapped before it is applied.
        heading_rad: wrap_pi(heading_rad),
        speed_target_mps: profile.desired_speed_mps * turn_scale,
        spacing_cap_mps,
    }
}

/// Advance a speed by at most one step of the documented acceleration bounds,
/// then apply the spacing cap.
///
/// Returns the new speed and whether the cap forced it below the bounded
/// deceleration, which is the documented emergency backstop.
pub(crate) fn advance_speed(
    speed_mps: f64,
    target_speed_mps: f64,
    spacing_cap_mps: Option<f64>,
    dt: f64,
) -> (f64, bool) {
    let change_mps =
        (target_speed_mps - speed_mps).clamp(-MAX_DECEL_MPS2 * dt, MAX_ACCEL_MPS2 * dt);
    let bounded_mps = (speed_mps + change_mps).max(0.0);
    let commanded_mps = spacing_cap_mps.map_or(bounded_mps, |cap_mps| bounded_mps.min(cap_mps));
    // Bounded braking alone would have left this speed, so anything below it
    // was forced by the spacing cap rather than by the steering model.
    let bounded_floor_mps = (speed_mps - MAX_DECEL_MPS2 * dt).max(0.0);
    let capped = commanded_mps < bounded_floor_mps - 1e-9;
    (commanded_mps.max(0.0), capped)
}

/// Nearest point on the surface of a circle body to `from`.
pub(crate) fn nearest_circle_point(centre: DVec2, radius_m: f64, from: DVec2) -> DVec2 {
    match unit(from - centre) {
        Some(direction) => centre + direction * radius_m,
        None => centre,
    }
}

/// Nearest point on the surface of an oriented box body to `from`.
///
/// The box is centred at `centre` with its length along `heading_rad`; the
/// point returned is on the box surface when `from` is outside it and the
/// projection of `from` when it is inside, which keeps the reported clearance
/// signed rather than clamped.
pub(crate) fn nearest_box_point(
    centre: DVec2,
    heading_rad: f64,
    length_m: f64,
    width_m: f64,
    from: DVec2,
) -> DVec2 {
    let (sin, cos) = heading_rad.sin_cos();
    let offset = from - centre;
    // Into the body frame: the box axis is the heading, so the rotation is the
    // transpose of the world rotation by `heading_rad`.
    let local = DVec2::new(
        offset.x * cos + offset.y * sin,
        -offset.x * sin + offset.y * cos,
    );
    let clamped = DVec2::new(
        local.x.clamp(-length_m * 0.5, length_m * 0.5),
        local.y.clamp(-width_m * 0.5, width_m * 0.5),
    );
    DVec2::new(
        clamped.x * cos - clamped.y * sin,
        clamped.x * sin + clamped.y * cos,
    ) + centre
}

/// Unit vector of `vector`, or `None` when it has no finite, positive length.
fn unit(vector: DVec2) -> Option<DVec2> {
    let length = vector.length();
    if length.is_finite() && length > 0.0 {
        Some(vector / length)
    } else {
        None
    }
}

/// Normalize an angle difference into `(-pi, pi]`.
fn wrap_pi(angle_rad: f64) -> f64 {
    let wrapped =
        (angle_rad + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if wrapped <= -std::f64::consts::PI {
        wrapped + std::f64::consts::TAU
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tangle_model::parse_scenario_source;

    const STEP_S: f64 = 0.05;

    fn profile() -> PedestrianProfile {
        PedestrianProfile {
            radius_m: 0.25,
            desired_speed_mps: 1.25,
            compliance: 1.0,
        }
    }

    fn state(heading_rad: f64, position: DVec2, speed_mps: f64, target: DVec2) -> PedestrianState {
        PedestrianState {
            agent: AgentId::from_index(0),
            position,
            heading_rad,
            speed_mps,
            target,
        }
    }

    fn conflict(agent: u32, to_surface: DVec2, clearance_m: f64, speed_mps: f64) -> Conflict {
        Conflict {
            agent: AgentId::from_index(agent as usize),
            to_surface,
            clearance_m,
            speed_mps,
        }
    }

    fn path(text: &str) -> CompiledPath {
        let source = parse_scenario_source(text).expect("scenario parses");
        let scenario = CompiledScenario::compile(source).expect("scenario compiles");
        scenario.paths()[0].clone()
    }

    #[test]
    fn closest_arc_projects_onto_the_nearest_segment() {
        let path = path(
            "{ schema_version: 1, id: 'bent', coordinate_system: { x: 'a', y: 'b' }, \
             paths: [ { id: 'p', points: [ { x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 10 } ] } ], \
             portals: [ { id: 'a', path: 'p', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'p', end: 'end', width_m: 3.0 } ] }",
        );
        // A point off the first segment projects onto it.
        assert!((closest_arc(&path, DVec2::new(4.0, 3.0)) - 4.0).abs() < 1e-12);
        // A point off the second segment projects onto it, past the corner.
        assert!((closest_arc(&path, DVec2::new(13.0, 14.0)) - 20.0).abs() < 1e-12);
        // A point beyond the far end clamps to the path length.
        assert!((closest_arc(&path, DVec2::new(30.0, 30.0)) - path.length()).abs() < 1e-12);
        // A point behind the start clamps to zero.
        assert!(closest_arc(&path, DVec2::new(-5.0, -5.0)).abs() < 1e-12);
    }

    #[test]
    fn free_flow_seeking_keeps_the_heading_and_the_desired_speed() {
        let steering = steer(
            &profile(),
            &state(
                std::f64::consts::FRAC_PI_2,
                DVec2::ZERO,
                1.25,
                DVec2::new(0.0, 10.0),
            ),
            &[],
            STEP_S,
        );
        assert!((steering.heading_rad - std::f64::consts::FRAC_PI_2).abs() < 1e-15);
        assert!((steering.speed_target_mps - 1.25).abs() < 1e-15);
        assert!(steering.spacing_cap_mps.is_none());
    }

    #[test]
    fn a_perpendicular_target_turns_within_the_documented_bounds() {
        let heading_north = state(0.0, DVec2::ZERO, 1.25, DVec2::new(0.0, 10.0));
        let steering = steer(&profile(), &heading_north, &[], STEP_S);
        let turned = steering.heading_rad - heading_north.heading_rad;
        // The turn-rate bound is the tighter of the two documented bounds.
        let bound = MAX_TURN_RATE_RAD_S.min(
            MAX_LATERAL_ACCEL_MPS2 / heading_north.speed_mps.max(MIN_SPEED_FOR_LATERAL_BOUND_MPS),
        );
        assert!((turned - bound * STEP_S).abs() < 1e-12, "turned {turned}");
        assert!(heading_north.speed_mps * turned / STEP_S <= MAX_LATERAL_ACCEL_MPS2 + 1e-9);
        // A 90° heading error gives up the full documented slowdown, and a
        // worse one never exceeds it.
        let expected = heading_north.speed_mps * (1.0 - TURN_SLOWDOWN_GAIN);
        assert!((steering.speed_target_mps - expected).abs() < 1e-12);
        let heading_west = state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0));
        let reversed = steer(&profile(), &heading_west, &[], STEP_S);
        assert!((reversed.heading_rad - heading_west.heading_rad).abs() < 1e-15);
        let heading_south = state(0.0, DVec2::ZERO, 1.25, DVec2::new(0.0, -10.0));
        let about_face = steer(&profile(), &heading_south, &[], STEP_S);
        assert!(
            (about_face.speed_target_mps - profile().desired_speed_mps * MIN_TURN_SPEED_FRACTION)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn a_body_dead_ahead_deflects_the_pedestrian_to_its_right() {
        let state = state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0));
        let conflict = conflict(1, DVec2::new(0.4, 0.0), 0.15, 1.25);
        let steering = steer(&profile(), &state, &[conflict], STEP_S);
        assert!(
            steering.heading_rad < state.heading_rad,
            "a right-hand deflection is a clockwise turn"
        );
        // Inside the personal space the body also caps the step: the allowance
        // is the clearance less the neighbour's own worst-case closing.
        // The cap bounds the closing component of the step, so the resulting
        // speed bound is the unprojected one divided by the step's closing
        // direction: slightly larger, never smaller.
        let cap = steering
            .spacing_cap_mps
            .expect("a body ahead bounds the step");
        let unprojected = (0.15 - 1.25 * STEP_S - CONTACT_MARGIN_M).max(0.0) / STEP_S;
        assert!(cap >= unprojected && cap <= 1.02 * unprojected, "cap {cap}");
    }

    #[test]
    fn a_body_behind_neither_deflects_nor_caps_the_pedestrian() {
        let state = state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0));
        let behind = conflict(1, DVec2::new(-0.2, 0.0), 0.0, 1.25);
        let steering = steer(&profile(), &state, &[behind], STEP_S);
        assert!((steering.heading_rad - state.heading_rad).abs() < 1e-15);
        assert!((steering.speed_target_mps - 1.25).abs() < 1e-15);
        assert!(steering.spacing_cap_mps.is_none());
    }

    #[test]
    fn a_body_beyond_the_personal_space_does_not_deflect() {
        let state = state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0));
        let far = conflict(1, DVec2::new(2.0, 0.0), PERSONAL_SPACE_M, 1.25);
        let steering = steer(&profile(), &state, &[far], STEP_S);
        assert!((steering.heading_rad - state.heading_rad).abs() < 1e-15);
        // With no deflection the heading is unchanged, so the cap is exact:
        // `clearance` less the neighbour's worst-case closing and the margin.
        let expected = (PERSONAL_SPACE_M - 1.25 * STEP_S - CONTACT_MARGIN_M) / STEP_S;
        assert!((steering.spacing_cap_mps.unwrap() - expected).abs() < 1e-12);
    }

    #[test]
    fn the_spacing_cap_is_the_minimum_over_every_candidate() {
        // The kernel reports conflicts in ascending agent-id order and keeps a
        // candidate only for a strictly smaller cap, so an exact tie keeps the
        // lowest id and the result is the minimum over the candidates.
        let state = state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0));
        let near = conflict(1, DVec2::new(0.2, 0.0), 0.1, 0.0);
        let far = conflict(2, DVec2::new(1.0, 0.0), 0.9, 0.0);
        let first = steer(&profile(), &state, &[near, far], STEP_S);
        let second = steer(&profile(), &state, &[far, near], STEP_S);
        let low = first.spacing_cap_mps.unwrap();
        assert!((low - second.spacing_cap_mps.unwrap()).abs() < 1e-12);
        // The nearest body decides: its bound is about 1 m/s while the farther
        // body's is tens of m/s.
        assert!((0.9..1.1).contains(&low), "cap {low}");
    }

    #[test]
    fn a_perfect_overlap_separates_by_agent_id() {
        let low = steer(
            &profile(),
            &state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0)),
            &[conflict(1, DVec2::ZERO, -0.5, 1.25)],
            STEP_S,
        );
        let high = steer(
            &profile(),
            &state(0.0, DVec2::ZERO, 1.25, DVec2::new(10.0, 0.0)),
            &[conflict(0, DVec2::ZERO, -0.5, 1.25)],
            STEP_S,
        );
        // The lower id (0) is pushed left (counter-clockwise) and the higher id
        // (1, the state's own agent in the second case) right.
        assert!(low.heading_rad > 0.0, "the lower id veers left");
        assert!(high.heading_rad < 0.0, "the higher id veers right");
        // A body exactly abeam is considered by the interaction but never caps
        // the step, so neither pedestrian can stall on it.
        assert!(low.spacing_cap_mps.is_none());
        assert!(high.spacing_cap_mps.is_none());
    }

    #[test]
    fn advance_speed_stays_within_the_bounds_unless_the_cap_binds() {
        // Speeding up and slowing down are each bounded by one step.
        let (faster, capped) = advance_speed(0.0, 1.25, None, STEP_S);
        assert!((faster - MAX_ACCEL_MPS2 * STEP_S).abs() < 1e-15);
        assert!(!capped);
        let (slower, capped) = advance_speed(1.25, 0.0, None, STEP_S);
        assert!((slower - (1.25 - MAX_DECEL_MPS2 * STEP_S)).abs() < 1e-15);
        assert!(!capped);
        // The spacing cap can demand more braking, and says so.
        let (capped_speed, capped) = advance_speed(1.25, 1.25, Some(0.0), STEP_S);
        assert_eq!(capped_speed, 0.0);
        assert!(capped);
    }

    #[test]
    fn nearest_surface_points_follow_the_body_shape() {
        // A circle's nearest point sits on its radius.
        let on_circle = nearest_circle_point(DVec2::new(3.0, 0.0), 0.5, DVec2::ZERO);
        assert!((on_circle - DVec2::new(2.5, 0.0)).length() < 1e-12);
        // A box's nearest point is clamped to its half extents in the body
        // frame, so a point off the side reports the side, not the corner.
        let on_box = nearest_box_point(DVec2::ZERO, 0.0, 4.0, 2.0, DVec2::new(10.0, 0.6));
        assert!((on_box - DVec2::new(2.0, 0.6)).length() < 1e-12);
        let past_the_nose = nearest_box_point(DVec2::ZERO, 0.0, 4.0, 2.0, DVec2::new(10.0, 0.0));
        assert!((past_the_nose - DVec2::new(2.0, 0.0)).length() < 1e-12);
        // A rotated box reports the same body in world terms.
        let rotated = nearest_box_point(
            DVec2::ZERO,
            std::f64::consts::FRAC_PI_2,
            4.0,
            2.0,
            DVec2::new(0.0, 10.0),
        );
        assert!((rotated - DVec2::new(0.0, 2.0)).length() < 1e-12);
    }

    #[test]
    fn the_commanded_heading_stays_normalized() {
        // A left turn from just below pi crosses the branch cut: the heading is
        // reported normalized and the turn is still the bounded one taken the
        // short way round, so a long run cannot grow the heading without bound.
        let start = std::f64::consts::PI - 0.05;
        let steering = steer(
            &profile(),
            &state(start, DVec2::ZERO, 1.25, DVec2::new(0.0, -10.0)),
            &[],
            STEP_S,
        );
        let heading = steering.heading_rad;
        assert!(heading > -std::f64::consts::PI && heading <= std::f64::consts::PI);
        let turned = wrap_pi(heading - start).abs();
        let bound = MAX_TURN_RATE_RAD_S
            .min(MAX_LATERAL_ACCEL_MPS2 / 1.25_f64.max(MIN_SPEED_FOR_LATERAL_BOUND_MPS))
            * STEP_S;
        assert!((turned - bound).abs() < 1e-12, "turned {turned}");
    }

    #[test]
    fn route_progress_measures_from_the_entry_along_travel() {
        assert!((route_progress_m(3.0, 1.0, 10.0) - 3.0).abs() < 1e-15);
        assert!((route_progress_m(3.0, -1.0, 10.0) - 7.0).abs() < 1e-15);
    }
}
