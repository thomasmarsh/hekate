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
//! [`crate::Simulation`]. The tactic is reselected every tick and its
//! `started_at` is the simulation time it was selected at; its
//! [`ManeuverState`] is the agent's own lateral-maneuver state, which only the
//! kernel's maneuver pass changes.
//!
//! Increment 2's lateral maneuver lifecycle is the same record's other fields:
//! [`LateralManeuverRequest`] is the tactical leaves' input, [`ManeuverEdge`],
//! [`ManeuverAbortReason`], and [`ManeuverTransition`] are the recorded state
//! machine, and `crate::Simulation` drives it.

use std::cmp::Ordering;

use tangle_model::{CrossingId, FacilityId};

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
/// fixes the five states with these names. The state describes a *lateral*
/// maneuver, so an agent running a longitudinal tactic with no maneuver in
/// flight is [`ManeuverState::Following`]: a free-flowing or following agent is
/// never misreported as preparing a maneuver it did not select. The other four
/// states are the kernel's maneuver pass' own states ([`crate::Simulation`]),
/// and no state is spelled differently anywhere else.
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

/// The lateral maneuver one agent is attempting: a tactical leaf's input to the
/// kernel's claim-resolution seam.
///
/// A leaf that decides an agent should displace laterally records one of these
/// on the simulation ([`crate::Simulation::request_lateral_maneuver`]); the
/// kernel then fixes the target and the candidate corridor
/// ([`ManeuverState::Preparing`]), seeks the claim at the decision cadence, and
/// drives the maneuver to its completion, return, or abort. Whether the attempt
/// is admissible is the kernel's decision, not the requester's: the mode must
/// carry a bounded-steering envelope and a lateral policy, the target must lie
/// inside the usable corridor, and the candidate corridor must predict as
/// feasible. A requester only supplies the target and the passed obstacle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LateralManeuverRequest {
    /// The target signed offset in metres, in the agent's own travel frame.
    pub target_offset_m: f64,
    /// The passed obstacle: the body the maneuver displaces around, which is
    /// the contract's *target body*. The maneuver completes when the agent's
    /// rear envelope point is at least the mode's target clearance ahead of
    /// this body's front envelope along the travel direction, and aborts when
    /// this body disappears.
    pub passed_body: AgentId,
}

/// The tolerance in metres within which a returning or aborted maneuver counts
/// as having reached its return target, fixed by `docs/schema-v2-contract.md`
/// *Maneuver lifecycle*.
pub const SETTLE_TOLERANCE_M: f64 = 1e-3;

/// Which documented edge of the maneuver state machine a transition took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManeuverEdge {
    /// `following -> preparing`: a lateral tactic fixed a target and a
    /// candidate corridor.
    Attempted,
    /// `preparing -> committed`: the batch arbitration granted this agent's
    /// claim for its candidate corridor.
    Committed,
    /// `committed -> returning`, or `returning -> following`.
    Completed,
    /// `preparing -> aborted`, `committed -> aborted`, or `aborted ->
    /// following`.
    Aborted,
}

impl ManeuverEdge {
    /// Stable lowercase label for diagnostics and later event emission.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Attempted => "attempted",
            Self::Committed => "committed",
            Self::Completed => "completed",
            Self::Aborted => "aborted",
        }
    }
}

/// Why a maneuver aborted, which is the reason the contract records on an
/// `aborted` edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManeuverAbortReason {
    /// The claim was rejected by arbitration; the loser aborts.
    ClaimRejected,
    /// The passed body disappeared. A compiled target facility is immutable
    /// within a run, so a body is the only target that can vanish under an
    /// active maneuver.
    TargetLost,
    /// The hold timeout elapsed without a grant.
    HoldTimeout,
    /// The candidate corridor became infeasible.
    CorridorInfeasible,
    /// The predicted swept clearance fell below the commit policy's
    /// minimum after commitment.
    ClearanceLost,
}

impl ManeuverAbortReason {
    /// Stable lowercase label for diagnostics and later event emission.
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClaimRejected => "claim_rejected",
            Self::TargetLost => "target_lost",
            Self::HoldTimeout => "hold_timeout",
            Self::CorridorInfeasible => "corridor_infeasible",
            Self::ClearanceLost => "clearance_lost",
        }
    }
}

