//! Stable-identifier agent storage.
//!
//! Hot agent state lives in struct-of-arrays storage with contiguous,
//! stable-order vectors. Spawn order defines [`AgentId`]. A despawned agent
//! keeps its slot and is marked dead so identifiers and iteration order never
//! shift; state-affecting logic therefore never iterates a hash map.

use glam::DVec2;
use tangle_model::{
    BodyKind, CompiledReferencePath, CrossingId, FacilityId, MovementId, PathId, PedestrianRouteId,
};

use crate::compliance::ComplianceDecision;
use crate::narrow::NarrowProfile;
use crate::pedestrian_compliance::PedestrianComplianceDecision;
use crate::profile::{PedestrianProfile, VehicleProfile};
use crate::stage::{LateralManeuverRequest, ManeuverCorridor, ManeuverState};
use crate::steering::BoundedSteering;
use crate::time::SimTime;

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

impl AgentMode {
    /// Short stable label for traces, inspectors, and event records.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Vehicle => "vehicle",
            Self::Pedestrian => "pedestrian",
        }
    }

    /// Envelope kind of the body this mode carries: a vehicle is an oriented
    /// box, a pedestrian a circle.
    pub const fn body_kind(self) -> BodyKind {
        match self {
            Self::Vehicle => BodyKind::Box,
            Self::Pedestrian => BodyKind::Circle,
        }
    }
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

/// Route-relative tactical state of one steering agent on a compiled facility.
///
/// `s_m` and `d_m` are the agent's authoritative world pose projected onto the
/// facility's compiled reference. The world pose stays collision and output
/// truth; this state is projected back after integration and is never written
/// to the world directly. It is present only for an agent whose compiled mode
/// family steers (a wheeled box or capsule) and whose route lies on a compiled
/// facility, so a pedestrian and a legacy version-1 path-following agent carry
/// no route state and keep their current output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RouteState {
    /// The facility whose compiled reference the coordinates are measured on.
    pub(crate) facility: FacilityId,
    /// Arc length along the facility reference in metres.
    pub(crate) s_m: f64,
    /// Signed lateral offset in metres, positive to the left of the agent's own
    /// direction of travel.
    pub(crate) d_m: f64,
    /// The agent's active maneuver lifecycle state.
    pub(crate) maneuver: ManeuverState,
    /// The active maneuver's target signed offset, once one is fixed.
    pub(crate) target_offset_m: Option<f64>,
    /// The target facility of a cross-facility transition, once one is fixed.
    pub(crate) target_facility: Option<FacilityId>,
    /// Predicted minimum clearance over the maneuver horizon, once predicted.
    pub(crate) predicted_min_clearance_m: Option<f64>,
    /// The mode's resolved target clearance, when its compiled policy declares
    /// one; `None` for a mode with no free lateral motion.
    pub(crate) target_clearance_m: Option<f64>,
    /// The mode's resolved feasible horizon in seconds, when its compiled policy
    /// declares one; `None` for a mode with no free lateral motion.
    pub(crate) horizon_s: Option<f64>,
    /// The mode's compiled bounded-steering limits and usable corridor, present
    /// exactly when the agent can steer laterally. `None` for a mode with no
    /// free lateral motion, so no bounded steering request is produced and the
    /// longitudinal command path is unchanged.
    pub(crate) bounded_steering: Option<BoundedSteering>,
    /// The lateral intent a tactical leaf recorded for this agent, consumed by
    /// the kernel's attempt guard. `None` when no maneuver is requested.
    pub(crate) intent: Option<LateralManeuverRequest>,
    /// The candidate corridor the current maneuver claims, fixed when the
    /// maneuver was attempted and never revised.
    pub(crate) corridor: Option<ManeuverCorridor>,
    /// The passed obstacle of the current maneuver, fixed at the attempt.
    pub(crate) passed_body: Option<AgentId>,
    /// The signed offset the agent held when the current maneuver was
    /// attempted: the return target of `returning` and `aborted`, in the
    /// agent's own travel frame.
    pub(crate) pre_maneuver_offset_m: f64,
    /// Simulation time the current maneuver state was entered, or `None` while
    /// the agent is `following`. A claim is sought at the decision after the
    /// attempt, and the preparing hold timeout runs from here.
    pub(crate) state_since: Option<SimTime>,
    /// Whether the agent's committed maneuver must brake for a predicted
    /// clearance loss this step: decelerate within the profile's comfortable
    /// braking and never accelerate.
    pub(crate) braking: bool,
    /// Simulation time the committed maneuver began holding at or below its
    /// target clearance, or `None` while it is not holding.
    pub(crate) hold_since: Option<SimTime>,
    /// Simulation time a returning or aborted maneuver last sat within the
    /// settle tolerance of its return target, or `None` while it is away from
    /// it.
    pub(crate) settled_since: Option<SimTime>,
}

