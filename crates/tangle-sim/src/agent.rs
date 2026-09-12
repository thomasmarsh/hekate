//! Stable-identifier agent storage.
//!
//! Hot agent state lives in struct-of-arrays storage with contiguous,
//! stable-order vectors. Spawn order defines [`AgentId`]. A despawned agent
//! keeps its slot and is marked dead so identifiers and iteration order never
//! shift; state-affecting logic therefore never iterates a hash map.

use glam::DVec2;
use tangle_model::{MovementId, PathId};

use crate::compliance::ComplianceDecision;
use crate::profile::VehicleProfile;

/// Stable identifier of one agent for the lifetime of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(u32);

impl AgentId {
    /// Construct an agent identifier from its spawn index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based spawn index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Initial state for one agent slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AgentInit {
    /// The guide path this agent follows.
    pub path: PathId,
    /// Current arc-length position along the path in metres.
    pub distance_m: f64,
    /// Constant longitudinal speed in metres per second.
    pub speed_mps: f64,
    /// World position in metres.
    pub position: DVec2,
    /// World heading in radians.
    pub heading_rad: f64,
    /// Body length in metres.
    pub body_length_m: f64,
    /// Body width in metres.
    pub body_width_m: f64,
    /// Longitudinal travel direction: `1.0` toward the path end, `-1.0`
    /// toward the path start.
    pub direction: f64,
    /// Assigned route, present for demand-generated vehicles.
    pub movement: Option<MovementId>,
    /// Sampled physical and behavior profile, present for demand-generated
    /// vehicles.
    pub profile: Option<VehicleProfile>,
}

/// Contiguous, stable-order agent state.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AgentStore {
    pub(crate) alive: Vec<bool>,
    pub(crate) path: Vec<PathId>,
    pub(crate) distance_m: Vec<f64>,
    pub(crate) speed_mps: Vec<f64>,
    pub(crate) position: Vec<DVec2>,
    pub(crate) heading_rad: Vec<f64>,
    pub(crate) body_length_m: Vec<f64>,
    pub(crate) body_width_m: Vec<f64>,
    pub(crate) direction: Vec<f64>,
    pub(crate) movement: Vec<Option<MovementId>>,
    pub(crate) profile: Vec<Option<VehicleProfile>>,
    /// Most recent signal-compliance decision, present for signal-controlled
    /// vehicles. `None` for vehicles with no signal rule.
    pub(crate) decision: Vec<Option<ComplianceDecision>>,
}

impl AgentStore {
    /// Append an agent, returning its stable identifier.
    pub(crate) fn push(&mut self, init: AgentInit) -> AgentId {
        let id = AgentId::from_index(self.alive.len());
        self.alive.push(true);
        self.path.push(init.path);
        self.distance_m.push(init.distance_m);
        self.speed_mps.push(init.speed_mps);
        self.position.push(init.position);
        self.heading_rad.push(init.heading_rad);
        self.body_length_m.push(init.body_length_m);
        self.body_width_m.push(init.body_width_m);
        self.direction.push(init.direction);
        self.movement.push(init.movement);
        self.profile.push(init.profile);
        self.decision.push(None);
        id
    }

    /// Number of slots, including dead ones.
    pub(crate) fn len(&self) -> usize {
        self.alive.len()
    }

    /// Number of slots still alive.
    pub(crate) fn alive_count(&self) -> usize {
        self.alive.iter().filter(|&&alive| alive).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(distance_m: f64) -> AgentInit {
        AgentInit {
            path: PathId::from_index(0),
            distance_m,
            speed_mps: 1.0,
            position: DVec2::ZERO,
            heading_rad: 0.0,
            body_length_m: 4.5,
            body_width_m: 1.8,
            direction: 1.0,
            movement: None,
            profile: None,
        }
    }

    #[test]
    fn assigns_stable_ids_in_spawn_order() {
        let mut store = AgentStore::default();
        assert_eq!(store.push(init(0.0)), AgentId::from_index(0));
        assert_eq!(store.push(init(10.0)), AgentId::from_index(1));
        assert_eq!(store.len(), 2);
        assert_eq!(store.alive_count(), 2);
        assert_eq!(store.distance_m[1], 10.0);
    }

    #[test]
    fn dead_slots_keep_their_identity() {
        let mut store = AgentStore::default();
        store.push(init(0.0));
        store.push(init(10.0));
        store.alive[0] = false;
        assert_eq!(store.len(), 2);
        assert_eq!(store.alive_count(), 1);
        assert_eq!(store.push(init(20.0)), AgentId::from_index(2));
    }
}
