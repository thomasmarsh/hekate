//! The four explicit controller stages of one agent update.
//!
//! `PHASE_2_PLAN.md` "Controller boundary" splits an agent update into four
//! stages, and this module names each one as an interface:
//!
//! 1. **Relevant-world query** ([`RelevantWorldQuery`]): collect nearby bodies,
//!    upcoming controls, route targets, and rules into an immutable
//!    [`Observation`].
//! 2. **Tactical choice** ([`TacticalChoice`]): select one maneuver as a
//!    [`Tactic`] recording a reason, target, commitment state, and abort
//!    condition.
//! 3. **Motion control** ([`MotionControl`]): convert the tactic into a bounded
//!    [`MotionCommand`], reaching the replaceable motion model through
//!    [`crate::controller`].
//! 4. **Physical advance** ([`PhysicalAdvance`]): integrate the pose the command
//!    implies and emit the step's diagnostics.
//!
//! Each interface consumes an immutable observation — the tactic from stage 2
//! on — and returns a command, so a stage can be replaced without one mode
//! mutating another agent directly. The kernel keeps every interaction
//! decision: constraint selection, leader and neighbour selection, stop-line
//! and crossing state, waypoint planning, conflict ordering, the safety
//! position caps, and the emergency counters all live in the kernel's stage
//! implementations, never in a model. A model only maps an observation and a
//! tactic to a command, exactly as [`crate::controller`] documents.
//!
//! The stages are crate-internal in Phase 1, like the model seam itself: the
//! kernel implements all four and drives them in order from
//! [`crate::Simulation`]. Phase 1 reselects the tactic every tick and commits
//! no maneuver, so a tactic is `Preparing` unless an active control holds it,
//! and its `started_at` is the simulation time it was selected at. The
//! commitment, abort, and abort conditions a future lateral maneuver needs are
//! the fields this record already carries.

use tangle_model::CrossingId;

use crate::agent::AgentId;
use crate::control::Constraint;
use crate::narrow::NarrowProfile;
use crate::pedestrian::{Conflict, PedestrianState, PedestrianWaypoint};
use crate::pedestrian_compliance::PedestrianComplianceDecision;
use crate::profile::{PedestrianProfile, VehicleProfile};
use crate::time::SimTime;

/// Why the tactical stage selected a maneuver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TacticReason {
    /// No active constraint: travel freely along the route.
    FreeFlow,
    /// Follow the nearest leader ahead on the same path.
    Follow,
    /// Hold at a required stop line.
    StopLine,
    /// Yield to an occupied crossing.
    YieldCrossing,
    /// Wait at a signal-controlled crossing.
    SignalWait,
    /// Steer toward the next route waypoint.
    SeekWaypoint,
}

/// What a tactic acts on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TacticTarget {
    /// No spatial target: the agent travels along its route.
    Route,
    /// The leader agent the tactic follows.
    Leader(AgentId),
    /// The required stop line the tactic holds at.
    StopLine,
    /// The crossing the tactic yields to.
    Crossing(CrossingId),
    /// The route waypoint the tactic steers toward.
    Waypoint(PedestrianWaypoint),
}

/// Where a tactic stands in the maneuver lifecycle state machine.
///
/// `PHASE_2_PLAN.md` names `following -> preparing -> committed -> returning`
/// with an `aborted` exit, and `docs/schema-v2-contract.md` *Maneuver lifecycle*
/// fixes the five states with these names. Increment 1 carries the first two as
/// its tactical record: a free or following tactic is reselected at the next
/// decision and is [`ManeuverState::Preparing`], while a tactic held by an
/// active control is [`ManeuverState::Committed`] for the step. The lateral
/// lifecycle — selecting a maneuver, committing a claim, returning, and aborting
/// — is Increment 2's state machine; this record is the state representation it
/// drives, and no state is spelled differently anywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManeuverState {
    /// No active lateral maneuver: the agent holds its offset and runs the
    /// longitudinal tactics.
    Following,
    /// A target and candidate corridor are fixed and a claim is sought.
    Preparing,
    /// The claim is granted and the agent displaces toward the target.
    Committed,
    /// The passed body is cleared and the agent returns to its own offset.
    Returning,
    /// The maneuver ended without reaching its target.
    Aborted,
}

impl ManeuverState {
    /// Stable lowercase label for snapshots, trajectories, and diagnostics.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Following => "following",
            Self::Preparing => "preparing",
            Self::Committed => "committed",
            Self::Returning => "returning",
            Self::Aborted => "aborted",
        }
    }

    /// Parse a label written by [`Self::label`], or `None` for an unknown label.
    pub fn from_label(label: &str) -> Option<Self> {
        [
            Self::Following,
            Self::Preparing,
            Self::Committed,
            Self::Returning,
            Self::Aborted,
        ]
        .into_iter()
        .find(|state| state.label() == label)
    }
}

/// The condition that ends a tactic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbortCondition {
    /// The tactic ends when its constraint clears.
    ConstraintClears,
    /// The tactic ends when the route waypoint is reached.
    WaypointReached,
    /// The tactic runs until the agent leaves the route.
    RouteComplete,
}