impl RouteState {
    /// Project a world pose onto a compiled facility reference.
    ///
    /// `direction` is the agent's longitudinal travel sign (`1.0` forward,
    /// `-1.0` reverse). The reference offsets are measured against the
    /// reference's own forward tangent, so the signed offset is mirrored into
    /// the agent's own travel frame: a body on the reference's left while
    /// travelling forward has a positive offset, and the same body travelling
    /// in reverse has the negated one.
    pub(crate) fn project(
        facility: FacilityId,
        geometry: &CompiledReferencePath,
        position: DVec2,
        direction: f64,
        target_clearance_m: Option<f64>,
        horizon_s: Option<f64>,
    ) -> Self {
        let coordinate = geometry.project(position);
        let d_m = coordinate.d() * direction;
        Self {
            facility,
            s_m: coordinate.s(),
            d_m,
            maneuver: ManeuverState::Following,
            target_offset_m: None,
            target_facility: None,
            predicted_min_clearance_m: None,
            target_clearance_m,
            horizon_s,
            bounded_steering: None,
            intent: None,
            corridor: None,
            passed_body: None,
            pre_maneuver_offset_m: d_m,
            state_since: None,
            braking: false,
            hold_since: None,
            settled_since: None,
        }
    }

    /// Attach the mode's compiled bounded-steering envelope.
    ///
    /// A projection with no envelope keeps the Increment 1 longitudinal-only
    /// behaviour, exactly as a projection with no lateral policy carries no
    /// target clearance.
    pub(crate) fn with_bounded_steering(mut self, bounded_steering: BoundedSteering) -> Self {
        self.bounded_steering = Some(bounded_steering);
        self
    }

