//! Intentionally lossy observer view of the world.
//!
//! A [`Snapshot`] is what a viewer, trace writer, or test observes. It is not a
//! serialization of internal state and deliberately cannot be fed back into the
//! kernel.

use glam::DVec2;
use tangle_model::{MovementId, PathId};

use crate::agent::AgentId;
use crate::compliance::ComplianceDecision;
use crate::profile::VehicleProfile;
use crate::time::SimTime;

/// How much per-agent detail a snapshot carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotDetail {
    /// Identifier, world position, and heading only; enough to render motion
    /// without carrying simulation internals.
    #[default]
    Position,
    /// Position plus longitudinal motion, route, profile, and body dimensions.
    Full,
}

/// Motion and route detail for one agent, present at [`SnapshotDetail::Full`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSample {
    /// Longitudinal speed in metres per second. IDM-controlled for demand
    /// vehicles; the static walking-skeleton population holds its configured
    /// constant speed.
    pub speed_mps: f64,
    /// The guide path the agent follows.
    pub path: PathId,
    /// Arc-length position in metres.
    pub path_distance_m: f64,
    /// Body length in metres.
    pub body_length_m: f64,
    /// Body width in metres.
    pub body_width_m: f64,
    /// Assigned route, present for demand-generated vehicles.
    pub route: Option<MovementId>,
    /// Sampled physical and behavior profile, present for demand-generated
    /// vehicles.
    pub profile: Option<VehicleProfile>,
    /// Most recent signal-compliance decision, present for signal-controlled
    /// vehicles. The snapshot deliberately carries only this small record, not
    /// the controller's internal state.
    pub decision: Option<ComplianceDecision>,
}

/// One agent as observed at a single instant.
#[derive(Debug, Clone, Copy, PartialEq)]
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
