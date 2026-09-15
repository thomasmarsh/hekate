//! Typed safety-event observation over one tick's body scan.
//!
//! # Model card
//!
//! `PHASE_1_PLAN.md` Increment 4 asks for typed collision, near-miss,
//! violation, entry/exit, queue, and control-transition events over the shared
//! vehicle/pedestrian world. This module owns the observation pass that turns
//! the tick's integrated state into those records. It is the consumer of slice
//! A's exact queries, slice B's swept bounds and time-of-impact cast, and the
//! decision records the kernel already stores, so both modes emit through one
//! stream and no observer needs private state.
//!
//! ## Predicates
//!
//! Each record is edge-triggered on a per-tick predicate, never emitted
//! per-step:
//!
//! - **contact** ([`Event::Collision`]): a candidate pair whose signed surface
//!   clearance reached or passed zero at some time during the tick. The
//!   post-step clearance alone would miss a fast pair that touches between two
//!   tick endpoints, so a pair whose clearance could have closed inside the tick
//!   is confirmed with [`time_of_impact`].
//! - **near miss** ([`Event::NearMiss`]): a pair that is not in contact and
//!   whose signed clearance reached the band `(0,
//!   NEAR_MISS_THRESHOLD_M]` at some time during the tick, confirmed the same
//!   way with [`band_entry`]. Contact and near miss are two nested per-tick
//!   predicates rather than one predicate with a special case: the contact band
//!   sits inside the near-miss band, so while a pair is in contact its near-miss
//!   state is `false`. A pair therefore ends its near-miss record where contact
//!   begins and begins a new one where contact ends, which keeps both families
//!   strictly alternating.
//! - **violation** ([`Event::Violation`]): a crossing action the recorded
//!   decision says was taken against a forbidding signal.
//! - **entry and exit** ([`Event::Entry`], [`Event::Exit`]): a body's tick-swept
//!   extent reaching, then clearing, a crossing or conflict region.
//! - **queue** ([`Event::Queue`]): the agent's longitudinal speed at the tick
//!   end at or below [`QUEUE_STOP_SPEED_MPS`].
//! - **control transition** ([`Event::ControlTransition`]): the recorded
//!   decision changing between "waiting" and "not waiting".
//!
//! ## No missed and no duplicate transitions
//!
//! - Duplicates: every predicate is a pure function of the tick's sampled
//!   state, each record's set membership is the previous sample, and a record is
//!   pushed exactly on a membership change, so a transition is reported once.
//!   A pair or slot in an open state is closed by exactly one end record: the
//!   same pass that found the change, or — when a pair's open state outlives its
//!   candidate window — the explicit stale pass below. Both pair families and
//!   every per-agent state therefore alternate begin, end, begin, and never
//!   repeat a begin without an intervening end.
//! - No misses: the contact predicate is swept over the whole tick, so a
//!   sub-tick touch is not missed; region occupancy uses the body's tick-swept
//!   bounding circle, which contains every pose the body takes during the tick,
//!   so a region the body reaches inside a tick is not missed. The queue and
//!   control predicates are tick-end *state* predicates: their transitions are
//!   exactly the transitions of the sampled state, and a state that comes and
//!   goes inside one tick is not a transition of that state.
//! - A despawn clears the agent's open pair and region states without a further
//!   safety record, because [`Event::Despawned`] closes the agent's stream;
//!   its open queue and control states are closed with their end records first,
//!   so every formation still has its departure.
//!
//! ## Determinism
//!
//! Candidate pairs come from a [`SweptBroadPhase`], which is a pure function of
//! the indexed bodies and ascending by [`AgentId`]. The open-state sets are
//! ordered, and nothing here iterates a hash map or depends on insertion order;
//! the caller sorts the tick's records by [`Event::order_key`]. The only
//! constants are the two declared bands below, both in metres and metres per
//! second, over `f64`/`glam::DVec2` state.

use std::collections::BTreeSet;

use glam::DVec2;
use hekate_model::CompiledScenario;

use crate::agent::{AgentId, AgentMode, AgentStore};
use crate::compliance::SignalAction;
use crate::event::{ControlTransitionKind, Event, RegionKey, ViolationKind};
use crate::index::{SweptBroadPhase, circle_overlaps_ring};
use crate::pedestrian_compliance::PedestrianSignalAction;
use crate::query::{self, BodyShape};
use crate::swept::{SweptBody, band_entry, time_of_impact};

/// Signed surface separation in metres at or below which a disjoint pair is a
/// near miss.
///
/// The band is a reporting threshold, not a physical property: it is well above
/// the `CONTACT_EPSILON_M` contact band so "close but not touching" is
/// observable, and well below the IDM leader standstill gap
/// ([`crate::control::IDM_STANDSTILL_GAP_M`]) so an ordinary car-following
/// queue is not reported as a stream of near misses.
pub const NEAR_MISS_THRESHOLD_M: f64 = 1.0;

/// Longitudinal speed in metres per second at or below which an agent counts as
/// stopped and waiting.
///
/// One millimetre per second: far below any commanded speed the controllers can
/// produce from rest in one step, so the state is a genuine standstill rather
/// than a slow approach.
pub const QUEUE_STOP_SPEED_MPS: f64 = 1e-3;