    /// Reproject an integrated world pose back into the facility route frame.
    ///
    /// The world pose is the integrated truth, so this is the drift check that
    /// keeps the tactical coordinates consistent with it; it never moves the
    /// pose and never writes a target offset to `d`.
    pub(crate) fn reproject(
        &mut self,
        geometry: &CompiledReferencePath,
        position: DVec2,
        direction: f64,
    ) {
        let coordinate = geometry.project(position);
        self.s_m = coordinate.s();
        self.d_m = coordinate.d() * direction;
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
    /// vehicles. A narrow wheeled agent carries the shared longitudinal
    /// projection of its [`NarrowProfile`] here, so every shared stage reads one
    /// profile.
    pub profile: Option<VehicleProfile>,
    /// Sampled narrow wheeled parameter set, present only for a narrow mode
    /// (a capsule that steers); `None` for a passenger car and the scripted
    /// population. Carries the narrow-specific steering and lateral-clearance
    /// parameters the longitudinal model does not read.
    pub narrow_profile: Option<NarrowProfile>,
    /// Assigned pedestrian route, present for demand-generated pedestrians.
    pub pedestrian_route: Option<PedestrianRouteId>,
    /// Sampled pedestrian body and gait, present for demand-generated
    /// pedestrians.
    pub pedestrian_profile: Option<PedestrianProfile>,
    /// Route-relative tactical state, present for a steering agent whose route
    /// lies on a compiled facility and absent for a pedestrian and a legacy
    /// version-1 path-following agent.
    pub route_state: Option<RouteState>,
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
    pub(crate) narrow_profile: Vec<Option<NarrowProfile>>,
    /// Envelope kind of each body, derived once at spawn from the mode and the
    /// narrow profile: a vehicle is a box, a narrow mode a capsule, and a
    /// pedestrian a circle. Snapshot output reads it so a capsule body is
    /// reported without a mode branch.
    pub(crate) body_kind: Vec<BodyKind>,
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
    /// Crossing a vehicle is currently yielding to, present for a vehicle
    /// obliged by a `yield` rule and stopped for an occupied crossing. `None`
    /// for every other agent, and for a yielding vehicle once the crossing
    /// clears.
    pub(crate) yield_crossing: Vec<Option<CrossingId>>,
    /// Route-relative tactical state of each steering agent on a compiled
    /// facility, in slot order. `None` for a pedestrian and for a legacy
    /// version-1 path-following agent, so absent state stays absent.
    pub(crate) route_state: Vec<Option<RouteState>>,
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
        self.body_kind.push(match (init.mode, init.narrow_profile) {
            (AgentMode::Pedestrian, _) => BodyKind::Circle,
            (AgentMode::Vehicle, Some(_)) => BodyKind::Capsule,
            (AgentMode::Vehicle, None) => BodyKind::Box,
        });
        self.narrow_profile.push(init.narrow_profile);
        self.pedestrian_route.push(init.pedestrian_route);
        self.pedestrian_profile.push(init.pedestrian_profile);
        self.pedestrian_waypoint_index.push(0);
        self.decision.push(None);
        self.pedestrian_decision.push(None);
        self.yield_crossing.push(None);
        self.route_state.push(init.route_state);
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
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
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

    /// A straight reference along the world x axis, so the left normal is
    /// `+y` and an offset's sign is unambiguous.
    fn straight_reference() -> CompiledReferencePath {
        CompiledReferencePath::from_polyline(&[DVec2::new(0.0, 0.0), DVec2::new(100.0, 0.0)])
    }

    /// Projection signs are measured in the agent's own travel frame: a point
    /// left of the reference while travelling forward is positively offset, and
    /// the same point travelling in reverse is negatively offset.
    #[test]
    fn projection_mirrors_the_signed_offset_in_the_travel_frame() {
        let reference = straight_reference();
        let left_of_forward = DVec2::new(10.0, 1.5);
        let forward = RouteState::project(
            FacilityId::from_index(0),
            &reference,
            left_of_forward,
            1.0,
            None,
            None,
        );
        assert!((forward.s_m - 10.0).abs() < 1e-12);
        assert!((forward.d_m - 1.5).abs() < 1e-12);

        let reverse = RouteState::project(
            FacilityId::from_index(0),
            &reference,
            left_of_forward,
            -1.0,
            None,
            None,
        );
        assert!((reverse.s_m - 10.0).abs() < 1e-12);
        assert!((reverse.d_m + 1.5).abs() < 1e-12);
        assert_eq!(reverse.d_m, -forward.d_m);
    }

    /// A projection also carries the mode's compiled maneuver policy and starts
    /// in the `following` state, and reprojection keeps only the geometry.
    #[test]
    fn projection_carries_the_maneuver_policy_and_follows() {
        let reference = straight_reference();
        let mut state = RouteState::project(
            FacilityId::from_index(3),
            &reference,
            DVec2::new(4.0, 0.25),
            1.0,
            Some(0.75),
            Some(4.0),
        );
        assert_eq!(state.maneuver, ManeuverState::Following);
        assert_eq!(state.target_clearance_m, Some(0.75));
        assert_eq!(state.horizon_s, Some(4.0));
        assert_eq!(state.target_offset_m, None);
        assert_eq!(state.target_facility, None);
        assert_eq!(state.predicted_min_clearance_m, None);

        state.reproject(&reference, DVec2::new(20.0, -0.5), 1.0);
        assert!((state.s_m - 20.0).abs() < 1e-12);
        assert!((state.d_m + 0.5).abs() < 1e-12);
        // Reprojection never clears or rewrites the maneuver policy.
        assert_eq!(state.target_clearance_m, Some(0.75));
    }

    /// Route state is stored in slot order and a dead slot keeps it, so the
    /// column never shifts under a despawn.
    #[test]
    fn route_state_is_stored_per_slot_and_survives_a_despawn() {
        let reference = straight_reference();
        let mut store = AgentStore::default();
        store.push(init(0.0));
        store.push(AgentInit {
            route_state: Some(RouteState::project(
                FacilityId::from_index(1),
                &reference,
                DVec2::new(5.0, 0.0),
                1.0,
                None,
                None,
            )),
            ..init(5.0)
        });
        assert_eq!(store.route_state[0], None);
        assert_eq!(
            store.route_state[1].map(|state| state.facility),
            Some(FacilityId::from_index(1))
        );

        store.alive[1] = false;
        assert_eq!(
            store.route_state[1].map(|state| state.s_m),
            Some(5.0),
            "a dead slot keeps its route state, so no column shifts"
        );
    }

    /// Every maneuver lifecycle state the contract fixes is representable and
    /// storable, and its stable label round-trips for snapshot and trajectory
    /// serialization.
    #[test]
    fn every_maneuver_state_is_recorded_and_labeled() {
        let states = [
            ManeuverState::Following,
            ManeuverState::Preparing,
            ManeuverState::Committed,
            ManeuverState::Returning,
            ManeuverState::Aborted,
        ];
        let reference = straight_reference();
        let mut store = AgentStore::default();
        for (offset, state) in states.into_iter().enumerate() {
            assert_eq!(ManeuverState::from_label(state.label()), Some(state));
            let mut route = RouteState::project(
                FacilityId::from_index(0),
                &reference,
                DVec2::new(offset as f64, 0.0),
                1.0,
                None,
                None,
            );
            route.maneuver = state;
            store.push(AgentInit {
                route_state: Some(route),
                ..init(offset as f64)
            });
        }
        let stored: Vec<ManeuverState> = store
            .route_state
            .iter()
            .map(|state| state.expect("every slot carries route state").maneuver)
            .collect();
        assert_eq!(stored, states);
        assert_eq!(ManeuverState::from_label("not_a_state"), None);
    }
}
