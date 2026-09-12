//! Stable-identifier agent storage.
//!
//! Hot agent state lives in struct-of-arrays storage with contiguous,
//! stable-order vectors. Spawn order defines [`AgentId`]. A despawned agent
//! keeps its slot and is marked dead so identifiers and iteration order never
//! shift; state-affecting logic therefore never iterates a hash map.

use glam::DVec2;
use tangle_model::{MovementId, PathId, PedestrianRouteId};

use crate::compliance::ComplianceDecision;
use crate::pedestrian_compliance::PedestrianComplianceDecision;
use crate::profile::{PedestrianProfile, VehicleProfile};

/// Which mode of agent a slot holds.
///
/// Both modes share one contiguous agent store, so they occupy the same world,
/// the same stable identifier space, and the same event stream. The mode only
/// selects how the body is sized and, later, how it is controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    /// A motor vehicle, an oriented box with a longitudinal controller.
    Vehicle,
    /// A pedestrian, a circle that follows a pedestrian route.
    Pedestrian,
}

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
    /// Longitudinal speed in metres per second. IDM-controlled for
    /// demand-generated vehicles, constant for the static walking population.
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
    /// Which mode this agent belongs to.
    pub mode: AgentMode,
    /// Assigned vehicle route, present for demand-generated vehicles.
    pub movement: Option<MovementId>,
    /// Sampled physical and behavior profile, present for demand-generated
    /// vehicles.
    pub profile: Option<VehicleProfile>,
    /// Assigned pedestrian route, present for demand-generated pedestrians.
    pub pedestrian_route: Option<PedestrianRouteId>,
    /// Sampled pedestrian body and gait, present for demand-generated
    /// pedestrians.
    pub pedestrian_profile: Option<PedestrianProfile>,
}

/// Contiguous, stable-order agent state.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AgentStore {
    pub(crate) alive: Vec<bool>,
    pub(crate) mode: Vec<AgentMode>,
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
    pub(crate) pedestrian_route: Vec<Option<PedestrianRouteId>>,
    pub(crate) pedestrian_profile: Vec<Option<PedestrianProfile>>,
    /// Index of the pedestrian's next waypoint target along its route. Always
    /// `0` for a vehicle, which has no waypoint plan.
    pub(crate) pedestrian_waypoint_index: Vec<usize>,
    /// Most recent signal-compliance decision, present for signal-controlled
    /// vehicles. `None` for vehicles with no signal rule.
    pub(crate) decision: Vec<Option<ComplianceDecision>>,
    /// Most recent pedestrian signal-compliance decision, present for a
    /// pedestrian approaching a signal-controlled crossing.
    pub(crate) pedestrian_decision: Vec<Option<PedestrianComplianceDecision>>,
}

impl AgentStore {
    /// Append an agent, returning its stable identifier.
    pub(crate) fn push(&mut self, init: AgentInit) -> AgentId {
        let id = AgentId::from_index(self.alive.len());
        self.alive.push(true);
        self.mode.push(init.mode);
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
        self.pedestrian_route.push(init.pedestrian_route);
        self.pedestrian_profile.push(init.pedestrian_profile);
        self.pedestrian_waypoint_index.push(0);
        self.decision.push(None);
        self.pedestrian_decision.push(None);
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
            mode: AgentMode::Vehicle,
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
            pedestrian_route: None,
            pedestrian_profile: None,
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
    fn stores_mode_and_pedestrian_columns_in_slot_order() {
        let mut store = AgentStore::default();
        store.push(init(0.0));
        store.push(AgentInit {
            mode: AgentMode::Pedestrian,
            body_length_m: 0.5,
            body_width_m: 0.5,
            pedestrian_route: Some(PedestrianRouteId::from_index(2)),
            pedestrian_profile: Some(PedestrianProfile {
                radius_m: 0.25,
                desired_speed_mps: 1.2,
                compliance: 1.0,
            }),
            ..init(5.0)
        });
        assert_eq!(store.mode, [AgentMode::Vehicle, AgentMode::Pedestrian]);
        assert_eq!(store.pedestrian_route[0], None);
        assert_eq!(
            store.pedestrian_route[1],
            Some(PedestrianRouteId::from_index(2))
        );
        assert_eq!(store.pedestrian_profile[0], None);
        assert_eq!(
            store.pedestrian_profile[1].map(|profile| profile.radius_m),
            Some(0.25)
        );
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
