//! Intentionally lossy observer view of the world.
//!
//! A [`Snapshot`] is what a viewer, trace writer, or test observes. It is not a
//! serialization of internal state and deliberately cannot be fed back into the
//! kernel.

use glam::DVec2;
use tangle_model::{BodyKind, CrossingId, MovementId, PathId, PedestrianRouteId};

use crate::agent::{AgentId, AgentMode};
use crate::compliance::ComplianceDecision;
use crate::pedestrian_compliance::PedestrianComplianceDecision;
use crate::profile::{PedestrianProfile, VehicleProfile};
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