/// Per-tick safety observation state.
///
/// One monitor lives in the simulation. Its columns are indexed by agent slot
/// and grown as the store grows; a slot that dies keeps its identity, so the
/// monitor never reindexes live state.
#[derive(Debug, Clone, Default)]
pub(crate) struct SafetyMonitor {
    /// Body shape of every live agent at the tick start, by slot.
    start_shapes: Vec<Option<BodyShape>>,
    /// Tick-swept body of every live agent, ascending by [`AgentId`].
    indexed: Vec<(AgentId, SweptBody)>,
    /// The same bodies grown by half [`NEAR_MISS_THRESHOLD_M`], which is what
    /// the candidate grid indexes; see [`Self::index_bodies`].
    inflated: Vec<(AgentId, SweptBody)>,
    /// Swept broad phase over this tick's grown live bodies.
    grid: SweptBroadPhase,
    /// Reused candidate-pair buffer, ascending `(AgentId, AgentId)`.
    pairs: Vec<(AgentId, AgentId)>,
    /// Reused buffer of open states whose candidate window has passed.
    stale: Vec<(AgentId, AgentId)>,
    /// Pairs currently in contact, ascending.
    contact: BTreeSet<(AgentId, AgentId)>,
    /// Pairs currently inside the near-miss band, ascending.
    near: BTreeSet<(AgentId, AgentId)>,
    /// Regions each live body currently overlaps, ascending by slot and key.
    occupied: BTreeSet<(AgentId, RegionKey)>,
    /// Whether each slot is currently stopped and waiting.
    queueing: Vec<bool>,
    /// Whether each slot's recorded decision is currently to wait.
    waiting: Vec<bool>,
}

impl SafetyMonitor {
    /// Record every live body's tick-start shape.
    ///
    /// Call once before the state-affecting loop, so the swept bodies the
    /// observation builds carry the whole tick's displacement.
    pub(crate) fn begin_tick(&mut self, agents: &AgentStore) {
        self.start_shapes.clear();
        self.start_shapes.resize(agents.len(), None);
        for index in 0..agents.len() {
            if agents.alive[index] {
                self.start_shapes[index] = Some(agent_broad_phase_shape(agents, index));
            }
        }
    }

    /// Observe the post-integration state and push this tick's safety records.
    ///
    /// Safe to call once per tick after every live agent has stepped and before
    /// new demand is admitted, so the records describe exactly the state the
    /// tick integrated.
    pub(crate) fn observe(
        &mut self,
        agents: &AgentStore,
        scenario: &CompiledScenario,
        events: &mut Vec<Event>,
    ) {
        self.queueing.resize(agents.len(), false);
        self.waiting.resize(agents.len(), false);
        self.forget_dead(agents);
        self.index_bodies(agents);
        self.scan_pairs(agents, events);
        self.scan_regions(agents, scenario, events);
        self.scan_states(agents, events);
    }

    /// Drop the open state of every dead slot without a further safety record.
    ///
    /// [`Event::Despawned`] closes the agent's stream, so a pair or region that
    /// a despawn ends needs no separate end record. The queue and control states
    /// are different: they are per-agent states with a departure record, and the
    /// state pass closes them below.
    fn forget_dead(&mut self, agents: &AgentStore) {
        let alive = |id: AgentId| agents.alive.get(id.index()).copied().unwrap_or(false);
        self.contact
            .retain(|(first, second)| alive(*first) && alive(*second));
        self.near
            .retain(|(first, second)| alive(*first) && alive(*second));
        self.occupied.retain(|(agent, _)| alive(*agent));
    }