/// One recorded edge of one agent's maneuver lifecycle.
///
/// Exactly one record is produced per state transition, in a deterministic
/// order, so a later increment can emit one public event per transition without
/// re-deriving it. No public event is emitted here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManeuverTransition {
    /// The agent whose maneuver moved.
    pub agent: AgentId,
    /// The state it left.
    pub from: ManeuverState,
    /// The state it entered.
    pub to: ManeuverState,
    /// The documented edge the transition took.
    pub edge: ManeuverEdge,
    /// Why the maneuver aborted, present exactly on an edge into
    /// [`ManeuverState::Aborted`].
    pub reason: Option<ManeuverAbortReason>,
    /// Simulation time of the transition.
    pub time: SimTime,
}

/// The corridor space one maneuver claims, fixed when the maneuver was
/// attempted and never revised: a committed target never changes, so the
/// corridor a claim covers cannot shift under the claimant.
///
/// The lateral interval is laid against the facility's own reference frame (the
/// frame the compiled corridor and the reference geometry use), widened by the
/// claimant's target clearance, and the longitudinal interval spans the passed
/// obstacle's footprint plus both bodies' half lengths and that same clearance,
/// which is the region the maneuver has to occupy and keep clear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ManeuverCorridor {
    /// The facility traversal the corridor lies on.
    pub(crate) facility: FacilityId,
    /// Lowest arc length of the corridor in metres, along the facility
    /// reference.
    pub(crate) s_min_m: f64,
    /// Highest arc length of the corridor in metres.
    pub(crate) s_max_m: f64,
    /// Lowest signed lateral offset in metres, in the facility's own frame.
    pub(crate) d_min_m: f64,
    /// Highest signed lateral offset in metres, in the facility's own frame.
    pub(crate) d_max_m: f64,
}

/// One agent's claim for corridor space in one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CorridorClaim {
    /// The claimant.
    pub(crate) agent: AgentId,
    /// The corridor space the claim covers.
    pub(crate) corridor: ManeuverCorridor,
    /// Remaining distance in metres along the claimant's own travel direction
    /// from its current arc length to the entry of the corridor it contests.
    pub(crate) entry_distance_m: f64,
    /// Whether the claim is an existing commitment, which is never revoked.
    pub(crate) committed: bool,
    /// The claimant's target clearance in metres.
    pub(crate) target_clearance_m: f64,
}

impl CorridorClaim {
    /// The remaining distance in metres from the claimant's current arc length
    /// to the entry of `corridor` along its own travel direction.
    ///
    /// The entry is the near edge of the corridor in the direction of travel, so
    /// a claimant already inside the corridor measures zero. The value is an
    /// immutable input at collection time and never negative.
    pub(crate) fn entry_distance_m(
        corridor: &ManeuverCorridor,
        progress_m: f64,
        direction: f64,
    ) -> f64 {
        let entry_m = if direction < 0.0 {
            corridor.s_max_m
        } else {
            corridor.s_min_m
        };
        ((entry_m - progress_m) * direction).max(0.0)
    }
}

/// One claim's outcome in one arbitration batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaimOutcome {
    /// The claimant.
    pub(crate) agent: AgentId,
    /// Whether the claim was granted.
    pub(crate) granted: bool,
    /// The granted claimant whose corridor this claim conflicted with, when the
    /// claim lost.
    pub(crate) lost_to: Option<AgentId>,
}

/// Arbitrate one step's corridor claims as a batch.
///
/// The result is a pure function of the claims and never of the order they are
/// supplied in: the batch is ordered by the stable [`AgentId`] first, and every
/// comparison below is a total order on distinct agents, so source declaration,
/// candidate discovery, and insertion order cannot change a winner.
///
/// The winner of a pair of conflicting claims is the first claimant in the
/// contract's lexicographic key, computed only from immutable inputs: an agent
/// already committed outranks an agent preparing; then the smaller remaining
/// distance to the contested corridor's entry; then the smaller [`AgentId`].
/// Claims are ranked by that key and granted in order, and a claim is rejected
/// when it conflicts with an already granted one, so a claim that loses to a
/// committed claimant leaves that claimant's corridor untouched: an existing
/// commitment is never revoked mid-step.
///
/// Outcomes are returned in ascending [`AgentId`] order.
pub(crate) fn arbitrate_claims(claims: &[CorridorClaim]) -> Vec<ClaimOutcome> {
    let mut batch: Vec<&CorridorClaim> = claims.iter().collect();
    batch.sort_by_key(|claim| claim.agent);

    let mut ranking: Vec<usize> = (0..batch.len()).collect();
    ranking.sort_by(|&first, &second| beats(batch[first], batch[second]));

    let mut outcomes: Vec<Option<ClaimOutcome>> = vec![None; batch.len()];
    let mut granted: Vec<usize> = Vec::new();
    for index in ranking {
        let claim = batch[index];
        let conflict = granted
            .iter()
            .copied()
            .find(|&winner| claims_conflict(batch[winner], claim));
        outcomes[index] = Some(match conflict {
            Some(winner) => ClaimOutcome {
                agent: claim.agent,
                granted: false,
                lost_to: Some(batch[winner].agent),
            },
            None => {
                granted.push(index);
                ClaimOutcome {
                    agent: claim.agent,
                    granted: true,
                    lost_to: None,
                }
            }
        });
    }

    outcomes
        .into_iter()
        .map(|outcome| outcome.expect("every claim in the batch is decided"))
        .collect()
}

