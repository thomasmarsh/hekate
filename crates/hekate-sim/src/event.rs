//! Typed state transitions observed during a step.
//!
//! # Record union
//!
//! [`Event`] is one closed union that both modes emit through, and
//! [`EVENT_VERSION`] describes the union as a whole: a consumer that keys on the
//! version may rely on the full variant set below, not on a subset. Three
//! families live in it:
//!
//! - lifecycle records that carry dense identifiers — [`Event::Spawned`],
//!   [`Event::Despawned`], and the kernel's own control transitions
//!   ([`Event::Yielded`], [`Event::ControlTransition`]);
//! - safety records produced by the tick's body scan
//!   ([`crate::safety`]) — [`Event::Collision`], [`Event::NearMiss`],
//!   [`Event::Violation`], [`Event::Entry`], [`Event::Exit`], and
//!   [`Event::Queue`];
//! - maneuver and rule records of the state changes the maneuver and wrong-way
//!   stages already own — [`Event::Maneuver`], [`Event::FacilityTransition`],
//!   and [`Event::OpposingTraversal`], whose `reason` fields are the closed code
//!   sets [`ManeuverReasonCode`] and [`crate::WrongWayReason`] — and the close
//!   pass a completed overtaking interval closes, [`Event::ClosePass`], whose
//!   band payload is the closed set [`ClosePassBand`]. A run whose scenario
//!   authors no lateral, transition, opposing-traversal, or pass shape emits
//!   none of them, so a Phase 1 or Increment 1 stream is unchanged apart from the
//!   recorded [`EVENT_VERSION`];
//! - the two articulated-chain records: [`Event::ArticulationLimitExceeded`],
//!   from the per-tick hitch check ([`crate::articulated`]), and
//!   [`Event::ArticulatedSegmentContact`], the segment-precise contact record
//!   [`crate::safety`] emits in place of [`Event::Collision`] when a
//!   contacting pair involves a chain. A run with no `ArticulatedWheeled`
//!   agent emits neither.
//!
//! # Within-tick order
//!
//! Events are emitted in a documented, stable order that never depends on a
//! hash-map iteration or on the order emission points happen to run. Every
//! record carries [`Event::order_key`]; a step sorts its buffer by that key, so
//! a consumer sees ascending [`AgentId`] first, then ascending event kind
//! ([`EventKind::order`]), then the variant's own stable key — up to five
//! ascending components in the contract's order, so a partner agent, crossing,
//! region, path, tactic, facility, movement, or state edge can all carry one —
//! and finally the record's edge flag (`yielding`, `contacting`, `entering`,
//! `joined`, `active`, or a violation or control kind). The sort is stable, so
//! two records that agree on all of that are the same variant with the same
//! key, which the once-per-transition lifecycle forbids; a residual tie can
//! therefore only be two identical records, and their relative order is the
//! kernel's emission order.
//!
//! # Emission lifecycle
//!
//! Every variant is edge-triggered, never per-step:
//!
//! - [`Event::Spawned`] and [`Event::Despawned`] emit once when the agent enters
//!   or leaves the world; they also delimit an agent's stream, so a safety state
//!   that a despawn ends is cleared without a further event.
//! - [`Event::Yielded`] emits once per yield transition, and
//!   [`Event::ControlTransition`] once per recorded controller-state change.
//! - [`Event::Collision`], [`Event::NearMiss`], [`Event::Entry`], [`Event::Exit`],
//!   and [`Event::Queue`] each emit once when their per-tick predicate turns
//!   true and once when it turns false; see [`crate::safety`] for the exact
//!   predicates and the no-miss argument.
//! - [`Event::Violation`] emits once per recorded noncompliant crossing action
//!   (a red-light run or a crossing against a forbidding signal), reusing the
//!   decision state the kernel already records.
//! - [`Event::Maneuver`] emits once per legal maneuver state-machine edge,
//!   [`Event::FacilityTransition`] once per recorded facility handoff, and
//!   [`Event::OpposingTraversal`] once when an opposing-traversal interval opens
//!   and once when it closes; each reads a state change the maneuver and
//!   wrong-way stages already record, so a retry cannot duplicate one.
//! - [`Event::ClosePass`] emits once per completed overtaking interval, at the
//!   tick the observation closes on pass completion, an aborted pass, or a
//!   participant's despawn.
//! - [`Event::ArticulationLimitExceeded`] emits once when a hitch's angle
//!   crosses above its compiled `articulation_limit_rad` and once when it
//!   falls back inside it, exactly like [`Event::Collision`]'s contact edge; a
//!   despawn while exceeding ends the state without a further event.

use hekate_model::{
    ClearanceBandId, ConflictRegionId, CrossingId, FacilityId, MovementDirection, MovementId,
    NominalDirection, PathId, PermissionEffect, TacticKind,
};

use crate::agent::{AgentId, AgentMode};
use crate::close_pass::ClosePassBand;
use crate::stage::{
    ManeuverAbortReason, ManeuverEdge, ManeuverReason, ManeuverState, PassSide, TransitionKind,
};
use crate::wrong_way::WrongWayReason;

