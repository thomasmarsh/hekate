//! Intentionally lossy observer view of the world.
//!
//! A [`Snapshot`] is what a viewer, trace writer, or test observes. It is not a
//! serialization of internal state and deliberately cannot be fed back into the
//! kernel.

use glam::DVec2;
use hekate_model::{
    BodyKind, CrossingId, FacilityId, MovementDirection, MovementId, PathId, PedestrianRouteId,
    PermissionEffect,
};

use crate::agent::{AgentId, AgentMode};
use crate::compliance::ComplianceDecision;
use crate::pedestrian_compliance::PedestrianComplianceDecision;
use crate::profile::{PedestrianProfile, VehicleProfile};
use crate::stage::ManeuverState;
use crate::time::SimTime;

/// How much per-agent detail a snapshot carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotDetail {
    /// Identifier, world position, and heading only; enough to render motion
    /// without carrying simulation internals.
    #[default]
    Position,
    /// Position plus mode, longitudinal motion, route, profile, and body
    /// dimensions.
    Full,
}

/// One body segment's world pose, ordered front to back within its body.
///
/// A Phase 1 body is a single envelope, so its segment list is empty; an
/// articulated chain later fills one pose per segment in chain order. The pose
/// is the segment's own centre and heading, which a sweep or a renderer reads
/// without re-deriving it from the body and the path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodySegmentSample {
    /// Segment centre in world metres.
    pub position: DVec2,
    /// Segment heading in world radians.
    pub heading_rad: f64,
}

/// Motion and route detail for one agent, present at [`SnapshotDetail::Full`].
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSample {
    /// Envelope kind of this body, so a consumer draws a box, a circle, or a
    /// segmented chain without re-deriving it from the mode.
    pub body_kind: BodyKind,
    /// Ordered body segments front to back, each with its own world pose. Empty
    /// for a Phase 1 single-envelope body.
    pub segments: Vec<BodySegmentSample>,
    /// Whether this body is a vehicle or a pedestrian. A pedestrian body is a
    /// circle inscribed in the reported body bounds.
    pub mode: AgentMode,
    /// Longitudinal speed in metres per second. IDM-controlled for demand
    /// vehicles; a pedestrian and the static walking-skeleton population hold
    /// a constant speed.
    pub speed_mps: f64,
    /// The guide path the agent follows.
    pub path: PathId,
    /// Arc-length position in metres.
    pub path_distance_m: f64,
    /// Body length in metres. A pedestrian body is a circle, so this is the
    /// diameter.
    pub body_length_m: f64,
    /// Body width in metres. A pedestrian body is a circle, so this is the
    /// diameter.
    pub body_width_m: f64,
    /// Assigned vehicle route, present for demand-generated vehicles.
    pub route: Option<MovementId>,
    /// Sampled vehicle profile, present for demand-generated vehicles.
    pub profile: Option<VehicleProfile>,
    /// Assigned pedestrian route, present for demand-generated pedestrians.
    pub pedestrian_route: Option<PedestrianRouteId>,
    /// Sampled pedestrian body and gait, present for demand-generated
    /// pedestrians.
    pub pedestrian_profile: Option<PedestrianProfile>,
    /// Most recent signal-compliance decision, present for signal-controlled
    /// vehicles. The snapshot deliberately carries only this small record, not
    /// the controller's internal state.
    pub decision: Option<ComplianceDecision>,
    /// Most recent pedestrian signal-compliance decision, present for a
    /// pedestrian on a route that reaches a signal-controlled crossing. As with
    /// `decision`, only the small record is carried, not internal state.
    pub pedestrian_decision: Option<PedestrianComplianceDecision>,
    /// Crossing a vehicle is currently yielding to, present for a vehicle
    /// stopped for an occupied crossing. `None` when it is not yielding.
    pub yield_crossing: Option<CrossingId>,
    /// Route-relative tactical state, present for a steering body whose route
    /// lies on a compiled facility. `None` for a pedestrian and a legacy
    /// version-1 path-following agent, which carry no route coordinates.
    pub route_state: Option<RouteStateSample>,
}

/// The optional route-relative tactical state of one agent.
///
/// The world pose stays collision and output truth; these coordinates are the
/// pose projected onto the compiled facility reference. A sample is present
/// only for an agent that carries route state, so a consumer distinguishes
/// "no route state" (`None`) from a coordinate of zero.
///
/// The wrong-way rule state ([`Self::perceived_rule`] and
/// [`Self::opposing_direction`]) is the sparse part of the same sample: it is
/// present only while the agent is actually on an opposing traversal, which is
/// the state a rule-state transition record (an `OpposingTraversal` boundary)
/// marks. Every other row carries it absent, and no row is added, removed, or
/// repeated for a transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteStateSample {
    /// Arc length along the compiled facility reference in metres.
    pub s_m: f64,
    /// Signed lateral offset in metres, positive to the left of the agent's own
    /// direction of travel.
    pub d_m: f64,
    /// The agent's active maneuver lifecycle state.
    pub maneuver_state: ManeuverState,
    /// The active maneuver's target signed offset, once one is fixed.
    pub target_offset_m: Option<f64>,
    /// The target facility of a cross-facility transition, once one is fixed.
    pub target_facility: Option<FacilityId>,
    /// Predicted minimum clearance over the maneuver horizon, once predicted.
    pub predicted_min_clearance_m: Option<f64>,
    /// The mode's resolved target clearance, when its policy declares one.
    pub target_clearance_m: Option<f64>,
    /// The mode's resolved feasible horizon in seconds, when its policy
    /// declares one.
    pub horizon_s: Option<f64>,
    /// The wrong-way rule state the agent perceived on its object, present
    /// exactly with [`Self::opposing_direction`]. It is the applicable
    /// `nominal_direction` permission statement, absent when none binds the
    /// pair, so a consumer reads why a body is on an opposing traversal
    /// without a second lookup.
    pub perceived_rule: Option<PermissionEffect>,
    /// The direction the agent travels on an opposing traversal of its object,
    /// the direction that opposes the object's rule direction. Present only
    /// while the agent's traversal is against that rule direction, so a
    /// nominal traversal, an `either` object with no rule direction, and an
    /// agent without route state all leave it absent rather than defaulted.
    pub opposing_direction: Option<MovementDirection>,
}

/// One agent as observed at a single instant.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentSample {
    /// Stable agent identifier.
    pub id: AgentId,
    /// World position in metres.
    pub position: DVec2,
    /// World heading in radians.
    pub heading_rad: f64,
    /// Motion detail, present when the snapshot requested it.
    pub motion: Option<MotionSample>,
}

/// An observer view of every live agent at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    scenario_id: String,
    time: SimTime,
    detail: SnapshotDetail,
    agents: Vec<AgentSample>,
}

impl Snapshot {
    pub(crate) fn new(
        scenario_id: String,
        time: SimTime,
        detail: SnapshotDetail,
        agents: Vec<AgentSample>,
    ) -> Self {
        Self {
            scenario_id,
            time,
            detail,
            agents,
        }
    }

    /// Authored scenario identifier, for provenance.
    pub fn scenario_id(&self) -> &str {
        &self.scenario_id
    }

    /// Simulation time this snapshot was taken at.
    pub fn time(&self) -> SimTime {
        self.time
    }

    /// The detail level this snapshot was built with.
    pub fn detail(&self) -> SnapshotDetail {
        self.detail
    }

    /// Live agents in stable spawn order.
    pub fn agents(&self) -> &[AgentSample] {
        &self.agents
    }
}