/// One tactic: the tactical stage's immutable command record.
///
/// The fields are the maneuver record `PHASE_2_PLAN.md` requires — reason,
/// target, commitment state, and abort condition — plus `started_at`, the
/// simulation time the tactic was selected at. Time comes from the kernel
/// clock, never from wall-clock time, so the record is deterministic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tactic {
    /// Why the maneuver was selected.
    pub(crate) reason: TacticReason,
    /// What the maneuver acts on.
    pub(crate) target: TacticTarget,
    /// Where the maneuver stands in its lifecycle.
    pub(crate) maneuver_state: ManeuverState,
    /// What ends the maneuver.
    pub(crate) abort: AbortCondition,
    /// Simulation time the maneuver was selected at.
    pub(crate) started_at: SimTime,
}

/// The immutable relevant world one path-following update consumes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VehicleObservation {
    /// Sampled behavior profile; `None` for the walking skeleton's scripted
    /// constant-speed population, which has no model to command.
    pub(crate) profile: Option<VehicleProfile>,
    /// The narrow wheeled parameter set when this path-following agent is a
    /// narrow mode (a capsule that steers); `None` for a passenger car and the
    /// scripted population. The kernel uses it to reach the narrow wheeled
    /// model; `profile` still carries the same agent's shared longitudinal
    /// parameters, so every shared constraint and cap reads one profile.
    pub(crate) narrow: Option<NarrowProfile>,
    /// Current speed in metres per second.
    pub(crate) speed_mps: f64,
    /// Nearest live leader ahead and the following constraint it imposes.
    pub(crate) leader: Option<(AgentId, Constraint)>,
    /// Required stop-line constraint, present while the recorded decision is
    /// to stop.
    pub(crate) stop_line: Option<Constraint>,
    /// Occupied crossing the vehicle yields to and the stop constraint at its
    /// entry.
    pub(crate) crossing_yield: Option<(CrossingId, Constraint)>,
}

/// The immutable relevant world one world-steering pedestrian update consumes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PedestrianObservation {
    /// Sampled body and gait profile.
    pub(crate) profile: PedestrianProfile,
    /// The controller's view of this pedestrian's own world state.
    pub(crate) state: PedestrianState,
    /// The waypoint the pedestrian steers toward, absent only when the route
    /// has no waypoint left at its cursor.
    pub(crate) target: Option<PedestrianWaypoint>,
    /// The upcoming crossing's signal-compliance decision, if the route reaches
    /// a signal-controlled crossing.
    pub(crate) decision: Option<PedestrianComplianceDecision>,
    /// Nearby bodies within the sense radius, in ascending agent id order.
    pub(crate) conflicts: Vec<Conflict>,
}

/// Stage 1's output: the immutable relevant world one agent update consumes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Observation {
    /// A path-following vehicle, or the walking skeleton's scripted body.
    Vehicle(VehicleObservation),
    /// A world-steering pedestrian.
    Pedestrian(PedestrianObservation),
}

/// Stage 3's output: the bounded motion command for one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum MotionCommand {
    /// No motion this step: the agent has no route waypoint to advance along.
    Idle,
    /// A bounded longitudinal speed for a path-following agent.
    Longitudinal {
        /// Speed in metres per second after the kernel's bounds and caps.
        speed_mps: f64,
    },
    /// A bounded heading and speed for a world-steering agent.
    Steering {
        /// Commanded heading in radians.
        heading_rad: f64,
        /// Speed in metres per second after the per-step bound.
        speed_mps: f64,
    },
}

/// Stage 1: relevant-world query.
///
/// The kernel implements this stage. It selects the constraint set, the leader,
/// the upcoming controls, the route target, and the nearby bodies, and reports
/// them as an immutable [`Observation`]; no model participates.
pub(crate) trait RelevantWorldQuery {
    /// Collect the observation one agent's update consumes this step.
    fn query_world(&mut self, index: usize) -> Observation;
}

/// Stage 2: tactical choice.
///
/// The kernel implements this stage. It maps an immutable observation to the
/// one maneuver the agent executes, recorded as a [`Tactic`].
pub(crate) trait TacticalChoice {
    /// Choose one tactic from an immutable observation.
    fn choose_tactic(&self, index: usize, observation: &Observation) -> Tactic;
}

/// Stage 3: motion control.
///
/// The kernel implements this stage. It converts the tactic into a bounded
/// [`MotionCommand`], calling the replaceable model through
/// [`crate::controller`] for the raw command and applying the kernel's bounds,
/// safety position caps, and emergency counters outside the model.
pub(crate) trait MotionControl {
    /// Convert a tactic into a bounded motion command for one step.
    fn command_motion(
        &mut self,
        index: usize,
        observation: &Observation,
        tactic: &Tactic,
        dt: f64,
    ) -> MotionCommand;
}

/// Stage 4: physical advance.
///
/// The kernel implements this stage. It integrates the pose the motion command
/// implies and emits the step's diagnostics, such as a route-exit despawn. The
/// swept-contact and safety diagnostics remain the kernel's separate per-tick
/// observation pass; they do not belong to a model.
pub(crate) trait PhysicalAdvance {
    /// Integrate the pose the command implies and emit diagnostics.
    fn advance_physics(
        &mut self,
        index: usize,
        observation: Observation,
        command: &MotionCommand,
        dt: f64,
    );
}