/// Version of the typed event schema emitted by this build.
///
/// Freeze this in the run manifest so a later trace invalidation is deliberate.
/// Increment it when an existing event's meaning, fields, or payload semantics
/// change; adding a new variant is also a consumer-visible change, so bump it
/// before any golden trace is regenerated for that reason.
///
/// Version 2 is the first version that describes the full record union: version
/// 1 named only `Spawned`, `Despawned`, and `Yielded`, and `Spawned` did not
/// carry the agent's mode. Version 2 adds the agent mode to `Spawned` and the
/// typed safety and control variants ([`Event::Collision`],
/// [`Event::NearMiss`], [`Event::Violation`], [`Event::Entry`], [`Event::Exit`],
/// [`Event::Queue`], and [`Event::ControlTransition`]).
///
/// Version 3 adds the maneuver and rule records [`Event::Maneuver`],
/// [`Event::FacilityTransition`], and [`Event::OpposingTraversal`] with the
/// closed reason set [`ManeuverReasonCode`], and the close-pass record
/// [`Event::ClosePass`]. No existing variant gains or loses a field, changes
/// meaning, or changes its [`EventKind::order`] position, so a version-2 trace
/// stays readable apart from the recorded version. This is the union's one
/// bump: the increment's fourth record, a close-pass observation, lands under
/// this same version rather than bumping again.
///
/// [`Event::ArticulationLimitExceeded`] lands under this same version 3 for
/// the identical reason: it is a new variant with no existing variant
/// changed, and no scenario compiled before `ArticulatedWheeled` existed can
/// ever emit it, so no golden trace is regenerated by adding it.
///
/// [`Event::ArticulatedSegmentContact`] is a second additive articulated-chain
/// record under this same unbumped version 3, for the identical reason: it is
/// a new variant, no existing variant's fields or meaning change, and no
/// scenario compiled before `ArticulatedWheeled` existed can ever emit it.
pub const EVENT_VERSION: u32 = 3;

/// Why an agent left the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DespawnReason {
    /// The agent reached the far end of its guide path.
    ExitedPath,
}

/// Which record a [`Event`] is, without its payload.
///
/// The declaration order is the documented within-tick kind order returned by
/// [`EventKind::order`]; see the module docs.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    /// An agent entered the world.
    Spawned,
    /// An agent left the world.
    Despawned,
    /// A yield transition began or ended.
    Yielded,
    /// Two bodies came into contact or separated.
    Collision,
    /// Two bodies entered or left the sub-threshold separation band.
    NearMiss,
    /// A body performed a crossing action its governing signal forbade.
    Violation,
    /// A body entered a safety region.
    Entry,
    /// A body left a safety region.
    Exit,
    /// An agent reached or left a stopped-and-waiting state.
    Queue,
    /// A recorded controller state changed.
    ControlTransition,
    /// An agent transitioned between two documented maneuver states.
    Maneuver,
    /// An agent handed off between two facilities.
    FacilityTransition,
    /// An agent opened or closed an opposing traversal of a facility.
    OpposingTraversal,
    /// A completed overtaking interval closed with its clearance evidence.
    ClosePass,
    /// A hitch's articulation angle crossed its compiled limit, or fell back
    /// inside it.
    ArticulationLimitExceeded,
    /// A specific segment pair of an articulated-chain contact came into
    /// contact, or separated.
    ArticulatedSegmentContact,
}

impl EventKind {
    /// Position of this kind in the documented within-tick event order.
    pub const fn order(self) -> u8 {
        self as u8
    }
}

/// A safety region whose boundary a body can cross.
///
/// Regions are authored scenario geometry: a crossing names the region its
/// pedestrians cross, and a conflict region names the area two movements share.
/// The two identifiers live in separate id spaces, so the variant tag is part
/// of the region's stable order key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegionKey {
    /// The region of a compiled crossing.
    Crossing(CrossingId),
    /// A compiled conflict region.
    ConflictRegion(ConflictRegionId),
}

impl RegionKey {
    /// Dense identifier of the region within its own id space.
    pub const fn get(self) -> u32 {
        match self {
            Self::Crossing(crossing) => crossing.get(),
            Self::ConflictRegion(region) => region.get(),
        }
    }

    /// Tag that separates the two region id spaces in the stable order key.
    pub const fn tag(self) -> u8 {
        match self {
            Self::Crossing(_) => 0,
            Self::ConflictRegion(_) => 1,
        }
    }
}

/// Which rule breach a [`Event::Violation`] records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViolationKind {
    /// A vehicle crossed its stop line while the governing head forbade it.
    RanRedLight,
    /// A pedestrian entered a crossing region while its signal forbade it.
    CrossedAgainstSignal,
}

impl ViolationKind {
    /// Position of this kind in the stable order key.
    pub const fn order(self) -> u8 {
        match self {
            Self::RanRedLight => 0,
            Self::CrossedAgainstSignal => 1,
        }
    }

    /// Short stable label for traces and inspectors.
    pub const fn label(self) -> &'static str {
        match self {
            Self::RanRedLight => "ran_red_light",
            Self::CrossedAgainstSignal => "crossed_against_signal",
        }
    }
}

/// Which controller state a [`Event::ControlTransition`] records.
///
/// A control transition is edge-triggered on the decision record the kernel
/// already stores per agent, so the event and
/// [`crate::Simulation::agent_decision`] never disagree. The yield transition
/// keeps its own variant, [`Event::Yielded`], because it carries the crossing
/// it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ControlTransitionKind {
    /// A vehicle began or ended holding at a signal-controlled stop line.
    SignalStop,
    /// A pedestrian began or ended waiting at a signal-controlled crossing.
    CrossingWait,
}