    /// Rebuild the tick-swept bodies and the swept broad phase.
    ///
    /// The grid indexes each body grown by half the near-miss threshold, so a
    /// candidate pair is every pair that could have been inside the contact band
    /// or the near-miss band at some time in the tick: growing both bodies by
    /// half the margin shrinks their surface clearance by the whole margin
    /// ([`BodyShape::inflated`]), so the grown swept bounds of an in-band pair
    /// overlap. The band tests in [`Self::scan_pair`] then decide each record
    /// from the true, un-grown swept bodies. [`SweptBroadPhase`]'s own candidate
    /// set is every pair whose un-grown swept bounds overlap, which is the
    /// contact superset only; the grown grid is what makes the near-miss band a
    /// candidate as well.
    fn index_bodies(&mut self, agents: &AgentStore) {
        self.indexed.clear();
        self.inflated.clear();
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            let end = agent_broad_phase_shape(agents, index);
            let start = self.start_shapes[index].unwrap_or(end);
            let body = SweptBody {
                shape: start,
                displacement_m: end.centre() - start.centre(),
            };
            self.indexed.push((AgentId::from_index(index), body));
            self.inflated.push((
                AgentId::from_index(index),
                SweptBody {
                    shape: start.inflated(NEAR_MISS_THRESHOLD_M * 0.5),
                    displacement_m: body.displacement_m,
                },
            ));
        }
        self.grid.rebuild(&self.inflated);
    }

    /// The tick-swept body of one live agent.
    fn body_of(&self, agent: AgentId) -> SweptBody {
        let position = self
            .indexed
            .binary_search_by_key(&agent, |(id, _)| *id)
            .expect("safety state refers only to indexed live agents");
        self.indexed[position].1
    }

    /// The tick-swept bounding circle of one live slot: the centre of its swept
    /// segment and the radius that contains every pose it took this tick.
    ///
    /// A body's shape lies inside a circle of its circumradius about its centre,
    /// and its centre stays within half the displacement of the swept segment's
    /// midpoint, so this circle contains the whole swept volume. Region
    /// occupancy therefore never misses a region the body reached during the
    /// tick; it can report entry up to `circumradius + |displacement| / 2`
    /// early, which is the conservative bound.
    fn swept_circle(&self, agent: AgentId) -> (DVec2, f64) {
        let body = self.body_of(agent);
        (
            body.shape.centre() + body.displacement_m * 0.5,
            body.shape.circumradius_m() + body.displacement_m.length() * 0.5,
        )
    }

    /// Emit the contact and near-miss edges of every candidate pair.
    fn scan_pairs(&mut self, agents: &AgentStore, events: &mut Vec<Event>) {
        self.pairs.clear();
        self.grid.candidate_pairs(&mut self.pairs);
        for position in 0..self.pairs.len() {
            let (first, second) = self.pairs[position];
            self.scan_pair(agents, first, second, events);
        }
        self.close_stale_pairs(agents, events);
    }

    /// Emit the edges of one observed pair.
    ///
    /// A pair where neither side is an `ArticulatedWheeled` chain keeps the
    /// exact existing swept clearance and time-of-impact/band-entry cast,
    /// unchanged. A pair where either side is a chain instead compares every
    /// segment of each side against every segment of the other
    /// ([`agent_segments`], [`min_segment_clearance`]), using only exact,
    /// tick-end (static) geometry: this is the deliberate, temporary
    /// precision gap documented on [`Event::ArticulatedSegmentContact`] —
    /// sub-tick swept precision for a chain segment is the next sibling
    /// node's job.
    fn scan_pair(
        &mut self,
        agents: &AgentStore,
        first: AgentId,
        second: AgentId,
        events: &mut Vec<Event>,
    ) {
        let key = (first, second);
        let first_is_chain = agents.articulated[first.index()].is_some();
        let second_is_chain = agents.articulated[second.index()].is_some();

        let (contact, near, clearance_m, first_segment, second_segment) =
            if first_is_chain || second_is_chain {
                let first_segments = agent_segments(agents, first.index());
                let second_segments = agent_segments(agents, second.index());
                let (clearance_m, first_index, second_index) =
                    min_segment_clearance(&first_segments, &second_segments);
                let contact = clearance_m <= 0.0;
                let near = !contact && clearance_m <= NEAR_MISS_THRESHOLD_M;
                (
                    contact,
                    near,
                    clearance_m,
                    first_is_chain.then_some(first_index),
                    second_is_chain.then_some(second_index),
                )
            } else {
                let first_body = self.body_of(first);
                let second_body = self.body_of(second);
                let clearance_m =
                    query::body_clearance_m(&first_body.end_shape(), &second_body.end_shape());
                // The signed clearance of two convex bodies under relative translation
                // is 1-Lipschitz, so a pair clear of a band by more than the tick's
                // relative displacement was never inside it this tick. That certificate
                // skips the cast for the pairs that cannot be close.
                let relative_m = (second_body.displacement_m - first_body.displacement_m).length();
                let near_hit = clearance_m <= NEAR_MISS_THRESHOLD_M
                    || (clearance_m <= NEAR_MISS_THRESHOLD_M + relative_m
                        && band_entry(&first_body, &second_body, NEAR_MISS_THRESHOLD_M).is_some());
                // The contact band sits inside the near-miss band, so a pair the
                // certificate excluded from the wider band cannot have touched either.
                // Contact takes precedence in the reporting: the near-miss state is
                // `false` for exactly the ticks the pair is in contact, which is why the
                // two records are two nested per-tick predicates rather than a special
                // case in the emission below.
                let contact = near_hit
                    && (clearance_m <= 0.0 || time_of_impact(&first_body, &second_body).is_some());
                let near = near_hit && !contact;
                (contact, near, clearance_m, None, None)
            };

        let was_contact = self.contact.contains(&key);
        if contact != was_contact {
            if contact {
                self.contact.insert(key);
            } else {
                self.contact.remove(&key);
            }
            if first_is_chain || second_is_chain {
                events.push(Event::ArticulatedSegmentContact {
                    agent: first,
                    agent_segment: first_segment,
                    other: second,
                    other_segment: second_segment,
                    clearance_m,
                    contacting: contact,
                });
            } else {
                events.push(Event::Collision {
                    agent: first,
                    other: second,
                    clearance_m,
                    contacting: contact,
                });
            }
        }
        let was_near = self.near.contains(&key);
        if near != was_near {
            if near {
                self.near.insert(key);
            } else {
                self.near.remove(&key);
            }
            events.push(Event::NearMiss {
                agent: first,
                other: second,
                clearance_m,
                entering: near,
            });
        }
    }

    /// Close the open state of pairs no longer in this tick's candidate set.
    ///
    /// A pair inside a reporting band is always a broad-phase candidate, so this
    /// is a completeness guard rather than the common path: it guarantees that
    /// no open state can outlive the candidate window without its end record.
    fn close_stale_pairs(&mut self, agents: &AgentStore, events: &mut Vec<Event>) {
        let pairs = &self.pairs;
        self.stale.clear();
        self.stale.extend(
            self.contact
                .iter()
                .filter(|key| pairs.binary_search(key).is_err())
                .copied(),
        );
        self.stale.extend(
            self.near
                .iter()
                .filter(|key| pairs.binary_search(key).is_err())
                .copied(),
        );
        for key in &self.stale {
            let (first, second) = *key;
            let first_is_chain = agents.articulated[first.index()].is_some();
            let second_is_chain = agents.articulated[second.index()].is_some();
            let (clearance_m, first_segment, second_segment) = if first_is_chain || second_is_chain
            {
                let first_segments = agent_segments(agents, first.index());
                let second_segments = agent_segments(agents, second.index());
                let (clearance_m, first_index, second_index) =
                    min_segment_clearance(&first_segments, &second_segments);
                (
                    clearance_m,
                    first_is_chain.then_some(first_index),
                    second_is_chain.then_some(second_index),
                )
            } else {
                let clearance_m = query::body_clearance_m(
                    &self.body_of(first).end_shape(),
                    &self.body_of(second).end_shape(),
                );
                (clearance_m, None, None)
            };
            if self.contact.remove(key) {
                if first_is_chain || second_is_chain {
                    events.push(Event::ArticulatedSegmentContact {
                        agent: first,
                        agent_segment: first_segment,
                        other: second,
                        other_segment: second_segment,
                        clearance_m,
                        contacting: false,
                    });
                } else {
                    events.push(Event::Collision {
                        agent: first,
                        other: second,
                        clearance_m,
                        contacting: false,
                    });
                }
            }
            if self.near.remove(key) {
                events.push(Event::NearMiss {
                    agent: first,
                    other: second,
                    clearance_m,
                    entering: false,
                });
            }
        }
    }

    /// Emit the entry and exit edges of every crossing and conflict region.
    fn scan_regions(
        &mut self,
        agents: &AgentStore,
        scenario: &CompiledScenario,
        events: &mut Vec<Event>,
    ) {
        for crossing in scenario.crossings() {
            let Some(ring) = scenario
                .region(crossing.region())
                .map(|region| region.polygon().ring())
            else {
                continue;
            };
            let signal_controlled = crossing.pedestrian_signal().is_some();
            self.scan_region(
                agents,
                ring,
                RegionKey::Crossing(crossing.id()),
                signal_controlled,
                events,
            );
        }
        for region in scenario.conflict_regions() {
            self.scan_region(
                agents,
                region.polygon().ring(),
                RegionKey::ConflictRegion(region.id()),
                false,
                events,
            );
        }
    }

    /// Emit the entry and exit edges of one region.
    ///
    /// A crossing region additionally records a pedestrian's crossing against a
    /// forbidding signal on its entry edge, from the decision the kernel
    /// recorded for this tick: the recorded decision still describes the
    /// approach (its `crossing_gap_m` is positive) exactly on the tick the body
    /// first reaches the region.
    fn scan_region(
        &mut self,
        agents: &AgentStore,
        ring: &[DVec2],
        region: RegionKey,
        signal_controlled: bool,
        events: &mut Vec<Event>,
    ) {
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            let (centre, radius_m) = self.swept_circle(AgentId::from_index(index));
            let inside = circle_overlaps_ring(ring, centre, radius_m);
            let agent = AgentId::from_index(index);
            let key = (agent, region);
            let was = self.occupied.contains(&key);
            if inside == was {
                continue;
            }
            if inside {
                self.occupied.insert(key);
                events.push(Event::Entry { agent, region });
                if signal_controlled
                    && let Some(kind) = pedestrian_crossing_violation(agents, index)
                {
                    events.push(Event::Violation { agent, kind });
                }
            } else {
                self.occupied.remove(&key);
                events.push(Event::Exit { agent, region });
            }
        }
    }

    /// Emit the queue and control-transition edges of every slot.
    fn scan_states(&mut self, agents: &AgentStore, events: &mut Vec<Event>) {
        for index in 0..agents.len() {
            let agent = AgentId::from_index(index);
            if !agents.alive[index] {
                if self.queueing[index] {
                    self.queueing[index] = false;
                    events.push(Event::Queue {
                        agent,
                        joined: false,
                    });
                }
                if self.waiting[index] {
                    self.waiting[index] = false;
                    events.push(Event::ControlTransition {
                        agent,
                        control: control_kind(agents.mode[index]),
                        active: false,
                    });
                }
                continue;
            }
            let stopped = agents.speed_mps[index] <= QUEUE_STOP_SPEED_MPS;
            if stopped != self.queueing[index] {
                self.queueing[index] = stopped;
                events.push(Event::Queue {
                    agent,
                    joined: stopped,
                });
            }
            let waiting = match agents.mode[index] {
                AgentMode::Vehicle => agents.decision[index]
                    .is_some_and(|decision| decision.action == SignalAction::Stop),
                AgentMode::Pedestrian => agents.pedestrian_decision[index]
                    .is_some_and(|decision| decision.action == PedestrianSignalAction::Wait),
            };
            if waiting != self.waiting[index] {
                self.waiting[index] = waiting;
                events.push(Event::ControlTransition {
                    agent,
                    control: control_kind(agents.mode[index]),
                    active: waiting,
                });
            }
        }
    }
}

