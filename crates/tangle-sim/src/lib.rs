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
mod compliance;
mod config;
mod control;
mod demand;
mod event;
pub mod pedestrian;
mod pedestrian_compliance;
mod profile;
mod rng;
mod signal;
mod sim;
mod snapshot;
mod time;
mod units;

pub use agent::{AgentId, AgentMode};
pub use compliance::{ComplianceDecision, ComplianceReason, SignalAction};
pub use config::{DEFAULT_STEP, RunConfig};
pub use event::{DespawnReason, EVENT_VERSION, Event};
pub use pedestrian::{PedestrianWaypoint, PedestrianZone};
pub use pedestrian_compliance::{
    PedestrianComplianceDecision, PedestrianComplianceReason, PedestrianSignalAction,
};
pub use profile::{PedestrianProfile, VehicleProfile};
pub use signal::PedestrianSignalColor;
pub use sim::{InitError, RunSummary, Simulation, StepOutput};
pub use snapshot::{AgentSample, MotionSample, Snapshot, SnapshotDetail};
pub use time::SimTime;
pub use units::Seconds;

pub use tangle_model::MODEL_VERSION;