impl ControlTransitionKind {
    /// Position of this kind in the stable order key.
    pub const fn order(self) -> u8 {
        match self {
            Self::SignalStop => 0,
            Self::CrossingWait => 1,
        }
    }

    /// Short stable label for traces and inspectors.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SignalStop => "signal_stop",
            Self::CrossingWait => "crossing_wait",
        }
    }
}

/// The closed code set an [`Event::Maneuver`] records as its reason.
///
/// `docs/schema-v2-contract.md` *Increment 2 events and metrics* fixes one
/// closed `ManeuverReason` set covering both the eligibility rejections a
/// lateral tactic reports and the terminations of a maneuver already in flight.
/// The two source enums already spell those codes —
/// [`crate::ManeuverReason`] carries the selection and rejection codes,
/// [`crate::ManeuverAbortReason`] the termination codes — so this set is the one
/// event surface they map into, and no code has a second spelling.
/// [`Self::Settled`] is the completing edge's own reason, which no source enum
/// records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManeuverReasonCode {
    /// Selection: a visible slower leader ahead is the obstacle the pass
    /// displaces around.
    SlowerLeader,
    /// Rejection: the mode's compiled tactics carry no `pass` or `overtake`
    /// capability.
    Capability,
    /// Rejection: an applicable `overtake` statement prohibits passing, or the
    /// facility offers no lateral maneuver target.
    NoPermission,
    /// Rejection: no visible slower leader, or no route benefit from passing
    /// one, or the leader is beyond the maneuver reach.
    NoBenefit,
    /// Rejection: the facility is too narrow for the pass on the selected side.
    InsufficientWidth,
    /// Rejection: the mode declares no lateral target clearance or horizon, or
    /// the candidate corridor predicts infeasible over the horizon.
    NoCorridor,
    /// Rejection or termination: the crossing would enter a traversal the
    /// applicable rule does not permit.
    BoundaryForbidden,
    /// Termination: the claim was rejected by arbitration; the loser aborts.
    ClaimRejected,
    /// Termination: the passed body disappeared under an active maneuver.
    TargetLost,
    /// Termination: the hold timeout elapsed without a grant.
    HoldTimeout,
    /// Termination: the candidate corridor became infeasible.
    CorridorInfeasible,
    /// Termination: the predicted swept clearance fell below the commit policy's
    /// minimum after commitment.
    ClearanceLost,
    /// Completion: the maneuver reached its target offset and settled.
    Settled,
}

impl ManeuverReasonCode {
    /// Short stable label for traces and inspectors, the contract's code.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SlowerLeader => "slower_leader",
            Self::Capability => "capability",
            Self::NoPermission => "no_permission",
            Self::NoBenefit => "no_benefit",
            Self::InsufficientWidth => "insufficient_width",
            Self::NoCorridor => "no_corridor",
            Self::BoundaryForbidden => "boundary_forbidden",
            Self::ClaimRejected => "claim_rejected",
            Self::TargetLost => "target_lost",
            Self::HoldTimeout => "hold_timeout",
            Self::CorridorInfeasible => "corridor_infeasible",
            Self::ClearanceLost => "clearance_lost",
            Self::Settled => "settled",
        }
    }
}

impl From<ManeuverReason> for ManeuverReasonCode {
    fn from(reason: ManeuverReason) -> Self {
        match reason {
            ManeuverReason::SlowerLeader => Self::SlowerLeader,
            ManeuverReason::Capability => Self::Capability,
            ManeuverReason::NoPermission => Self::NoPermission,
            ManeuverReason::NoBenefit => Self::NoBenefit,
            ManeuverReason::InsufficientWidth => Self::InsufficientWidth,
            ManeuverReason::NoCorridor => Self::NoCorridor,
            ManeuverReason::BoundaryForbidden => Self::BoundaryForbidden,
        }
    }
}

impl From<ManeuverAbortReason> for ManeuverReasonCode {
    fn from(reason: ManeuverAbortReason) -> Self {
        match reason {
            ManeuverAbortReason::ClaimRejected => Self::ClaimRejected,
            ManeuverAbortReason::TargetLost => Self::TargetLost,
            ManeuverAbortReason::HoldTimeout => Self::HoldTimeout,
            ManeuverAbortReason::CorridorInfeasible => Self::CorridorInfeasible,
            ManeuverAbortReason::ClearanceLost => Self::ClearanceLost,
            ManeuverAbortReason::BoundaryForbidden => Self::BoundaryForbidden,
        }
    }
}