/// Whether `first` outranks `second` in the contract's winner key.
fn beats(first: &CorridorClaim, second: &CorridorClaim) -> Ordering {
    (if first.committed { 0_u8 } else { 1 })
        .cmp(&(if second.committed { 0 } else { 1 }))
        .then_with(|| first.entry_distance_m.total_cmp(&second.entry_distance_m))
        .then_with(|| first.agent.cmp(&second.agent))
}

/// Whether two claims contend for the same corridor space.
///
/// Two claims conflict when they lie on the same facility traversal and
/// granting both would put either corridor within the other's target clearance,
/// in the longitudinal and the lateral extent alike.
fn claims_conflict(first: &CorridorClaim, second: &CorridorClaim) -> bool {
    first.corridor.facility == second.corridor.facility
        && intervals_conflict(
            first.corridor.s_min_m,
            first.corridor.s_max_m,
            first.target_clearance_m,
            second.corridor.s_min_m,
            second.corridor.s_max_m,
            second.target_clearance_m,
        )
        && intervals_conflict(
            first.corridor.d_min_m,
            first.corridor.d_max_m,
            first.target_clearance_m,
            second.corridor.d_min_m,
            second.corridor.d_max_m,
            second.target_clearance_m,
        )
}

/// Whether two intervals come within each other's clearance of overlapping.
fn intervals_conflict(
    first_min: f64,
    first_max: f64,
    first_clearance: f64,
    second_min: f64,
    second_max: f64,
    second_clearance: f64,
) -> bool {
    first_min <= second_max + first_clearance && second_min <= first_max + second_clearance
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
    /// Where the maneuver the agent is executing stands in its lifecycle. For a
    /// longitudinal tactic this is the agent's own maneuver state, so it is
    /// [`ManeuverState::Following`] unless the kernel's maneuver pass holds the
    /// agent in a lateral state.
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
    /// A bounded route-relative steering step for a wheeled agent that carries
    /// route state.
    ///
    /// The heading and speed come from [`crate::steering::bounded_steering_step`],
    /// so the per-step heading change obeys the mode's `heading_rate_max_rad_s`
    /// and `lateral_accel_max_mps2` limits and the proposed world step stays
    /// inside the usable corridor. The physical advance integrates this command
    /// in world coordinates and projects the result back onto the route; it
    /// never writes a target offset to `d` or to a world position.
    RouteSteering {
        /// Commanded world heading in radians.
        heading_rad: f64,
        /// Speed in metres per second used for the step.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A corridor on `facility` covering arc lengths `s` and lateral offsets
    /// `d`, in the facility's own frame.
    fn corridor(facility: usize, s: (f64, f64), d: (f64, f64)) -> ManeuverCorridor {
        ManeuverCorridor {
            facility: FacilityId::from_index(facility),
            s_min_m: s.0,
            s_max_m: s.1,
            d_min_m: d.0,
            d_max_m: d.1,
        }
    }

    /// A claim with an explicit entry distance, so a test pins the key clauses
    /// directly rather than deriving them from geometry.
    fn claim(
        agent: usize,
        corridor: ManeuverCorridor,
        entry_distance_m: f64,
        committed: bool,
    ) -> CorridorClaim {
        CorridorClaim {
            agent: AgentId::from_index(agent),
            corridor,
            entry_distance_m,
            committed,
            target_clearance_m: 0.5,
        }
    }

    /// Outcome of one agent, so a test reads the batch by identifier.
    fn outcome(outcomes: &[ClaimOutcome], agent: usize) -> ClaimOutcome {
        outcomes
            .iter()
            .find(|outcome| outcome.agent == AgentId::from_index(agent))
            .copied()
            .expect("the batch decides every claim")
    }

    /// Two claims for the same gap with the same entry distance are decided by
    /// the stable agent id, the third clause of the winner key.
    #[test]
    fn a_same_gap_tie_is_won_by_the_lower_agent_id() {
        let corridor = corridor(0, (40.0, 60.0), (0.0, 1.5));
        let outcomes = arbitrate_claims(&[
            claim(3, corridor, 5.0, false),
            claim(1, corridor, 5.0, false),
            claim(2, corridor, 5.0, false),
        ]);
        assert_eq!(
            outcomes
                .iter()
                .map(|outcome| outcome.agent)
                .collect::<Vec<_>>(),
            [1, 2, 3]
                .into_iter()
                .map(AgentId::from_index)
                .collect::<Vec<_>>(),
            "outcomes are reported in ascending agent id order"
        );
        assert!(outcome(&outcomes, 1).granted);
        assert_eq!(outcome(&outcomes, 2).lost_to, Some(AgentId::from_index(1)));
        assert_eq!(outcome(&outcomes, 3).lost_to, Some(AgentId::from_index(1)));
        assert!(!outcome(&outcomes, 2).granted);
    }

    /// An existing commitment outranks a candidate: the first clause of the
    /// winner key, and the reason a winner is never revoked mid-step.
    #[test]
    fn a_committed_claim_beats_a_preparing_claim() {
        let corridor = corridor(0, (40.0, 60.0), (0.0, 1.5));
        let outcomes = arbitrate_claims(&[
            claim(0, corridor, 30.0, false),
            claim(1, corridor, 1.0, true),
        ]);
        assert!(outcome(&outcomes, 1).granted);
        assert_eq!(outcome(&outcomes, 0).lost_to, Some(AgentId::from_index(1)));
    }

    /// Among preparing claims the hint is the smaller remaining distance along
    /// the claimant's own travel direction to the contested entry.
    #[test]
    fn the_smaller_entry_distance_wins_between_preparing_claims() {
        let corridor = corridor(0, (40.0, 60.0), (0.0, 1.5));
        let outcomes = arbitrate_claims(&[
            claim(0, corridor, 12.0, false),
            claim(1, corridor, 4.0, false),
        ]);
        assert!(outcome(&outcomes, 1).granted);
        assert_eq!(outcome(&outcomes, 0).lost_to, Some(AgentId::from_index(1)));
    }

    /// The batch is invariant to the order the claims arrive in, which is what
    /// keeps behavior independent of agent iteration order: one committed claim
    /// on its own corridor, and two preparing claims contesting a second.
    #[test]
    fn arbitration_is_invariant_to_insertion_order() {
        let held_corridor = corridor(0, (40.0, 60.0), (4.0, 5.0));
        let corridor = corridor(0, (40.0, 60.0), (0.0, 1.5));
        let far = claim(4, corridor, 9.0, false);
        let near = claim(2, corridor, 1.0, false);
        let held = claim(7, held_corridor, 20.0, true);
        let forward = arbitrate_claims(&[far, near, held]);
        let reversed = arbitrate_claims(&[held, near, far]);
        let interleaved = arbitrate_claims(&[near, far, held]);
        assert_eq!(forward, reversed);
        assert_eq!(forward, interleaved);
        assert!(outcome(&forward, 7).granted);
        assert!(outcome(&forward, 2).granted);
        assert!(!outcome(&forward, 4).granted);
        assert_eq!(outcome(&forward, 4).lost_to, Some(AgentId::from_index(2)));
    }

    /// Corridors that do not overlap along the facility are granted together:
    /// only a contested corridor is arbitrated.
    #[test]
    fn disjoint_corridors_are_both_granted() {
        let outcomes = arbitrate_claims(&[
            claim(0, corridor(0, (0.0, 10.0), (0.0, 1.0)), 1.0, false),
            claim(1, corridor(0, (90.0, 100.0), (0.0, 1.0)), 1.0, false),
        ]);
        assert!(outcomes.iter().all(|outcome| outcome.granted));

        // A lateral band more than the target clearance apart is free too.
        let outcomes = arbitrate_claims(&[
            claim(0, corridor(0, (40.0, 60.0), (0.0, 1.0)), 1.0, false),
            claim(1, corridor(0, (40.0, 60.0), (3.0, 4.0)), 1.0, false),
        ]);
        assert!(outcomes.iter().all(|outcome| outcome.granted));

        // And the same corridor space on a different facility is a different
        // traversal, so it is not contested.
        let outcomes = arbitrate_claims(&[
            claim(0, corridor(0, (40.0, 60.0), (0.0, 1.0)), 1.0, false),
            claim(1, corridor(1, (40.0, 60.0), (0.0, 1.0)), 1.0, false),
        ]);
        assert!(outcomes.iter().all(|outcome| outcome.granted));
    }

    /// A corridor that comes within the other claimant's target clearance of
    /// the shared space conflicts even when the bands themselves are disjoint or
    /// only touch; a gap of exactly the two target clearances is admissible,
    /// exactly as the prediction's feasibility admits a clearance at the target.
    #[test]
    fn a_corridor_within_the_target_clearance_conflicts() {
        // Lateral bands 1.0 m apart with a 0.5 m target clearance each: the
        // widened bands overlap.
        let outcomes = arbitrate_claims(&[
            claim(0, corridor(0, (40.0, 60.0), (0.0, 1.0)), 1.0, false),
            claim(1, corridor(0, (40.0, 60.0), (1.0, 2.0)), 1.0, false),
        ]);
        assert!(outcome(&outcomes, 0).granted);
        assert!(!outcome(&outcomes, 1).granted);

        // Bands 2.0 m apart leave exactly the two target clearances, so both
        // are granted.
        let outcomes = arbitrate_claims(&[
            claim(0, corridor(0, (40.0, 60.0), (0.0, 1.0)), 1.0, false),
            claim(1, corridor(0, (40.0, 60.0), (2.0, 3.0)), 1.0, false),
        ]);
        assert!(outcomes.iter().all(|outcome| outcome.granted));

        // Longitudinally: the first corridor ends exactly where the second
        // begins, so the two touch and the clearance still binds.
        let outcomes = arbitrate_claims(&[
            claim(0, corridor(0, (0.0, 10.0), (0.0, 1.0)), 1.0, false),
            claim(1, corridor(0, (10.0, 20.0), (0.0, 1.0)), 1.0, false),
        ]);
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.granted).count(),
            1,
            "touching corridors contest the same space"
        );
    }

    /// A claim with no conflict is granted, and a lone claim is always granted.
    #[test]
    fn a_single_claim_is_always_granted() {
        let outcomes =
            arbitrate_claims(&[claim(0, corridor(0, (40.0, 60.0), (0.0, 1.0)), 1.0, false)]);
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].granted);
        assert_eq!(outcomes[0].lost_to, None);
        assert!(arbitrate_claims(&[]).is_empty());
    }

    /// The entry distance is measured from the claimant's own arc length along
    /// its own travel direction, and a claimant already inside its corridor
    /// measures zero; the entry distance never falls below zero.
    #[test]
    fn the_entry_distance_is_measured_along_the_travel_direction() {
        let corridor = corridor(0, (40.0, 60.0), (0.0, 1.0));
        assert!(
            (CorridorClaim::entry_distance_m(&corridor, 30.0, 1.0) - 10.0).abs() < 1e-12,
            "a forward claimant measures to the low edge"
        );
        assert!(
            (CorridorClaim::entry_distance_m(&corridor, 70.0, -1.0) - 10.0).abs() < 1e-12,
            "a reverse claimant measures to the high edge"
        );
        assert_eq!(
            CorridorClaim::entry_distance_m(&corridor, 55.0, 1.0),
            0.0,
            "a claimant inside its corridor is at its entry"
        );
        assert_eq!(
            CorridorClaim::entry_distance_m(&corridor, 80.0, 1.0),
            0.0,
            "a corridor left behind measures zero, never a negative distance"
        );
    }

    /// Every documented edge and abort reason has a stable label, so a later
    /// event surface can name them without a second spelling.
    #[test]
    fn edges_and_reasons_have_stable_labels() {
        let edges = [
            (ManeuverEdge::Attempted, "attempted"),
            (ManeuverEdge::Committed, "committed"),
            (ManeuverEdge::Completed, "completed"),
            (ManeuverEdge::Aborted, "aborted"),
        ];
        for (edge, label) in edges {
            assert_eq!(edge.label(), label);
        }
        let reasons = [
            (ManeuverAbortReason::ClaimRejected, "claim_rejected"),
            (ManeuverAbortReason::TargetLost, "target_lost"),
            (ManeuverAbortReason::HoldTimeout, "hold_timeout"),
            (
                ManeuverAbortReason::CorridorInfeasible,
                "corridor_infeasible",
            ),
            (ManeuverAbortReason::ClearanceLost, "clearance_lost"),
        ];
        for (reason, label) in reasons {
            assert_eq!(reason.label(), label);
        }
        assert_eq!(SETTLE_TOLERANCE_M, 1e-3);
    }
}