/// Every exact body shape of one agent's own segments, in chain order,
/// segment `0` first. A non-chain agent has exactly one: its own body.
fn agent_segments(agents: &AgentStore, index: usize) -> Vec<(u32, BodyShape)> {
    let lead = query::agent_body(agents, index);
    let Some(state) = &agents.articulated[index] else {
        return vec![(0, lead)];
    };
    let mut shapes = vec![(0, lead)];
    for (offset, (geometry, pose)) in state.segments()[1..]
        .iter()
        .zip(state.trailers())
        .enumerate()
    {
        shapes.push((
            (offset + 1) as u32,
            BodyShape::Box {
                centre: pose.position,
                heading_rad: pose.heading_rad,
                length_m: geometry.length_m,
                width_m: geometry.width_m,
            },
        ));
    }
    shapes
}

/// The broad-phase indexing proxy for one agent: exactly its own body for a
/// non-chain agent, so every existing box/circle/capsule scenario is
/// byte-for-byte unaffected, or one circle enclosing every segment's box for
/// an `ArticulatedWheeled` chain, so the swept broad phase can never miss a
/// contact any segment of the chain could make.
fn agent_broad_phase_shape(agents: &AgentStore, index: usize) -> BodyShape {
    if agents.articulated[index].is_none() {
        return query::agent_body(agents, index);
    }
    let segments = agent_segments(agents, index);
    let mut bounds = segments[0].1.bounds();
    for (_, shape) in &segments[1..] {
        let next = shape.bounds();
        bounds = query::Aabb::new(bounds.min.min(next.min), bounds.max.max(next.max));
    }
    BodyShape::Circle {
        centre: bounds.centre(),
        radius_m: bounds.half_extent().length(),
    }
}