/// A typed transition emitted by a step.
///
/// Events carry dense identifiers and unit-suffixed fields so observers can
/// build traces without touching internal state. See [`EVENT_VERSION`] for the
/// schema version recorded in run provenance, and [`Event::order_key`] for the
/// documented within-tick order.
///
/// A record is [`Clone`] but not `Copy`: [`Event::ClosePass`] carries the
/// participating bands as a variable-length list, so a consumer that holds an
/// event owns it and a consumer that borrows one clones it to take it by value.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// An agent entered the world at a path position.
    Spawned {
        /// The new agent.
        agent: AgentId,
        /// Which mode the new agent belongs to, so a consumer can size and
        /// interpret it without a second lookup.
        mode: AgentMode,
        /// The guide path it entered on.
        path: PathId,
        /// Arc-length entry position in metres.
        distance_m: f64,
    },
    /// An agent left the world.
    Despawned {
        /// The departing agent.
        agent: AgentId,
        /// The guide path it was following.
        path: PathId,
        /// Why it left.
        reason: DespawnReason,
    },
    /// A vehicle began or ended yielding to an occupied crossing.
    ///
    /// Emitted once per transition, so a consumer sees exactly the two edges
    /// of the yield. The vehicle yields while a pedestrian body overlaps the
    /// crossing region and its front bumper is still upstream of the crossing
    /// entry; see [`crate::Simulation`] for the yielding rule.
    Yielded {
        /// The yielding vehicle.
        agent: AgentId,
        /// The crossing it yields to.
        crossing: CrossingId,
        /// `true` when the yield began, `false` when it ended.
        yielding: bool,
    },
    /// Two bodies came into contact or separated.
    ///
    /// Contact is a signed surface clearance at or below zero: touching counts,
    /// and `clearance_m < 0` is an overlap of that depth. The predicate is swept
    /// over the tick, so a body pair that touches between two tick endpoints and
    /// separates again inside the tick is still a contact. `agent` is the lower
    /// [`AgentId`] of the pair and `other` the higher, so the pair has one
    /// canonical spelling. See [`crate::safety`] for the lifecycle.
    Collision {
        /// Lower agent id of the contacting pair.
        agent: AgentId,
        /// Higher agent id of the contacting pair.
        other: AgentId,
        /// Signed surface clearance in metres at the tick end: zero or
        /// negative for a contact, positive once the pair has separated.
        clearance_m: f64,
        /// `true` when contact began, `false` when it ended.
        contacting: bool,
    },
    /// Two bodies entered or left the sub-threshold separation band.
    ///
    /// A near miss is a pair whose signed surface clearance is positive but at
    /// or below [`crate::safety::NEAR_MISS_THRESHOLD_M`] at some time in the
    /// tick, and which is not in contact. The two predicates are nested rather
    /// than one predicate with a special case: the contact band sits inside the
    /// near-miss band, so a pair in contact has no open near miss, and the tick
    /// that begins a contact also ends the pair's near miss. `agent` is the
    /// lower [`AgentId`] of the pair and `other` the higher.
    NearMiss {
        /// Lower agent id of the separating pair.
        agent: AgentId,
        /// Higher agent id of the separating pair.
        other: AgentId,
        /// Signed surface clearance in metres at the tick end.
        clearance_m: f64,
        /// `true` when the pair entered the band, `false` when it left.
        entering: bool,
    },
    /// A body performed a crossing action its governing signal forbade.
    ///
    /// Emitted once per crossing action, from the decision the kernel already
    /// records: a vehicle that crossed its stop line while the recorded
    /// decision was to proceed past a forbidding head, or a pedestrian that
    /// entered a crossing region while the recorded decision was to cross
    /// against a forbidding signal. The recorded decision's reason says whether
    /// the breach was the noncompliant choice or an entry the body could not
    /// brake out of; see [`crate::Simulation::agent_decision`] and
    /// [`crate::Simulation::agent_pedestrian_decision`].
    Violation {
        /// The breaching agent.
        agent: AgentId,
        /// Which breach it was.
        kind: ViolationKind,
    },
    /// A body entered a safety region.
    ///
    /// The predicate is the body's tick-swept extent reaching the region, so a
    /// region a body reaches inside a tick is never missed; see
    /// [`crate::safety`] for the conservative bound.
    Entry {
        /// The entering body.
        agent: AgentId,
        /// The region it entered.
        region: RegionKey,
    },
    /// A body left a safety region.
    Exit {
        /// The leaving body.
        agent: AgentId,
        /// The region it left.
        region: RegionKey,
    },
    /// An agent reached or left a stopped-and-waiting state.
    ///
    /// The state is the agent's longitudinal speed at the tick end being at or
    /// below [`crate::safety::QUEUE_STOP_SPEED_MPS`], so a standing queue at a
    /// stop line, a yield point, or behind a leader reports the same record.
    /// Emitted once per formation and once per departure; a despawn closes an
    /// open state with a departure.
    Queue {
        /// The standing or departing agent.
        agent: AgentId,
        /// `true` when the agent came to rest, `false` when it moved off.
        joined: bool,
    },
    /// A recorded controller state changed.
    ///
    /// The state is the decision record the kernel already stores: a vehicle
    /// holding at a signal-controlled stop line
    /// ([`ControlTransitionKind::SignalStop`]) or a pedestrian waiting at a
    /// signal-controlled crossing ([`ControlTransitionKind::CrossingWait`]).
    /// Emitted once per change, and never per step.
    ControlTransition {
        /// The agent whose controller state changed.
        agent: AgentId,
        /// Which controller state changed.
        control: ControlTransitionKind,
        /// `true` when the state began, `false` when it ended.
        active: bool,
    },
    /// An agent transitioned between two documented maneuver states.
    ///
    /// Emitted once per legal maneuver state-machine edge, at the step the
    /// transition happens. The record names the tactic the maneuver belongs to,
    /// both states and the edge between them, the passed body when the maneuver
    /// displaces around one, the facility traversals and the target offset, and
    /// why the edge happened, so an observer reads the whole attempt without a
    /// trajectory sample.
    Maneuver {
        /// The maneuvering agent.
        agent: AgentId,
        /// Which tactic the maneuver belongs to.
        kind: TacticKind,
        /// The state the maneuver left.
        from: ManeuverState,
        /// The state it entered.
        to: ManeuverState,
        /// Which documented edge it took.
        edge: ManeuverEdge,
        /// The body the maneuver displaces around, absent for a maneuver that
        /// targets an offset rather than a body.
        partner: Option<AgentId>,
        /// The facility traversal the maneuver started on.
        source_facility: FacilityId,
        /// The facility the maneuver targets, absent for a same-facility
        /// maneuver.
        target_facility: Option<FacilityId>,
        /// Target signed offset in metres, in the agent's own travel frame.
        target_offset_m: f64,
        /// The side the displacement claims, in the agent's own travel frame.
        side: PassSide,
        /// Why the edge happened.
        reason: ManeuverReasonCode,
    },
    /// An agent handed off between two facilities.
    ///
    /// Emitted once per handoff at the handoff step, from the record the kernel
    /// already produces, so the event and [`crate::FacilityTransitionRecord`]
    /// never disagree. `permitted: false` is the forbidden-boundary fact: the
    /// agent crossed into a traversal the applicable rule does not permit.
    FacilityTransition {
        /// The agent that changed facility.
        agent: AgentId,
        /// The facility the agent left.
        from_facility: FacilityId,
        /// The facility the agent entered.
        to_facility: FacilityId,
        /// The traversal direction it travelled on the facility it left.
        from_direction: MovementDirection,
        /// The traversal direction it travels on the facility it entered.
        to_direction: MovementDirection,
        /// Which geometric handoff it took.
        via: TransitionKind,
        /// The side of the crossing in the agent's own travel frame.
        side: PassSide,
        /// Route progress in metres at the handoff, on the facility it left.
        s_m: f64,
        /// Signed lateral offset in metres at the handoff, in the agent's own
        /// travel frame on the facility it left.
        d_m: f64,
        /// Whether the applicable rule permitted the destination traversal.
        permitted: bool,
    },
    /// An agent opened or closed an opposing traversal of a facility.
    ///
    /// Emitted once when the interval opens and once when it closes, from the
    /// wrong-way decision the kernel already records. `reason` is the decision's
    /// own reason and `perceived_rule` the permission statement the agent acted
    /// under, absent when no statement applies. A permitted or obligated
    /// opposing traversal is a legal traversal: it is recorded with
    /// `violating: false` and never counted as a violation.
    OpposingTraversal {
        /// The traversing agent.
        agent: AgentId,
        /// The facility traversed against its rule direction.
        facility: FacilityId,
        /// The movement connector it entered on, absent for a facility
        /// traversal.
        movement: Option<MovementId>,
        /// The direction the agent actually travels.
        direction: MovementDirection,
        /// The facility's authored nominal direction.
        nominal_direction: NominalDirection,
        /// The permission statement the agent acted under, absent when none
        /// applies.
        perceived_rule: Option<PermissionEffect>,
        /// Why the decision selected the opposing option.
        reason: WrongWayReason,
        /// `true` for a traversal the applicable rule does not permit.
        violating: bool,
        /// `true` when the interval opened, `false` when it closed.
        entering: bool,
    },
    /// A completed overtaking interval closed with its exact clearance evidence.
    ///
    /// Emitted once when the observation closes: the faster body's pass of a
    /// slower one completed (the pair stopped being alongside), the pass was
    /// aborted, or a participant despawned. `agent` is the passing body and
    /// `partner` the passed body, so the pair keeps its actor and passed-user
    /// roles and never collapses to a symmetric distance. `min_clearance_m` is
    /// the least body-to-body swept clearance over the interval, the boundary
    /// crossing excluded, and `bands` reports each participating clearance
    /// band's accumulated duration in declaration order.
    ClosePass {
        /// The passing agent: the faster body of the pair.
        agent: AgentId,
        /// The passed body.
        partner: AgentId,
        /// The facility the pass happened on.
        facility: FacilityId,
        /// The side the pass claims, in the passing agent's travel frame.
        side: PassSide,
        /// Least signed surface clearance in metres over the interval.
        min_clearance_m: f64,
        /// Simulated time of that minimum, in seconds.
        min_clearance_time_s: f64,
        /// Relative speed along the shared reference at that minimum, in metres
        /// per second.
        relative_speed_mps: f64,
        /// Each participating band's accumulated duration, in declaration order.
        bands: Vec<ClosePassBand>,
        /// The participating bands whose observed minimum fell below their
        /// threshold and that record a violation.
        violating_bands: Vec<ClearanceBandId>,
        /// `true` when either participant crossed a facility boundary during the
        /// interval.
        crossed_boundary: bool,
        /// `true` when either participant traversed against its facility's
        /// nominal direction during the interval.
        entered_opposing: bool,
    },
    /// An articulated chain's hitch angle crossed its compiled
    /// `articulation_limit_rad`, or fell back inside it.
    ///
    /// Emitted once per edge, exactly like [`Event::Collision`]'s contact
    /// flag: a jackknife is a typed, observable fact, never a silent clip or a
    /// per-tick repeat while it persists. `hitch_index` is the trailing
    /// segment's own index in the chain (`1` for the hitch between the lead
    /// and the first trailer, and so on), matching
    /// `hekate_model::BodySegment` order. See [`crate::articulated`] for the
    /// deterministic pose integration this reads.
    ArticulationLimitExceeded {
        /// The articulated agent.
        agent: AgentId,
        /// The trailing segment's own index in the chain.
        hitch_index: u32,
        /// The hitch's absolute articulation angle in radians at this edge.
        angle_rad: f64,
        /// The compiled limit this hitch was checked against.
        limit_rad: f64,
        /// `true` when the angle just exceeded the limit, `false` when it
        /// just returned inside it.
        exceeding: bool,
    },
    /// A specific segment pair came into contact, or separated, naming the
    /// touching segment of any articulated-chain participant rather than the
    /// whole chain [`Event::Collision`] reports.
    ///
    /// Emitted instead of `Event::Collision` for a contacting pair where at
    /// least one side is an `ArticulatedWheeled` chain; a pair of two ordinary
    /// (non-chain) bodies still emits only `Event::Collision`, unchanged.
    /// `agent` is the lower [`AgentId`] of the pair and `other` the higher,
    /// matching `Event::Collision`'s pair spelling. See [`crate::safety`].
    ArticulatedSegmentContact {
        /// Lower agent id of the contacting pair.
        agent: AgentId,
        /// `agent`'s own touching chain-segment index (`0` = lead), or `None`
        /// when `agent` is not an articulated chain.
        agent_segment: Option<u32>,
        /// Higher agent id of the contacting pair.
        other: AgentId,
        /// `other`'s own touching chain-segment index, or `None` when `other`
        /// is not an articulated chain.
        other_segment: Option<u32>,
        /// Signed surface clearance in metres between the two named segments
        /// at the tick end. Static (tick-end) only: sub-tick swept precision
        /// for a chain segment is deferred to the swept-queries follow-up.
        clearance_m: f64,
        /// `true` when contact began, `false` when it ended.
        contacting: bool,
    },
}

