//! Deterministic, fixed-step simulation kernel.
//!
//! The kernel owns the authoritative clock. It must never read wall-clock time,
//! and it must not depend on Bevy, a window, or the filesystem. The one-way
//! dependency direction is enforced in CI by
//! `scripts/check-dependency-direction.sh`.
//!
//! The public surface mirrors the kernel API planned in `PHASE_1_PLAN.md`:
//!
//! ```
//! use tangle_sim::{RunConfig, Simulation, SnapshotDetail};
//! # let scenario = {
//! #     let source = tangle_model::parse_scenario_source(
//! #         "{ schema_version: 1, id: 'demo', coordinate_system: { x: 'east_m', y: 'north_m' }, \
//! #          paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
//! #          portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, \
//! #          { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ], \
//! #          population: { vehicle_count: 1, vehicle_speed_mps: 5.0, vehicle_spacing_m: 5.0, \
//! #          vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
//! #     )
//! #     .expect("parses");
//! #     tangle_model::CompiledScenario::compile(source).expect("compiles")
//! # };
//! let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("builds");
//! {
//!     let output = sim.step();
//!     assert_eq!(output.events().len(), 1);
//! }
//! assert_eq!(sim.time().tick(), 1);
//! let snapshot = sim.snapshot(SnapshotDetail::Full);
//! assert_eq!(snapshot.agents().len(), 1);
//! let summary = sim.finish();
//! assert_eq!(summary.ticks(), 1);
//! ```

mod agent;
mod close_pass;
mod compliance;
mod config;
mod control;
mod controller;
mod demand;
mod event;
mod index;
mod metrics;
mod narrow;
pub mod pedestrian;
mod pedestrian_compliance;
pub mod prediction;
mod profile;
mod query;
mod rng;
mod safety;
mod signal;
mod sim;
mod snapshot;
mod stage;
pub mod steering;
mod swept;
mod time;
mod units;
pub mod wrong_way;

pub use agent::{AgentId, AgentMode};
pub use close_pass::{ClosePassBandDuration, ClosePassTracker, OvertakeObservation};
pub use compliance::{ComplianceDecision, ComplianceReason, SignalAction};
pub use config::{DEFAULT_STEP, RunConfig};
pub use controller::ControllerModelNames;
pub use event::{
    ControlTransitionKind, DespawnReason, EVENT_VERSION, Event, EventKind, ManeuverReasonCode,
    RegionKey, ViolationKind,
};
pub use index::{BroadPhase, SweptBroadPhase};
pub use metrics::{
    INTERACTION_RANGE_M, InteractionMetrics, MetricMinimum, ModePair, MovementKey,
    OperationMetrics, OperationValues, PostEncroachment, QueueDuration, QueueLength,
    RegionOccupancy, SEPARATION_RESOLUTION_M, TTC_HORIZON_S, TTC_TIME_TOLERANCE_S,
    tick_minimum_clearance_m, time_to_collision,
};
pub use narrow::NarrowProfile;
pub use pedestrian::{PedestrianWaypoint, PedestrianZone};
pub use pedestrian_compliance::{
    PedestrianComplianceDecision, PedestrianComplianceReason, PedestrianSignalAction,
};
pub use prediction::{
    ClearanceFact, CorridorSample, DEFAULT_SUBDIVISIONS, LimitingObject, MAX_PREDICTION_STEPS,
    ManeuverInputs, ManeuverPrediction, PredictedBody, PredictedClearances, PredictionVerdict,
    predict_crossing_corridor, predict_maneuver_corridor,
};
pub use profile::{PedestrianProfile, VehicleProfile};
pub use query::{
    Aabb, BodyShape, CONTACT_EPSILON_M, bodies_intersect, body_clearance_m, body_contact_normal,
};
pub use safety::{NEAR_MISS_THRESHOLD_M, QUEUE_STOP_SPEED_MPS};
pub use signal::PedestrianSignalColor;
pub use sim::{CONNECTOR_CONTINUITY_TOLERANCE_M, InitError, RunSummary, Simulation, StepOutput};
pub use snapshot::{
    AgentSample, BodySegmentSample, MotionSample, RouteStateSample, Snapshot, SnapshotDetail,
};
pub use stage::{
    FacilityTransitionRecord, LateralManeuverRequest, ManeuverAbortReason, ManeuverEdge,
    ManeuverReason, ManeuverState, ManeuverTransition, PassSide, SETTLE_TOLERANCE_M,
    TransitionKind,
};
pub use steering::{
    BoundedSteering, LATERAL_APPROACH_S, LateralCorridor, MIN_SPEED_FOR_LATERAL_BOUND_MPS,
    SteeringLimits, SteeringRequest, SteeringStep, bounded_steering_step,
};
pub use swept::{SweptBody, TOI_TIME_TOLERANCE, TimeOfImpact, band_entry, time_of_impact};
pub use time::SimTime;
pub use units::Seconds;
pub use wrong_way::{
    WrongWayDecision, WrongWayInputs, WrongWayOption, WrongWayReason, decide, maneuver_draw,
};

pub use tangle_model::MODEL_VERSION;