/// The minimum exact clearance between any segment of `first` and any
/// segment of `second`, and the two segment indices that achieve it.
fn min_segment_clearance(
    first: &[(u32, BodyShape)],
    second: &[(u32, BodyShape)],
) -> (f64, u32, u32) {
    let mut best = (f64::INFINITY, 0u32, 0u32);
    for (first_index, first_shape) in first {
        for (second_index, second_shape) in second {
            let clearance_m = query::body_clearance_m(first_shape, second_shape);
            if clearance_m < best.0 {
                best = (clearance_m, *first_index, *second_index);
            }
        }
    }
    best
}

/// Which controller state a mode's recorded decision reports.
const fn control_kind(mode: AgentMode) -> ControlTransitionKind {
    match mode {
        AgentMode::Vehicle => ControlTransitionKind::SignalStop,
        AgentMode::Pedestrian => ControlTransitionKind::CrossingWait,
    }
}

/// Whether a pedestrian's recorded decision was to cross against a forbidding
/// signal while it was still upstream of the crossing.
///
/// [`PedestrianComplianceDecision`](crate::PedestrianComplianceDecision) carries
/// the signal and the decision point, so this reads no live signal state: a
/// `Cross` action on a `DontWalk` signal with a positive crossing gap is the
/// recorded choice to cross against the signal. Once the pedestrian is past the
/// decision point the recorded gap is zero, so the check holds exactly for the
/// tick the body first reaches the crossing and can never repeat.
fn pedestrian_crossing_violation(agents: &AgentStore, index: usize) -> Option<ViolationKind> {
    if agents.mode[index] != AgentMode::Pedestrian {
        return None;
    }
    let decision = agents.pedestrian_decision[index]?;
    if decision.action != PedestrianSignalAction::Cross
        || decision.signal != crate::signal::PedestrianSignalColor::DontWalk
        || decision.crossing_gap_m <= 0.0
    {
        return None;
    }
    Some(ViolationKind::CrossedAgainstSignal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentInit;
    use crate::articulated::{ArticulatedSegmentGeometry, ArticulatedState};
    use crate::compliance::{ComplianceDecision, ComplianceReason};
    use crate::pedestrian_compliance::{PedestrianComplianceDecision, PedestrianComplianceReason};
    use crate::query::CONTACT_EPSILON_M;
    use crate::signal::PedestrianSignalColor;
    use hekate_model::{CompiledScenario, PathId, parse_scenario_source};

    fn circle(centre: DVec2, radius_m: f64) -> BodyShape {
        BodyShape::Circle { centre, radius_m }
    }

    /// Assert two event lists match, comparing the metre fields within a
    /// nanometre so a hand-computed expectation need not repeat the exact
    /// floating-point arithmetic of the comparison.
    fn assert_same_events(actual: &[Event], expected: &[Event]) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "event count: {actual:?} vs {expected:?}"
        );
        for (actual, expected) in actual.iter().zip(expected.iter()) {
            match (actual, expected) {
                (
                    Event::Collision {
                        clearance_m: actual_m,
                        contacting: actual_flag,
                        ..
                    },
                    Event::Collision {
                        clearance_m: expected_m,
                        contacting: expected_flag,
                        ..
                    },
                )
                | (
                    Event::NearMiss {
                        clearance_m: actual_m,
                        entering: actual_flag,
                        ..
                    },
                    Event::NearMiss {
                        clearance_m: expected_m,
                        entering: expected_flag,
                        ..
                    },
                ) => {
                    assert_eq!(actual_flag, expected_flag, "{actual:?} vs {expected:?}");
                    assert!(
                        (actual_m - expected_m).abs() < 1e-9,
                        "clearance: {actual:?} vs {expected:?}"
                    );
                }
                _ => assert_eq!(actual, expected),
            }
        }
    }

    /// A scenario with no region at all, so region scans stay empty.
    fn no_region_scenario() -> CompiledScenario {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'safety', coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ], \
             population: { vehicle_count: 0, vehicle_speed_mps: 1.0, vehicle_spacing_m: 5.0, \
             vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    /// Append one pedestrian body of radius `radius_m` at `position`.
    fn push_pedestrian(
        store: &mut AgentStore,
        position: DVec2,
        speed_mps: f64,
        radius_m: f64,
    ) -> AgentId {
        store.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path: PathId::from_index(0),
            distance_m: 0.0,
            speed_mps,
            position,
            heading_rad: 0.0,
            body_length_m: radius_m * 2.0,
            body_width_m: radius_m * 2.0,
            direction: 1.0,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
        })
    }

    /// Append one vehicle box body of the given size at `position`.
    fn push_vehicle(store: &mut AgentStore, position: DVec2, speed_mps: f64) -> AgentId {
        store.push(AgentInit {
            mode: AgentMode::Vehicle,
            path: PathId::from_index(0),
            distance_m: 0.0,
            speed_mps,
            position,
            heading_rad: 0.0,
            body_length_m: 4.0,
            body_width_m: 2.0,
            direction: 1.0,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
        })
    }

    fn stop_decision() -> Option<ComplianceDecision> {
        Some(ComplianceDecision {
            action: SignalAction::Stop,
            reason: ComplianceReason::CompliantStop,
            color: hekate_model::SignalColor::Red,
            stop_line_gap_m: 5.0,
            required_decel_mps2: 1.0,
        })
    }

    fn wait_decision(gap_m: f64) -> Option<PedestrianComplianceDecision> {
        Some(PedestrianComplianceDecision {
            action: PedestrianSignalAction::Wait,
            reason: PedestrianComplianceReason::CompliantWait,
            signal: PedestrianSignalColor::DontWalk,
            crossing_gap_m: gap_m,
            required_decel_mps2: 0.5,
        })
    }

    fn cross_against_signal_decision() -> Option<PedestrianComplianceDecision> {
        Some(PedestrianComplianceDecision {
            action: PedestrianSignalAction::Cross,
            reason: PedestrianComplianceReason::NonCompliantCross,
            signal: PedestrianSignalColor::DontWalk,
            crossing_gap_m: 3.0,
            required_decel_mps2: 0.5,
        })
    }

    #[test]
    fn the_swept_circle_contains_the_whole_swept_body() {
        let body = SweptBody {
            shape: circle(DVec2::new(1.0, 2.0), 0.5),
            displacement_m: DVec2::new(3.0, -4.0),
        };
        let monitor = SafetyMonitor {
            indexed: vec![(AgentId::from_index(0), body)],
            ..SafetyMonitor::default()
        };
        let (centre, radius_m) = monitor.swept_circle(AgentId::from_index(0));
        assert_eq!(centre, DVec2::new(2.5, 0.0));
        assert!((radius_m - (0.5 + 2.5)).abs() < 1e-12);
        for fraction in [0.0, 0.25, 0.5, 1.0] {
            let pose = body.shape_at(fraction).centre();
            assert!(
                (pose - centre).length() + body.shape.circumradius_m() <= radius_m + 1e-12,
                "the swept circle must contain the pose at {fraction}"
            );
        }
    }

    #[test]
    fn the_contact_band_is_the_narrowest_band_a_cast_reports() {
        let still = SweptBody {
            shape: circle(DVec2::ZERO, 1.0),
            displacement_m: DVec2::ZERO,
        };
        let passing = SweptBody {
            shape: circle(DVec2::new(-5.0, 2.05), 1.0),
            displacement_m: DVec2::new(10.0, 0.0),
        };
        // A 5 cm pass-by is clear of the contact band but inside the near-miss
        // threshold, and the wider band is what reports it.
        assert_eq!(time_of_impact(&still, &passing), None);
        let hit = band_entry(&still, &passing, NEAR_MISS_THRESHOLD_M).expect("inside the band");
        assert!(hit.clearance_m <= NEAR_MISS_THRESHOLD_M);
        assert!(hit.clearance_m > CONTACT_EPSILON_M);
        // A pair already touching at the tick start reports time zero.
        let touching = SweptBody {
            shape: circle(DVec2::new(2.0, 0.0), 1.0),
            displacement_m: DVec2::new(1.0, 0.0),
        };
        assert_eq!(
            band_entry(&still, &touching, 0.0).expect("contact").time,
            0.0
        );
        assert_eq!(
            time_of_impact(&still, &touching).expect("contact").time,
            0.0
        );
    }

    /// Drive one tick of the real observation pass: capture the tick-start
    /// bodies, move the store, then observe.
    fn tick(
        monitor: &mut SafetyMonitor,
        store: &mut AgentStore,
        scenario: &CompiledScenario,
        motion: impl FnOnce(&mut AgentStore),
    ) -> Vec<Event> {
        monitor.begin_tick(store);
        motion(store);
        let mut events = Vec::new();
        monitor.observe(store, scenario, &mut events);
        events
    }

    /// The pair lifecycle, one record per transition: the near-miss band on
    /// approach, the contact edge that ends the band, the separation edge that
    /// begins it again, and the band's own end as the pair parts.
    #[test]
    fn a_pair_reports_one_edge_per_transition() {
        let scenario = no_region_scenario();
        let mut store = AgentStore::default();
        let first = push_pedestrian(&mut store, DVec2::ZERO, 0.0, 0.25);
        let second = push_pedestrian(&mut store, DVec2::new(-3.0, 0.0), 8.0, 0.25);
        let mut monitor = SafetyMonitor::default();
        // One 0.4 m step per tick, closing on the still body at the origin. The
        // assertions below are about the pair records, so the still body's queue
        // state is filtered out.
        let drive = |store: &mut AgentStore| store.position[second.index()].x += 0.4;
        let pair_events = |events: Vec<Event>| -> Vec<Event> {
            events
                .into_iter()
                .filter(|event| matches!(event, Event::Collision { .. } | Event::NearMiss { .. }))
                .collect()
        };
        let clearance = |store: &AgentStore| store.position[second.index()].x.abs() - 0.5;

        // Ticks 1-3: 2.1 m, 1.7 m, and 1.3 m of clearance, all outside the band.
        for _ in 0..3 {
            assert!(pair_events(tick(&mut monitor, &mut store, &scenario, drive)).is_empty());
        }
        // Tick 4: 0.9 m, inside the 1.0 m band: the band begins, once.
        assert_same_events(
            &pair_events(tick(&mut monitor, &mut store, &scenario, drive)),
            &[Event::NearMiss {
                agent: first,
                other: second,
                clearance_m: 0.9,
                entering: true,
            }],
        );
        // Ticks 5-6: still inside the band, so no second begin.
        for _ in 0..2 {
            assert!(pair_events(tick(&mut monitor, &mut store, &scenario, drive)).is_empty());
        }
        assert!((clearance(&store) - 0.1).abs() < 1e-9);
        // Tick 7: -0.3 m of clearance, contact. Contact closes the open band, so
        // the tick reports both edges, in the documented kind order.
        assert_same_events(
            &pair_events(tick(&mut monitor, &mut store, &scenario, drive)),
            &[
                Event::Collision {
                    agent: first,
                    other: second,
                    clearance_m: -0.3,
                    contacting: true,
                },
                Event::NearMiss {
                    agent: first,
                    other: second,
                    clearance_m: -0.3,
                    entering: false,
                },
            ],
        );
        // Ticks 8-9: the overlap persists, so the contact does not repeat.
        for _ in 0..2 {
            assert!(pair_events(tick(&mut monitor, &mut store, &scenario, drive)).is_empty());
        }
        // Tick 10: separated at the tick start and 0.5 m apart, so contact ends
        // and the pair is inside the band again: both edges report, in the
        // documented kind order.
        assert_same_events(
            &pair_events(tick(&mut monitor, &mut store, &scenario, drive)),
            &[
                Event::Collision {
                    agent: first,
                    other: second,
                    clearance_m: 0.5,
                    contacting: false,
                },
                Event::NearMiss {
                    agent: first,
                    other: second,
                    clearance_m: 0.5,
                    entering: true,
                },
            ],
        );
        // Ticks 11-12: still inside the band at some time in the tick.
        for _ in 0..2 {
            assert!(pair_events(tick(&mut monitor, &mut store, &scenario, drive)).is_empty());
        }
        // Tick 13: 1.7 m and separating, so the band ends, once.
        assert_same_events(
            &pair_events(tick(&mut monitor, &mut store, &scenario, drive)),
            &[Event::NearMiss {
                agent: first,
                other: second,
                clearance_m: 1.7,
                entering: false,
            }],
        );
        assert!(monitor.contact.is_empty() && monitor.near.is_empty());
    }

    #[test]
    fn an_open_pair_outside_the_candidate_window_is_closed() {
        let mut store = AgentStore::default();
        let first = push_pedestrian(&mut store, DVec2::ZERO, 0.0, 0.25);
        let second = push_pedestrian(&mut store, DVec2::new(-0.6, 0.0), 0.0, 0.25);
        let mut monitor = SafetyMonitor {
            indexed: vec![
                (
                    first,
                    SweptBody {
                        shape: circle(DVec2::ZERO, 0.25),
                        displacement_m: DVec2::ZERO,
                    },
                ),
                (
                    second,
                    SweptBody {
                        shape: circle(DVec2::new(-0.6, 0.0), 0.25),
                        displacement_m: DVec2::ZERO,
                    },
                ),
            ],
            ..SafetyMonitor::default()
        };
        monitor.near.insert((first, second));
        let mut events = Vec::new();
        monitor.close_stale_pairs(&store, &mut events);
        assert_same_events(
            &events,
            &[Event::NearMiss {
                agent: first,
                other: second,
                clearance_m: 0.1,
                entering: false,
            }],
        );
        assert!(monitor.near.is_empty());
    }

    /// A non-lead (trailer) segment of an articulated chain can contact a
    /// body the lead segment's own box never reaches. The old lead-only
    /// proxy misses this contact entirely; the fix names the touching
    /// trailer segment through a segment-precise `Event::ArticulatedSegmentContact`
    /// instead of a plain `Event::Collision`.
    #[test]
    fn a_trailer_segment_contact_is_detected_and_named() {
        let scenario = no_region_scenario();
        let mut store = AgentStore::default();
        let chain = push_vehicle(&mut store, DVec2::ZERO, 0.0);
        let segments = vec![
            ArticulatedSegmentGeometry {
                length_m: 6.0,
                width_m: 2.5,
                hitch_offset_m: None,
            },
            ArticulatedSegmentGeometry {
                length_m: 13.6,
                width_m: 2.55,
                hitch_offset_m: Some(1.2),
            },
        ];
        store.articulated[chain.index()] =
            Some(ArticulatedState::spawn(segments, 0.9, DVec2::ZERO, 0.0));

        // Recompute the trailer's spawn layout from the actual state rather
        // than trusting hand arithmetic: kingpin trailer, 1.2 m setback,
        // ends up centred 8.6 m behind the tractor's own centre.
        let trailer_position = store.articulated[chain.index()]
            .as_ref()
            .expect("chain state")
            .trailers()[0]
            .position;
        assert!((trailer_position.x - (-8.6)).abs() < 1e-9);
        assert!(trailer_position.y.abs() < 1e-9);

        // A 2 m square obstacle centred at (-10, 0): inside the trailer's
        // x in [-15.4, -1.8] span, clear outside the tractor's x in [-3, 3].
        let obstacle = push_vehicle(&mut store, DVec2::new(-10.0, 0.0), 0.0);
        store.body_length_m[obstacle.index()] = 2.0;
        store.body_width_m[obstacle.index()] = 2.0;

        // Sanity check proving the necessity of this fix: the old lead-only
        // proxy never sees this contact.
        assert!(
            !query::bodies_intersect(
                &query::agent_body(&store, chain.index()),
                &query::agent_body(&store, obstacle.index()),
            ),
            "the lead-only proxy must miss the trailer's contact"
        );

        let mut monitor = SafetyMonitor::default();
        let events = tick(&mut monitor, &mut store, &scenario, |_| {});

        let contact = events
            .iter()
            .find(|event| {
                matches!(
                    event,
                    Event::ArticulatedSegmentContact {
                        contacting: true,
                        ..
                    }
                )
            })
            .unwrap_or_else(|| panic!("expected an articulated segment contact: {events:?}"));
        let Event::ArticulatedSegmentContact {
            agent,
            agent_segment,
            other,
            other_segment,
            contacting,
            ..
        } = contact
        else {
            unreachable!("matched above");
        };
        assert_eq!(*agent, chain);
        assert_eq!(*agent_segment, Some(1), "segment 1 is the trailer");
        assert_eq!(*other, obstacle);
        assert_eq!(*other_segment, None, "the obstacle is not a chain");
        assert!(*contacting);

        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Collision { .. })),
            "a chain-involved contact must not also emit a plain Collision: {events:?}"
        );
    }

    /// A despawn ends the agent's pair and region states without a further
    /// safety record, and closes the queue and control states with their own
    /// departure records.
    #[test]
    fn a_despawn_closes_each_state_exactly_once() {
        let scenario = no_region_scenario();
        let mut store = AgentStore::default();
        let vehicle = push_vehicle(&mut store, DVec2::ZERO, 0.0);
        store.decision[vehicle.index()] = stop_decision();
        let mut monitor = SafetyMonitor::default();
        let events = tick(&mut monitor, &mut store, &scenario, |_| {});
        assert_eq!(
            events,
            vec![
                Event::Queue {
                    agent: vehicle,
                    joined: true,
                },
                Event::ControlTransition {
                    agent: vehicle,
                    control: ControlTransitionKind::SignalStop,
                    active: true,
                },
            ]
        );

        // A second standing tick reports nothing new.
        assert!(tick(&mut monitor, &mut store, &scenario, |_| {}).is_empty());

        let events = tick(&mut monitor, &mut store, &scenario, |store| {
            store.alive[vehicle.index()] = false;
        });
        assert_eq!(
            events,
            vec![
                Event::Queue {
                    agent: vehicle,
                    joined: false,
                },
                Event::ControlTransition {
                    agent: vehicle,
                    control: ControlTransitionKind::SignalStop,
                    active: false,
                },
            ]
        );
        assert!(!monitor.queueing[vehicle.index()] && !monitor.waiting[vehicle.index()]);
    }

    /// A pedestrian's wait decision is a control transition on the crossing
    /// wait state, and it ends when the recorded decision stops waiting.
    #[test]
    fn a_wait_decision_reports_one_control_transition() {
        let scenario = no_region_scenario();
        let mut store = AgentStore::default();
        let pedestrian = push_pedestrian(&mut store, DVec2::ZERO, 1.0, 0.25);
        let mut monitor = SafetyMonitor::default();
        store.pedestrian_decision[pedestrian.index()] = wait_decision(2.0);
        let mut events = tick(&mut monitor, &mut store, &scenario, |_| {});
        events.retain(|event| matches!(event, Event::ControlTransition { .. }));
        assert_eq!(
            events,
            vec![Event::ControlTransition {
                agent: pedestrian,
                control: ControlTransitionKind::CrossingWait,
                active: true,
            }]
        );

        store.pedestrian_decision[pedestrian.index()] = None;
        let mut events = tick(&mut monitor, &mut store, &scenario, |_| {});
        events.retain(|event| matches!(event, Event::ControlTransition { .. }));
        assert_eq!(
            events,
            vec![Event::ControlTransition {
                agent: pedestrian,
                control: ControlTransitionKind::CrossingWait,
                active: false,
            }]
        );
    }

    /// Region occupancy is swept, so a body that crosses a region inside one
    /// tick is reported entering and then leaving, and a pedestrian that entered
    /// against a forbidding signal reports one violation on that entry edge.
    #[test]
    fn region_edges_are_swept_and_report_one_violation_per_entry() {
        let ring = [
            DVec2::new(-1.0, -1.0),
            DVec2::new(1.0, -1.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(-1.0, 1.0),
        ];
        let region = RegionKey::Crossing(hekate_model::CrossingId::from_index(0));
        let mut store = AgentStore::default();
        let pedestrian = push_pedestrian(&mut store, DVec2::new(-5.0, 0.0), 20.0, 0.25);
        store.pedestrian_decision[pedestrian.index()] = cross_against_signal_decision();
        let mut monitor = SafetyMonitor::default();

        // One 8 m step takes the body from x = -5 to x = 3, clean through the
        // 2 m region without ending inside it: the swept circle still reports
        // the entry, and nothing is missed.
        monitor.begin_tick(&store);
        store.position[pedestrian.index()] = DVec2::new(3.0, 0.0);
        monitor.index_bodies(&store);
        let mut events = Vec::new();
        monitor.scan_region(&store, &ring, region, true, &mut events);
        assert_eq!(
            events,
            vec![
                Event::Entry {
                    agent: pedestrian,
                    region,
                },
                Event::Violation {
                    agent: pedestrian,
                    kind: ViolationKind::CrossedAgainstSignal,
                },
            ]
        );

        // Still outside at the next tick end: the region is left, once.
        monitor.begin_tick(&store);
        store.position[pedestrian.index()] = DVec2::new(4.0, 0.0);
        monitor.index_bodies(&store);
        let mut events = Vec::new();
        monitor.scan_region(&store, &ring, region, true, &mut events);
        assert_eq!(
            events,
            vec![Event::Exit {
                agent: pedestrian,
                region,
            }]
        );
    }
}