impl Event {
    /// The agent this event is about.
    ///
    /// For a body-pair record this is the lower [`AgentId`] of the pair, and for
    /// a close pass the passing agent.
    pub const fn agent(&self) -> AgentId {
        match self {
            Self::Spawned { agent, .. }
            | Self::Despawned { agent, .. }
            | Self::Yielded { agent, .. }
            | Self::Collision { agent, .. }
            | Self::NearMiss { agent, .. }
            | Self::Violation { agent, .. }
            | Self::Entry { agent, .. }
            | Self::Exit { agent, .. }
            | Self::Queue { agent, .. }
            | Self::ControlTransition { agent, .. }
            | Self::Maneuver { agent, .. }
            | Self::FacilityTransition { agent, .. }
            | Self::OpposingTraversal { agent, .. }
            | Self::ClosePass { agent, .. }
            | Self::ArticulationLimitExceeded { agent, .. }
            | Self::ArticulatedSegmentContact { agent, .. } => *agent,
        }
    }

    /// Which record this event is, without its payload.
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::Spawned { .. } => EventKind::Spawned,
            Self::Despawned { .. } => EventKind::Despawned,
            Self::Yielded { .. } => EventKind::Yielded,
            Self::Collision { .. } => EventKind::Collision,
            Self::NearMiss { .. } => EventKind::NearMiss,
            Self::Violation { .. } => EventKind::Violation,
            Self::Entry { .. } => EventKind::Entry,
            Self::Exit { .. } => EventKind::Exit,
            Self::Queue { .. } => EventKind::Queue,
            Self::ControlTransition { .. } => EventKind::ControlTransition,
            Self::Maneuver { .. } => EventKind::Maneuver,
            Self::FacilityTransition { .. } => EventKind::FacilityTransition,
            Self::OpposingTraversal { .. } => EventKind::OpposingTraversal,
            Self::ClosePass { .. } => EventKind::ClosePass,
            Self::ArticulationLimitExceeded { .. } => EventKind::ArticulationLimitExceeded,
            Self::ArticulatedSegmentContact { .. } => EventKind::ArticulatedSegmentContact,
        }
    }

    /// The documented within-tick order key of this record.
    ///
    /// The slots are compared in order: the ascending [`AgentId`] the record is
    /// about, the ascending [`EventKind::order`], a tag that separates the
    /// variant's key space, then the variant's own stable key in up to five
    /// ascending components, the last of which is the record's edge flag. A key
    /// component that names a tactic, maneuver state, or maneuver edge is that
    /// enum's declaration order, which is the contract's kind, state-machine,
    /// and edge order, and an optional component is ordered by [`facility_key`]'s
    /// rule. A step sorts its buffer
    /// by this key, so a consumer can assert the documented order directly. See
    /// the module docs for why a residual tie can only be two identical records.
    pub const fn order_key(&self) -> (u32, u8, u8, u32, u32, u32, u32, u32) {
        let kind = self.kind().order();
        match self {
            Self::Spawned { agent, path, .. } => (agent.get(), kind, 0, path.get(), 0, 0, 0, 0),
            Self::Despawned { agent, path, .. } => (agent.get(), kind, 0, path.get(), 0, 0, 0, 0),
            Self::Yielded {
                agent,
                crossing,
                yielding,
            } => (
                agent.get(),
                kind,
                0,
                crossing.get(),
                *yielding as u32,
                0,
                0,
                0,
            ),
            Self::Collision {
                agent,
                other,
                contacting,
                ..
            } => (
                agent.get(),
                kind,
                0,
                other.get(),
                *contacting as u32,
                0,
                0,
                0,
            ),
            Self::NearMiss {
                agent,
                other,
                entering,
                ..
            } => (agent.get(), kind, 0, other.get(), *entering as u32, 0, 0, 0),
            Self::Violation {
                agent,
                kind: breach,
            } => (agent.get(), kind, 0, breach.order() as u32, 0, 0, 0, 0),
            Self::Entry { agent, region } => {
                (agent.get(), kind, region.tag(), region.get(), 0, 0, 0, 0)
            }
            Self::Exit { agent, region } => {
                (agent.get(), kind, region.tag(), region.get(), 0, 0, 0, 0)
            }
            Self::Queue { agent, joined } => (agent.get(), kind, 0, 0, *joined as u32, 0, 0, 0),
            Self::ControlTransition {
                agent,
                control,
                active,
            } => (
                agent.get(),
                kind,
                0,
                control.order() as u32,
                *active as u32,
                0,
                0,
                0,
            ),
            // The increment-2 keys of *Ordering*: the tactic, the target
            // facility, both states, and the edge; then the two facilities of a
            // handoff; then the facility, movement, and interval edge of an
            // opposing traversal; then the partner and facility of a close pass.
            Self::Maneuver {
                agent,
                kind: tactic,
                target_facility,
                from,
                to,
                edge,
                ..
            } => (
                agent.get(),
                kind,
                0,
                *tactic as u32,
                facility_key(*target_facility),
                *from as u32,
                *to as u32,
                *edge as u32,
            ),
            Self::FacilityTransition {
                agent,
                from_facility,
                to_facility,
                ..
            } => (
                agent.get(),
                kind,
                0,
                from_facility.get(),
                to_facility.get(),
                0,
                0,
                0,
            ),
            Self::OpposingTraversal {
                agent,
                facility,
                movement,
                entering,
                ..
            } => (
                agent.get(),
                kind,
                0,
                facility.get(),
                movement_key(*movement),
                *entering as u32,
                0,
                0,
            ),
            Self::ClosePass {
                agent,
                partner,
                facility,
                ..
            } => (agent.get(), kind, 0, partner.get(), facility.get(), 0, 0, 0),
            Self::ArticulationLimitExceeded {
                agent,
                hitch_index,
                exceeding,
                ..
            } => (
                agent.get(),
                kind,
                0,
                *hitch_index,
                *exceeding as u32,
                0,
                0,
                0,
            ),
            Self::ArticulatedSegmentContact {
                agent,
                other,
                contacting,
                ..
            } => (
                agent.get(),
                kind,
                0,
                other.get(),
                *contacting as u32,
                0,
                0,
                0,
            ),
        }
    }
}

