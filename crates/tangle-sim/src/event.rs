//! Typed state transitions observed during a step.
//!
//! # Record union
//!
//! [`Event`] is one closed union that both modes emit through, and
//! [`EVENT_VERSION`] describes the union as a whole: a consumer that keys on the
//! version may rely on the full variant set below, not on a subset. Two
//! families live in it:
//!
//! - lifecycle records that carry dense identifiers — [`Event::Spawned`],
//!   [`Event::Despawned`], and the kernel's own control transitions
//!   ([`Event::Yielded`], [`Event::ControlTransition`]);
//! - safety records produced by the tick's body scan
//!   ([`crate::safety`]) — [`Event::Collision`], [`Event::NearMiss`],
//!   [`Event::Violation`], [`Event::Entry`], [`Event::Exit`], and
//!   [`Event::Queue`].
//!
//! # Within-tick order
//!
//! Events are emitted in a documented, stable order that never depends on a
//! hash-map iteration or on the order emission points happen to run. Every
//! record carries [`Event::order_key`]; a step sorts its buffer by that key, so
//! a consumer sees ascending [`AgentId`] first, then ascending event kind
//! ([`EventKind::order`]), then the variant's own stable key (its partner agent,
//! crossing, region, path, or sub-kind), and finally the record's edge flag
//! (`yielding`, `contacting`, `entering`, `joined`, `active`, or a violation or
//! control kind). The sort is stable, so two records that agree on all of that
//! are the same variant with the same key, which the once-per-transition
//! lifecycle forbids; a residual tie can therefore only be two identical
//! records, and their relative order is the kernel's emission order.
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

use tangle_model::{ConflictRegionId, CrossingId, PathId};

use crate::agent::{AgentId, AgentMode};

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
pub const EVENT_VERSION: u32 = 2;

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

/// A typed transition emitted by a step.
///
/// Events carry dense identifiers and unit-suffixed fields so observers can
/// build traces without touching internal state. See [`EVENT_VERSION`] for the
/// schema version recorded in run provenance, and [`Event::order_key`] for the
/// documented within-tick order.
#[derive(Debug, Clone, Copy, PartialEq)]
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
}

impl Event {
    /// The agent this event is about.
    ///
    /// For a body-pair record this is the lower [`AgentId`] of the pair.
    pub const fn agent(self) -> AgentId {
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
            | Self::ControlTransition { agent, .. } => agent,
        }
    }

    /// Which record this event is, without its payload.
    pub const fn kind(self) -> EventKind {
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
        }
    }

    /// The documented within-tick order key of this record.
    ///
    /// The fields are, in order: the ascending [`AgentId`] the record is about,
    /// the ascending [`EventKind::order`], a tag that separates the variant's
    /// key space, the variant's own stable key (partner agent, crossing, region,
    /// path, or sub-kind), and the record's edge flag. A step sorts its buffer
    /// by this key, so a consumer can assert the documented order directly. See
    /// the module docs for why a residual tie can only be two identical records.
    pub const fn order_key(self) -> (u32, u8, u8, u32, u8) {
        let kind = self.kind().order();
        match self {
            Self::Spawned { agent, path, .. } => (agent.get(), kind, 0, path.get(), 0),
            Self::Despawned { agent, path, .. } => (agent.get(), kind, 0, path.get(), 0),
            Self::Yielded {
                agent,
                crossing,
                yielding,
            } => (agent.get(), kind, 0, crossing.get(), yielding as u8),
            Self::Collision {
                agent,
                other,
                contacting,
                ..
            } => (agent.get(), kind, 0, other.get(), contacting as u8),
            Self::NearMiss {
                agent,
                other,
                entering,
                ..
            } => (agent.get(), kind, 0, other.get(), entering as u8),
            Self::Violation {
                agent,
                kind: breach,
            } => (agent.get(), kind, 0, breach.order() as u32, 0),
            Self::Entry { agent, region } => (agent.get(), kind, region.tag(), region.get(), 0),
            Self::Exit { agent, region } => (agent.get(), kind, region.tag(), region.get(), 0),
            Self::Queue { agent, joined } => (agent.get(), kind, 0, 0, joined as u8),
            Self::ControlTransition {
                agent,
                control,
                active,
            } => (agent.get(), kind, 0, control.order() as u32, active as u8),
        }
    }
}
