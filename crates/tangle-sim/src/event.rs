//! Typed state transitions observed during a step.

use tangle_model::PathId;

use crate::agent::AgentId;

/// Version of the typed event schema emitted by this build.
///
/// Freeze this in the run manifest so a later trace invalidation is deliberate.
/// Increment it when an existing event's meaning, fields, or payload semantics
/// change; adding a new variant is also a consumer-visible change, so bump it
/// before any golden trace is regenerated for that reason.
pub const EVENT_VERSION: u32 = 1;

/// Why an agent left the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DespawnReason {
    /// The agent reached the far end of its guide path.
    ExitedPath,
}

/// A typed transition emitted by a step.
///
/// Events carry dense identifiers and unit-suffixed fields so observers can
/// build traces without touching internal state. See [`EVENT_VERSION`] for the
/// schema version recorded in run provenance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// An agent entered the world at a path position.
    Spawned {
        /// The new agent.
        agent: AgentId,
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
}

impl Event {
    /// The agent this event is about.
    pub const fn agent(self) -> AgentId {
        match self {
            Self::Spawned { agent, .. } | Self::Despawned { agent, .. } => agent,
        }
    }
}