/// The order-key component of an optional facility.
///
/// An absent component is `0` and a present one its dense index plus one, so an
/// absent component orders before every present one, exactly as `Option`'s own
/// order does. A dense index is an array position, so it cannot reach
/// `u32::MAX` and the two encodings stay distinct.
const fn facility_key(facility: Option<FacilityId>) -> u32 {
    match facility {
        Some(facility) => facility.get() + 1,
        None => 0,
    }
}

/// The order-key component of an optional movement, on [`facility_key`]'s rule.
const fn movement_key(movement: Option<MovementId>) -> u32 {
    match movement {
        Some(movement) => movement.get() + 1,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The event kinds of version 2, in their documented order.
    const VERSION_2_KINDS: [EventKind; 10] = [
        EventKind::Spawned,
        EventKind::Despawned,
        EventKind::Yielded,
        EventKind::Collision,
        EventKind::NearMiss,
        EventKind::Violation,
        EventKind::Entry,
        EventKind::Exit,
        EventKind::Queue,
        EventKind::ControlTransition,
    ];

    /// One agent, so a test compares variant keys alone.
    const AGENT: AgentId = AgentId::from_index(2);

    fn maneuver(target_facility: Option<FacilityId>, reason: ManeuverReasonCode) -> Event {
        Event::Maneuver {
            agent: AGENT,
            kind: TacticKind::Overtake,
            from: ManeuverState::Following,
            to: ManeuverState::Preparing,
            edge: ManeuverEdge::Attempted,
            partner: None,
            source_facility: FacilityId::from_index(0),
            target_facility,
            target_offset_m: 2.5,
            side: PassSide::Left,
            reason,
        }
    }

    fn opposing(movement: Option<MovementId>, entering: bool) -> Event {
        Event::OpposingTraversal {
            agent: AGENT,
            facility: FacilityId::from_index(1),
            movement,
            direction: MovementDirection::Reverse,
            nominal_direction: NominalDirection::Forward,
            perceived_rule: None,
            reason: WrongWayReason::NoncompliantChoice,
            violating: true,
            entering,
        }
    }

    fn hitch_limit(hitch_index: u32, exceeding: bool) -> Event {
        Event::ArticulationLimitExceeded {
            agent: AGENT,
            hitch_index,
            angle_rad: 1.0,
            limit_rad: 0.9,
            exceeding,
        }
    }

    fn close_pass(partner: usize, facility: usize) -> Event {
        Event::ClosePass {
            agent: AGENT,
            partner: AgentId::from_index(partner),
            facility: FacilityId::from_index(facility),
            side: PassSide::Left,
            min_clearance_m: 0.3,
            min_clearance_time_s: 1.5,
            relative_speed_mps: 1.25,
            bands: vec![ClosePassBand {
                band: ClearanceBandId::from_index(0),
                duration_s: 0.4,
            }],
            violating_bands: vec![ClearanceBandId::from_index(0)],
            crossed_boundary: false,
            entered_opposing: false,
        }
    }

    /// The version-2 kinds keep their order values and the four additive kinds
    /// are appended after them, so no existing stream reorders.
    #[test]
    fn the_additive_kinds_are_appended_after_control_transition() {
        for (position, kind) in VERSION_2_KINDS.iter().enumerate() {
            assert_eq!(
                kind.order() as usize,
                position,
                "{kind:?} kept its version-2 order"
            );
        }
        assert_eq!(EventKind::Maneuver.order() as usize, VERSION_2_KINDS.len());
        assert_eq!(
            EventKind::FacilityTransition.order() as usize,
            VERSION_2_KINDS.len() + 1
        );
        assert_eq!(
            EventKind::OpposingTraversal.order() as usize,
            VERSION_2_KINDS.len() + 2
        );
        assert_eq!(
            EventKind::ClosePass.order() as usize,
            VERSION_2_KINDS.len() + 3
        );
        assert_eq!(
            EventKind::ArticulationLimitExceeded.order() as usize,
            VERSION_2_KINDS.len() + 4,
            "the articulated-chain record is appended after close pass, still under version 3"
        );
        assert_eq!(EVENT_VERSION, 3, "the additive union's single bump");
    }

    /// The appended kinds order by the contract's own key, and the accessors
    /// report the agent and kind of every new variant.
    #[test]
    fn a_new_record_orders_by_its_contract_key() {
        let later_facility = maneuver(Some(FacilityId::from_index(3)), ManeuverReasonCode::Settled);
        let earlier_facility =
            maneuver(Some(FacilityId::from_index(1)), ManeuverReasonCode::Settled);
        assert!(
            earlier_facility.order_key() < later_facility.order_key(),
            "a maneuver orders by its target facility"
        );
        assert!(
            maneuver(None, ManeuverReasonCode::Settled).order_key() < earlier_facility.order_key(),
            "a maneuver with no target facility orders before every one that has one"
        );
        assert_eq!(later_facility.agent(), AGENT);
        assert_eq!(later_facility.kind(), EventKind::Maneuver);

        // The two interval edges of one traversal, and an absent movement before
        // a present one, both follow the contract's key.
        assert!(
            opposing(None, true).order_key()
                < opposing(Some(MovementId::from_index(0)), true).order_key()
        );
        assert!(
            opposing(Some(MovementId::from_index(4)), false).order_key()
                < opposing(Some(MovementId::from_index(4)), true).order_key(),
            "the interval edge orders as every other edge flag does: closed before open"
        );
        assert_eq!(opposing(None, true).kind(), EventKind::OpposingTraversal);

        let transition = |to_facility| Event::FacilityTransition {
            agent: AGENT,
            from_facility: FacilityId::from_index(0),
            to_facility: FacilityId::from_index(to_facility),
            from_direction: MovementDirection::Forward,
            to_direction: MovementDirection::Reverse,
            via: TransitionKind::Lateral,
            side: PassSide::Right,
            s_m: 30.0,
            d_m: -1.25,
            permitted: false,
        };
        assert!(
            transition(1).order_key() < transition(2).order_key(),
            "a handoff orders by its destination facility"
        );
        assert_eq!(transition(1).kind(), EventKind::FacilityTransition);

        // A close pass orders by its partner and then its facility, and the
        // accessors report the passing agent and its kind.
        assert!(
            close_pass(1, 0).order_key() < close_pass(2, 0).order_key(),
            "a close pass orders by its partner"
        );
        assert!(
            close_pass(1, 0).order_key() < close_pass(1, 1).order_key(),
            "a close pass orders by its facility within one partner"
        );
        assert_eq!(close_pass(1, 0).agent(), AGENT);
        assert_eq!(close_pass(1, 0).kind(), EventKind::ClosePass);

        // An articulation-limit record orders by its hitch and then its
        // exceeding edge (closed before open, as every other edge flag does).
        assert!(
            hitch_limit(1, true).order_key() < hitch_limit(2, true).order_key(),
            "an articulation-limit record orders by its hitch index"
        );
        assert!(
            hitch_limit(1, false).order_key() < hitch_limit(1, true).order_key(),
            "the falling edge orders before the rising edge for the same hitch"
        );
        assert_eq!(hitch_limit(1, true).agent(), AGENT);
        assert_eq!(
            hitch_limit(1, true).kind(),
            EventKind::ArticulationLimitExceeded
        );

        // Every new kind follows the version-2 kinds for the same agent.
        assert!(
            Event::ControlTransition {
                agent: AGENT,
                control: ControlTransitionKind::SignalStop,
                active: true,
            }
            .order_key()
                < earlier_facility.order_key()
        );
    }
}
