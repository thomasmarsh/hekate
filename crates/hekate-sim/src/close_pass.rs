//! Close-pass tracking: overtaking-interval detection and clearance-band
//! duration accumulation.
//!
//! # Model card
//!
//! `docs/schema-v2-contract.md` *Increment 2 events and metrics* has the
//! `ClosePass` observation carry the exact minimum body clearance, its time and
//! relative speed, one duration per declared clearance band keyed by its stable
//! id, the violating bands, and the boundary-crossing and opposing-traversal
//! evidence. This module owns the two facts the observation is built from and
//! that metric definition v1 did not produce: whether an **executed overtake**
//! happened, and how long the passing pair's exact clearance sat inside each
//! configured band while it happened — plus the exact minimum, its time and
//! relative speed, the side, the facility, and the evidence flags the closed
//! observation reports.
//!
//! It is a pure observer. Every input is borrowed from the agent store and the
//! compiled scenario, nothing here writes agent state, and no observation emits
//! an event or a metric: the kernel reads a closed observation and emits one
//! [`crate::Event::ClosePass`] from it. Adding the pass therefore cannot change
//! a simulated trajectory, an event stream, a trace hash, or a golden.
//!
//! ## Detection: an actual overtaking interval
//!
//! Two bodies share a **reference** when they travel the same compiled path in
//! the same travel direction ([`AgentStore::path`] and `direction`, the same
//! reference [`crate::Simulation`] measures longitudinal gaps on). A body's
//! **progress** along that shared reference in its travel direction is
//! `direction * distance_m`, the spelling the kernel's leader query and entry
//! check already use.
//!
//! The pair is **alongside** on the reference when their progress difference is
//! less than the sum of their half lengths along the reference,
//! `|progress_first - progress_second| < (length_first + length_second) / 2`:
//! equivalently their longitudinal footprints overlap. This is a longitudinal
//! overlap on the shared reference, not a Euclidean centre distance.
//!
//! An **overtaking interval** is a maximal run of consecutive observed ticks on
//! which a candidate pair is alongside. It is an *actual overtake*, not two
//! bodies cruising abreast, when one body's progress is strictly advancing on
//! the other's: the body with the greater speed along the shared reference is
//! the **passing agent** and the other the **passed body**. Equal speeds are not
//! an overtake, so an abreast convoy is never mistaken for one. A pair that
//! stops being alongside (or whose members despawn) closes the interval.
//!
//! ## Band accumulation
//!
//! Detection is deliberately independent of clearance: it is tied to the
//! overtaking interval alone. Clearance enters only as the exact world body
//! query [`tick_minimum_clearance_m`], the least signed surface clearance of the
//! tick-swept pair, so a sub-tick approach is not sampled away and no
//! uncalibrated time-to-collision surrogate is read.
//!
//! Each configured [`CompiledClearanceBand`] accumulates its own duration,
//! keyed by its stable [`ClearanceBandId`], while the pair is alongside. A band
//! participates when [`CompiledClearanceBand::applies_to`] holds for either
//! body's compiled mode template; a body with no mode template (a version-1
//! path agent or a pedestrian) can only participate in a band that declares no
//! `applies_to_modes`. A band's `threshold_m` is its definition: the tick's
//! clearance `<= threshold_m` adds the whole step to that band alone. The bands
//! are reported in declaration order, which is the deterministic report order,
//! and each carries its stable id. A band is a **violating** band when it
//! participates, its compiled `violation` is `true`, and the interval's observed
//! minimum fell strictly below the band's threshold.
//!
//! ## Observation closure
//!
//! A completed interval records the least swept clearance observed over it, the
//! tick's simulated time at that minimum, and the relative speed along the
//! shared reference then, so the contract's `min_clearance_m`,
//! `min_clearance_time_s`, and `relative_speed_mps` are the facts a metric
//! aggregate reads rather than a re-derivation. The minimum is a minimum of the
//! symmetric pair query, so it is the same whichever body the query is asked
//! with; the passing agent and passed body keep their roles as separate fields
//! and are never collapsed into a symmetric distance.
//!
//! The observation also carries the side, the facility, and the two evidence
//! flags the contract's `ClosePass` row names. The side and facility come from
//! the two bodies' compiled route state when both ride one shared facility, the
//! side resolved into the passing agent's own travel frame. `crossed_boundary`
//! is `true` when either participant crossed a facility boundary during the
//! interval, read from the kernel's own facility-transition records;
//! `entered_opposing` is `true` when either participant traversed against its
//! facility's authored nominal direction.
//!
//! An interval closes exactly once, when the pair stops being alongside (a
//! completed or aborted pass) or a participant despawns, and every still-open
//! interval is closed by [`ClosePassTracker::close_open`] at run end. Each close
//! records exactly one observation for the pair, so a pair can never be counted
//! twice.
//!
//! ## Determinism and order independence
//!
//! Candidate pairs are canonical ascending `(AgentId, AgentId)` with the lower
//! id first, so a pair has one spelling however it was queried, and the
//! per-pair accumulation is a property of the pair and not of the query order.
//! Completed intervals are recorded in close order — ascending tick, then
//! ascending canonical pair — so the report is a pure function of the states
//! the ticks produced. Nothing here draws a random number, reads a clock, or
//! iterates a hash map.

use std::collections::{BTreeMap, BTreeSet};

use hekate_model::{
    ClearanceBandId, CompiledClearanceBand, CompiledScenario, FacilityId, ModeTemplateId,
    NominalDirection, PathId,
};

use crate::agent::{AgentId, AgentStore};
use crate::metrics::tick_minimum_clearance_m;
use crate::query::{self, BodyShape};
use crate::stage::{FacilityTransitionRecord, PassSide};
use crate::swept::SweptBody;
use crate::time::SimTime;
use crate::units::Seconds;

/// One body's state on its shared reference for one observed tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PassingBody {
    /// The body's stable identifier.
    agent: AgentId,
    /// The compiled path the body travels.
    path: PathId,
    /// Travel direction sign along the path: `1.0` toward the path end, `-1.0`
    /// toward its start.
    direction: f64,
    /// Progress along the shared reference in the travel direction, in metres.
    progress_m: f64,
    /// Half the body's length along the reference, in metres.
    half_length_m: f64,
    /// Speed along the reference in the travel direction, in metres per second.
    speed_mps: f64,
    /// The mode template the body was spawned from, or `None` for a body with
    /// no compiled mode (a version-1 path agent or a pedestrian).
    mode_template: Option<ModeTemplateId>,
    /// The compiled facility the body rides, or `None` for a body with no route
    /// state (a version-1 path agent or a pedestrian).
    facility: Option<FacilityId>,
    /// The body's signed lateral offset on its facility's reference, positive
    /// to the left of its own travel direction, or `None` with no route state.
    lateral_m: Option<f64>,
    /// The body swept over the observed tick, for the exact clearance query.
    body: SweptBody,
}

/// The direction sign of a travel direction: `-1` toward the path start, `+1`
/// otherwise.
fn direction_sign(direction: f64) -> i8 {
    if direction < 0.0 { -1 } else { 1 }
}

/// Whether two bodies travel the same reference in the same direction.
fn same_reference(first: &PassingBody, second: &PassingBody) -> bool {
    first.path == second.path && first.direction == second.direction
}

/// Whether two bodies' longitudinal footprints overlap on the shared reference.
fn alongside(first: &PassingBody, second: &PassingBody) -> bool {
    (first.progress_m - second.progress_m).abs() < first.half_length_m + second.half_length_m
}

/// The passing agent and the passed body of an alongside pair, or `None` when
/// neither passes the other because their speeds along the reference are equal.
fn passing_pair(first: &PassingBody, second: &PassingBody) -> Option<(AgentId, AgentId)> {
    if first.speed_mps > second.speed_mps {
        Some((first.agent, second.agent))
    } else if second.speed_mps > first.speed_mps {
        Some((second.agent, first.agent))
    } else {
        None
    }
}

/// The canonical `(AgentId, AgentId)` key of a pair: the lower id first.
fn canonical_pair(first: AgentId, second: AgentId) -> (AgentId, AgentId) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

/// Whether a compiled band participates for a pair with the given mode
/// templates, one of which may be absent.
///
/// A band declaring no `applies_to_modes` applies to every pair; one that names
/// modes applies only when a body's compiled mode template is named, so a body
/// with no mode template cannot participate through the gate.
fn band_applies(
    band: &CompiledClearanceBand,
    first: Option<ModeTemplateId>,
    second: Option<ModeTemplateId>,
) -> bool {
    match band.applies_to_modes() {
        None => true,
        Some(_) => {
            first.is_some_and(|mode| band.applies_to(mode))
                || second.is_some_and(|mode| band.applies_to(mode))
        }
    }
}

/// One band's duration accumulated within an open overtaking interval.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BandAccumulator {
    /// The band's stable compiled identifier.
    id: ClearanceBandId,
    /// The band's signed surface clearance threshold, in metres.
    threshold_m: f64,
    /// Whether a pass below the threshold is a violation in this scenario.
    violation: bool,
    /// Seconds the pair's exact clearance was at or below `threshold_m`.
    seconds: f64,
}

impl BandAccumulator {
    /// Add `step` to this band's duration when the observed clearance is inside
    /// the band.
    fn accumulate(&mut self, clearance_m: f64, step: Seconds) {
        if clearance_m <= self.threshold_m {
            self.seconds += step.as_secs();
        }
    }
}

/// One overtaking interval that is still open.
#[derive(Debug, Clone, PartialEq)]
struct OpenOvertake {
    /// The passing agent.
    agent: AgentId,
    /// The passed body.
    partner: AgentId,
    /// First observed tick the pair was alongside.
    start_tick: u64,
    /// Last observed tick the pair was alongside.
    last_tick: u64,
    /// The shared facility the pair rides, latched once both bodies carry route
    /// state on one facility.
    facility: Option<FacilityId>,
    /// The side the pass claims, latched into the passing agent's travel frame.
    side: Option<PassSide>,
    /// Whether either body crossed a facility boundary during the interval.
    crossed_boundary: bool,
    /// Whether either body traversed against its facility's nominal direction.
    entered_opposing: bool,
    /// Least signed surface clearance observed over the interval, in metres.
    min_clearance_m: f64,
    /// Simulated time of that minimum, in seconds.
    min_clearance_time_s: f64,
    /// Relative speed along the shared reference at that minimum, in metres per
    /// second: the passing agent's speed minus the passed body's.
    relative_speed_mps: f64,
    /// The participating bands, in declaration order, with their accumulated
    /// durations.
    bands: Vec<BandAccumulator>,
}

impl OpenOvertake {
    /// A freshly opened interval, before its first observed tick.
    fn new(agent: AgentId, partner: AgentId, tick: u64, bands: Vec<BandAccumulator>) -> Self {
        Self {
            agent,
            partner,
            start_tick: tick,
            last_tick: tick,
            facility: None,
            side: None,
            crossed_boundary: false,
            entered_opposing: false,
            min_clearance_m: f64::INFINITY,
            min_clearance_time_s: 0.0,
            relative_speed_mps: 0.0,
            bands,
        }
    }

    /// Record one observed tick of the interval: the tick and time that saw it
    /// alongside, the pair's exact clearance over that tick, and the pair's
    /// relative speed along the shared reference then.
    fn observe(
        &mut self,
        tick: u64,
        time_s: f64,
        clearance_m: f64,
        relative_speed_mps: f64,
        step: Seconds,
    ) {
        self.last_tick = tick;
        if clearance_m < self.min_clearance_m {
            self.min_clearance_m = clearance_m;
            self.min_clearance_time_s = time_s;
            self.relative_speed_mps = relative_speed_mps;
        }
        for band in &mut self.bands {
            band.accumulate(clearance_m, step);
        }
    }

    /// Record the tick's side, facility, and evidence facts, latching the first
    /// resolution the pair admits.
    fn observe_evidence(
        &mut self,
        scenario: &CompiledScenario,
        first: &PassingBody,
        second: &PassingBody,
        transitions: &[FacilityTransitionRecord],
    ) {
        if self.facility.is_none()
            && let (Some(first_facility), Some(second_facility)) = (first.facility, second.facility)
            && first_facility == second_facility
        {
            self.facility = Some(first_facility);
        }
        if self.side.is_none()
            && let (Some(first_lateral), Some(second_lateral)) = (first.lateral_m, second.lateral_m)
        {
            let (passing, passed) = if first.agent == self.agent {
                (first_lateral, second_lateral)
            } else {
                (second_lateral, first_lateral)
            };
            self.side = Some(if passing > passed {
                PassSide::Left
            } else {
                PassSide::Right
            });
        }
        if transitions
            .iter()
            .any(|record| record.agent == self.agent || record.agent == self.partner)
        {
            self.crossed_boundary = true;
        }
        if body_opposes_nominal(scenario, first) || body_opposes_nominal(scenario, second) {
            self.entered_opposing = true;
        }
    }

    /// Convert the closed interval into its observation.
    fn into_observation(self) -> OvertakeObservation {
        let violating_bands: Vec<ClearanceBandId> = self
            .bands
            .iter()
            .filter(|band| band.violation && self.min_clearance_m < band.threshold_m)
            .map(|band| band.id)
            .collect();
        OvertakeObservation {
            agent: self.agent,
            partner: self.partner,
            facility: self.facility,
            side: self.side,
            start_tick: self.start_tick,
            end_tick: self.last_tick,
            min_clearance_m: self.min_clearance_m,
            min_clearance_time_s: self.min_clearance_time_s,
            relative_speed_mps: self.relative_speed_mps,
            bands: self
                .bands
                .into_iter()
                .map(|band| ClosePassBand {
                    band: band.id,
                    duration_s: band.seconds,
                })
                .collect(),
            violating_bands,
            crossed_boundary: self.crossed_boundary,
            entered_opposing: self.entered_opposing,
        }
    }
}

/// Whether one body traverses against its facility's authored nominal direction.
///
/// The body's travel direction sign is compared with the compiled facility's
/// nominal direction: a `Forward` facility makes a reverse-travelling body an
/// opposing traversal, a `Reverse` facility a forward-travelling one, and an
/// `Either` facility makes none. A body with no compiled facility opposes none.
fn body_opposes_nominal(scenario: &CompiledScenario, body: &PassingBody) -> bool {
    let Some(facility) = body
        .facility
        .and_then(|facility| scenario.facility(facility))
    else {
        return false;
    };
    match facility.nominal_direction() {
        NominalDirection::Forward => body.direction < 0.0,
        NominalDirection::Reverse => body.direction >= 0.0,
        NominalDirection::Either => false,
    }
}

/// One clearance band's duration within one overtaking interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosePassBand {
    /// The band's stable compiled identifier.
    pub band: ClearanceBandId,
    /// Seconds the passing pair's exact clearance was at or below the band's
    /// threshold while the pair was alongside.
    pub duration_s: f64,
}

/// One completed overtaking interval and the clearance evidence it accumulated.
///
/// `agent` is the passing body and `partner` the passed body, so the pair is
/// directional and never collapses to a symmetric distance. `facility` and
/// `side` are the shared facility and the side the pass claims, absent when the
/// two bodies carry no shared compiled route state. `bands` is in the scenario's
/// declaration order and omits any band whose `applies_to_modes` gate excludes
/// the pair.
#[derive(Debug, Clone, PartialEq)]
pub struct OvertakeObservation {
    /// The passing agent: the body with the greater speed along the shared
    /// reference.
    pub agent: AgentId,
    /// The passed body.
    pub partner: AgentId,
    /// The shared facility the pass happened on, when both bodies ride one.
    pub facility: Option<FacilityId>,
    /// The side the pass claims in the passing agent's travel frame, when the
    /// two bodies' lateral offsets resolve it.
    pub side: Option<PassSide>,
    /// First observed tick the pair was alongside.
    pub start_tick: u64,
    /// Last observed tick the pair was alongside.
    pub end_tick: u64,
    /// Least signed surface clearance observed over the interval, in metres.
    pub min_clearance_m: f64,
    /// Simulated time of that minimum, in seconds.
    pub min_clearance_time_s: f64,
    /// Relative speed along the shared reference at that minimum, in metres per
    /// second: the passing agent's speed minus the passed body's.
    pub relative_speed_mps: f64,
    /// The participating bands in declaration order, each with its accumulated
    /// seconds.
    pub bands: Vec<ClosePassBand>,
    /// The participating bands whose observed minimum fell below their threshold
    /// and that record a violation, in declaration order.
    pub violating_bands: Vec<ClearanceBandId>,
    /// Whether either participant crossed a facility boundary during the
    /// interval.
    pub crossed_boundary: bool,
    /// Whether either participant traversed against its facility's nominal
    /// direction during the interval.
    pub entered_opposing: bool,
}

/// Online close-pass tracking for one run.
///
/// Fed once per tick from the integrated state by
/// [`Simulation`](crate::Simulation); read through
/// [`Simulation::close_pass_tracker`](crate::Simulation::close_pass_tracker).
/// Fields are private: the pass only borrows state, and no caller can inject an
/// observation a tick did not produce. See the module card for the detection
/// definition, the band accumulation rule, and the determinism argument.
#[derive(Debug, Default)]
pub struct ClosePassTracker {
    /// Body shape of every live agent at the tick start, by slot.
    start_shapes: Vec<Option<BodyShape>>,
    /// Per-tick bodies, ascending by [`AgentId`].
    bodies: Vec<PassingBody>,
    /// Body indices sorted by `(path, direction, progress)`, reused per tick.
    order: Vec<usize>,
    /// Reused candidate buffer of alongside body-index pairs, in scan order.
    candidates: Vec<(usize, usize)>,
    /// Open overtaking intervals, ascending by canonical pair.
    open: BTreeMap<(AgentId, AgentId), OpenOvertake>,
    /// Canonical pairs observed alongside on the tick being observed.
    seen: BTreeSet<(AgentId, AgentId)>,
    /// Completed intervals, in close order: ascending tick, then pair.
    overtakes: Vec<OvertakeObservation>,
}

impl ClosePassTracker {
    /// Record every live body's tick-start shape.
    ///
    /// Call once before the state-affecting loop, so the swept bodies this pass
    /// reads carry the whole tick's displacement, exactly as the interaction
    /// metrics pass does.
    pub(crate) fn begin_tick(&mut self, agents: &AgentStore) {
        self.start_shapes.clear();
        self.start_shapes.resize(agents.len(), None);
        for index in 0..agents.len() {
            if agents.alive[index] {
                self.start_shapes[index] = Some(query::agent_body(agents, index));
            }
        }
    }

    /// Observe one integrated tick: open, extend, or close the overtaking
    /// interval of every alongside pair.
    ///
    /// Call once per tick, after the bodies have stepped and before new demand
    /// is admitted, so the bodies observed are exactly the ones the tick
    /// integrated. The pass borrows everything it reads and writes only its own
    /// records.
    pub(crate) fn observe(
        &mut self,
        agents: &AgentStore,
        scenario: &CompiledScenario,
        transitions: &[FacilityTransitionRecord],
        tick: u64,
        step: Seconds,
    ) {
        self.build_bodies(agents);
        self.order_bodies();
        self.collect_candidates();
        self.advance_intervals(scenario, transitions, tick, step);
    }

    /// Completed overtaking intervals, in close order.
    ///
    /// A pair that is still alongside when the run's last tick has been observed
    /// leaves its interval open here; [`Self::close_open`] closes it as a
    /// termination when the run ends.
    pub fn overtakes(&self) -> &[OvertakeObservation] {
        &self.overtakes
    }

    /// Close every interval still open at the run's end, recording each as a
    /// termination.
    ///
    /// A pass still in progress when the run ends terminates with the run, so
    /// closing it here keeps the "exactly one observation per participant pair"
    /// promise: a pair that never separated still yields its one observation,
    /// and closing twice is impossible because `observe` never reopens a pair
    /// whose interval has closed. The closed observations are recorded in
    /// `(end tick, pair)` order, appended after the tick closures. This is the
    /// run-end read surface a metric aggregate uses; a tick closure still emits
    /// its own event, while a run-end closure has no tick left to carry one.
    pub fn close_open(&mut self) {
        let keys: Vec<(AgentId, AgentId)> = self.open.keys().copied().collect();
        let mut closed: Vec<OvertakeObservation> = keys
            .into_iter()
            .filter_map(|key| self.open.remove(&key))
            .map(OpenOvertake::into_observation)
            .collect();
        closed.sort_by_key(|observation| {
            (
                observation.end_tick,
                canonical_pair(observation.agent, observation.partner),
            )
        });
        self.overtakes.extend(closed);
    }

    /// Build the per-tick passing body of every live agent.
    fn build_bodies(&mut self, agents: &AgentStore) {
        self.bodies.clear();
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            let end = query::agent_body(agents, index);
            let start = self.start_shapes[index].unwrap_or(end);
            let direction = agents.direction[index];
            self.bodies.push(PassingBody {
                agent: AgentId::from_index(index),
                path: agents.path[index],
                direction,
                progress_m: direction * agents.distance_m[index],
                half_length_m: agents.body_length_m[index] * 0.5,
                speed_mps: agents.speed_mps[index],
                mode_template: agents.route_state[index].map(|state| state.mode_template),
                facility: agents.route_state[index].map(|state| state.facility),
                lateral_m: agents.route_state[index].map(|state| state.d_m),
                body: SweptBody {
                    shape: start,
                    displacement_m: end.centre() - start.centre(),
                },
            });
        }
    }

    /// Sort the bodies by `(path, direction, progress)` so same-reference bodies
    /// are contiguous and ascending along the reference.
    fn order_bodies(&mut self) {
        let bodies = &self.bodies;
        self.order.clear();
        self.order.extend(0..bodies.len());
        self.order.sort_by(|&first, &second| {
            bodies[first]
                .path
                .cmp(&bodies[second].path)
                .then(
                    direction_sign(bodies[first].direction)
                        .cmp(&direction_sign(bodies[second].direction)),
                )
                .then(
                    bodies[first]
                        .progress_m
                        .total_cmp(&bodies[second].progress_m),
                )
        });
    }

    /// Collect every alongside pair as a candidate, in a deterministic scan
    /// order.
    ///
    /// Within one `(path, direction)` run the bodies are ascending by progress,
    /// so a body's alongside partners are a contiguous suffix of the run whose
    /// gap is below the run's longest half-length sum; the scan stops at the
    /// first body past that bound.
    fn collect_candidates(&mut self) {
        let bodies = &self.bodies;
        let order = &self.order;
        self.candidates.clear();
        let mut run_start = 0;
        while run_start < order.len() {
            let mut run_end = run_start + 1;
            while run_end < order.len()
                && same_reference(&bodies[order[run_start]], &bodies[order[run_end]])
            {
                run_end += 1;
            }
            let furthest_half_m = (run_start..run_end)
                .map(|position| bodies[order[position]].half_length_m)
                .fold(0.0, f64::max);
            for position in run_start..run_end {
                let first = &bodies[order[position]];
                for other in (position + 1)..run_end {
                    let second = &bodies[order[other]];
                    if second.progress_m - first.progress_m >= first.half_length_m + furthest_half_m
                    {
                        break;
                    }
                    if alongside(first, second) {
                        self.candidates.push((order[position], order[other]));
                    }
                }
            }
            run_start = run_end;
        }
    }

    /// Open, extend, or close the overtaking interval of every candidate pair,
    /// then close the intervals no candidate revisited this tick.
    fn advance_intervals(
        &mut self,
        scenario: &CompiledScenario,
        transitions: &[FacilityTransitionRecord],
        tick: u64,
        step: Seconds,
    ) {
        self.seen.clear();
        let time_s = SimTime::from_tick(tick, step).seconds();
        for position in 0..self.candidates.len() {
            let (first_index, second_index) = self.candidates[position];
            let first = self.bodies[first_index];
            let second = self.bodies[second_index];
            let key = canonical_pair(first.agent, second.agent);
            self.seen.insert(key);
            let clearance_m = tick_minimum_clearance_m(&first.body, &second.body);
            if let Some(open) = self.open.get_mut(&key) {
                let relative_speed_mps = relative_speed_mps(&first, &second, open.agent);
                open.observe(tick, time_s, clearance_m, relative_speed_mps, step);
                open.observe_evidence(scenario, &first, &second, transitions);
                continue;
            }
            let Some((agent, partner)) = passing_pair(&first, &second) else {
                continue;
            };
            let relative_speed_mps = relative_speed_mps(&first, &second, agent);
            let mut open = OpenOvertake::new(
                agent,
                partner,
                tick,
                participating_bands(scenario, &first, &second),
            );
            open.observe(tick, time_s, clearance_m, relative_speed_mps, step);
            open.observe_evidence(scenario, &first, &second, transitions);
            self.open.insert(key, open);
        }
        let closing: Vec<(AgentId, AgentId)> = self
            .open
            .keys()
            .filter(|key| !self.seen.contains(key))
            .copied()
            .collect();
        for key in closing {
            if let Some(open) = self.open.remove(&key) {
                self.overtakes.push(open.into_observation());
            }
        }
    }
}

/// The pair's relative speed along the shared reference in the passing agent's
/// travel direction: the passing agent's speed minus the passed body's.
///
/// Both bodies travel the same reference in the same direction, so the sign says
/// which body is closing on the other and the roles the observation retains give
/// the sign its meaning.
fn relative_speed_mps(first: &PassingBody, second: &PassingBody, agent: AgentId) -> f64 {
    if first.agent == agent {
        first.speed_mps - second.speed_mps
    } else {
        second.speed_mps - first.speed_mps
    }
}

/// The participating bands of a pair, in declaration order.
fn participating_bands(
    scenario: &CompiledScenario,
    first: &PassingBody,
    second: &PassingBody,
) -> Vec<BandAccumulator> {
    scenario
        .clearance_bands()
        .iter()
        .filter(|band| band_applies(band, first.mode_template, second.mode_template))
        .map(|band| BandAccumulator {
            id: band.id(),
            threshold_m: band.threshold_m(),
            violation: band.violation(),
            seconds: 0.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use hekate_model::{CompiledScenario, MovementDirection, parse_scenario_source_v2};

    use crate::agent::{AgentInit, AgentMode, RouteState};
    use crate::stage::TransitionKind;

    /// The default kernel step the analytic tests advance by.
    const DT: f64 = 0.1;

    /// The straight reference every analytic test travels: the world x axis, so
    /// progress is the x coordinate and a lateral offset is the y coordinate.
    const REFERENCE_LENGTH_M: f64 = 200.0;

    /// Append a circular body of radius `radius_m` at `position` with the given
    /// along-reference progress, speed, and travel direction.
    fn push_circle(
        store: &mut AgentStore,
        path: PathId,
        progress_m: f64,
        direction: f64,
        position: DVec2,
        radius_m: f64,
        speed_mps: f64,
    ) -> AgentId {
        push_body(
            store, path, progress_m, direction, position, radius_m, speed_mps, None,
        )
    }

    /// Append a circular body that rides `facility` with route state, so the
    /// tracker can resolve its facility, side, and nominal-direction evidence.
    #[allow(clippy::too_many_arguments)]
    fn push_rider(
        store: &mut AgentStore,
        scenario: &CompiledScenario,
        facility: FacilityId,
        mode: ModeTemplateId,
        distance_m: f64,
        direction: f64,
        position: DVec2,
        radius_m: f64,
        speed_mps: f64,
    ) -> AgentId {
        let geometry = scenario
            .facility(facility)
            .and_then(|facility| facility.reference())
            .expect("the facility has a compiled reference")
            .geometry();
        let route = RouteState::project(mode, facility, geometry, position, direction, None, None);
        push_body(
            store,
            PathId::from_index(0),
            distance_m,
            direction,
            position,
            radius_m,
            speed_mps,
            Some(route),
        )
    }

    /// Append a circular body of radius `radius_m` at `position` with the given
    /// along-reference progress, speed, travel direction, and optional route
    /// state.
    #[allow(clippy::too_many_arguments)]
    fn push_body(
        store: &mut AgentStore,
        path: PathId,
        progress_m: f64,
        direction: f64,
        position: DVec2,
        radius_m: f64,
        speed_mps: f64,
        route_state: Option<RouteState>,
    ) -> AgentId {
        store.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path,
            distance_m: progress_m,
            speed_mps,
            position,
            heading_rad: 0.0,
            body_length_m: radius_m * 2.0,
            body_width_m: radius_m * 2.0,
            direction,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state,
        })
    }

    /// Advance both bodies along the reference at their own speeds and observe
    /// the tick, so a test can drive the tracker without the physics kernel.
    fn advance_and_observe(
        tracker: &mut ClosePassTracker,
        store: &mut AgentStore,
        scenario: &CompiledScenario,
        tick: u64,
    ) {
        tracker.begin_tick(store);
        advance_store(store);
        tracker.observe(store, scenario, &[], tick, Seconds::from_secs(DT));
    }

    /// Move every live body along the reference by its own speed for one step.
    fn advance_store(store: &mut AgentStore) {
        for index in 0..store.len() {
            if !store.alive[index] {
                continue;
            }
            let direction = store.direction[index];
            let step_m = store.speed_mps[index] * DT * direction;
            store.distance_m[index] += step_m;
            store.position[index].x += step_m;
        }
    }

    /// One facility boundary crossing of `agent`, for the tracker's evidence.
    fn boundary_crossing(agent: AgentId) -> FacilityTransitionRecord {
        FacilityTransitionRecord {
            agent,
            from_facility: FacilityId::from_index(0),
            to_facility: FacilityId::from_index(1),
            from_direction: MovementDirection::Forward,
            to_direction: MovementDirection::Forward,
            via: TransitionKind::Lateral,
            side: PassSide::Left,
            s_m: 10.0,
            d_m: 1.0,
            permitted: true,
        }
    }

    /// Drive `ticks` ticks of two circles that start `gap_m` apart in progress
    /// and `offset_m` apart laterally, the first moving at `fast_mps` and the
    /// second at `slow_mps` in the `+x` travel direction.
    fn run_two_circles(
        scenario: &CompiledScenario,
        ticks: u64,
        gap_m: f64,
        offset_m: f64,
        fast_mps: f64,
        slow_mps: f64,
    ) -> ClosePassTracker {
        let mut store = AgentStore::default();
        let path = PathId::from_index(0);
        push_circle(
            &mut store,
            path,
            0.0,
            1.0,
            DVec2::new(0.0, 0.0),
            0.5,
            fast_mps,
        );
        push_circle(
            &mut store,
            path,
            gap_m,
            1.0,
            DVec2::new(gap_m, offset_m),
            0.5,
            slow_mps,
        );
        let mut tracker = ClosePassTracker::default();
        for tick in 1..=ticks {
            advance_and_observe(&mut tracker, &mut store, scenario, tick);
        }
        tracker
    }

    /// A version-2 document with a straight reference, the given clearance
    /// bands, and a `bike` mode a band can name, and no demand, so a test can
    /// read the compiled bands and drive the tracker by hand.
    fn scenario_with_bands(bands: &str) -> CompiledScenario {
        let source = parse_scenario_source_v2(&format!(
            "{{ schema_version: 2, id: 'close_pass', \
             coordinate_system: {{ x: 'east_m', y: 'north_m' }}, \
             paths: [ {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, \
             {{ x: {REFERENCE_LENGTH_M}, y: 0.0 }} ] }} ], \
             portals: [ {{ id: 'entry', path: 'guide', end: 'start', width_m: 7.0 }}, \
             {{ id: 'exit', path: 'guide', end: 'end', width_m: 7.0 }} ], \
             regions: [ {{ id: 'band', points: [ {{ x: 0.0, y: -3.0 }}, \
             {{ x: {REFERENCE_LENGTH_M}, y: -3.0 }}, \
             {{ x: {REFERENCE_LENGTH_M}, y: 3.0 }}, {{ x: 0.0, y: 3.0 }} ] }} ], \
             facilities: [ {{ id: 'guide_facility', region: 'band', \
             reference_path: 'guide', width_m: 6.0, nominal_direction: 'forward', \
             access: {{ modes: [ 'car', 'bike' ] }}, lateral_use: 'shared', \
             lateral_policy: {{ passing_side: 'left' }}, \
             speed_policy: {{ limit_mps: null }} }} ], \
             mode_templates: [ \
             {{ id: 'car', body: {{ kind: 'box', length_m: {{ min: 4.0, max: 4.0 }}, \
             width_m: {{ min: 1.8, max: 1.8 }} }}, motion: 'single_body_wheeled', \
             tactics: [ 'follow', 'stop', 'yield' ], \
             access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either', \
             speed_policy: {{ limit_mps: null }} }}, occupancy: 'operator_only', \
             profiles: {{ speed_mps: {{ min: 8.0, max: 8.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 1.2 }}, \
             comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }}, \
             time_gap_s: {{ min: 1.0, max: 1.0 }}, \
             compliance: {{ min: 1.0, max: 1.0 }} }} }}, \
             {{ id: 'bike', body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }}, \
             radius_m: {{ min: 0.35, max: 0.35 }} }}, motion: 'single_body_wheeled', \
             tactics: [ 'follow', 'stop', 'yield' ], \
             access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either', \
             speed_policy: {{ limit_mps: null }} }}, occupancy: 'operator_only', \
             profiles: {{ speed_mps: {{ min: 4.0, max: 4.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 1.2 }}, \
             comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }}, \
             time_gap_s: {{ min: 1.0, max: 1.0 }}, \
             steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }}, \
             lateral_clearance_m: {{ min: 0.3, max: 0.3 }}, \
             compliance: {{ min: 1.0, max: 1.0 }} }} }} ], \
             clearance_bands: [{bands}] }}"
        ))
        .expect("the document is version 2");
        CompiledScenario::compile_v2(source).expect("the scenario compiles")
    }

    /// The dense id of a compiled mode template named `id`.
    fn mode_id(scenario: &CompiledScenario, id: &str) -> ModeTemplateId {
        let position = scenario
            .mode_templates()
            .iter()
            .position(|template| template.id() == id)
            .unwrap_or_else(|| panic!("the scenario declares mode '{id}'"));
        ModeTemplateId::from_index(position)
    }

    /// The single band id of an observation's band list.
    fn only_band(observation: &OvertakeObservation) -> ClosePassBand {
        assert_eq!(
            observation.bands.len(),
            1,
            "expected exactly one participating band, got {:?}",
            observation.bands
        );
        observation.bands[0]
    }

    #[test]
    fn a_close_overtake_accumulates_its_bands_duration() {
        // Two half-metre circles 1.2 m apart laterally pass with 0.2 m of
        // minimum surface clearance, inside a 0.5 m band, so the band
        // accumulates a positive, sub-interval duration.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let tracker = run_two_circles(&scenario, 80, 3.0, 1.2, 2.0, 1.0);
        let observed = tracker.overtakes();
        assert_eq!(observed.len(), 1, "exactly one overtake of the slow body");
        let observation = &observed[0];
        assert_eq!(observation.agent, AgentId::from_index(0));
        assert_eq!(observation.partner, AgentId::from_index(1));
        let band = only_band(observation);
        assert_eq!(band.band, ClearanceBandId::from_index(0));
        assert!(
            band.duration_s > 0.0 && band.duration_s < 6.0,
            "close band duration {} must be positive and below the full run",
            band.duration_s
        );
    }

    #[test]
    fn a_safe_overtake_records_the_interval_but_no_band_duration() {
        // The same pass two metres apart laterally keeps 1 m of surface
        // clearance, above the 0.5 m band, so the interval is recorded and the
        // band stays at zero: the band is a definition, not a safety claim.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let tracker = run_two_circles(&scenario, 80, 3.0, 2.0, 2.0, 1.0);
        let observed = tracker.overtakes();
        assert_eq!(observed.len(), 1);
        assert_eq!(only_band(&observed[0]).duration_s, 0.0);
    }

    #[test]
    fn nested_bands_accumulate_independently_and_monotonically() {
        // A 0.5 m and a 1.5 m band: the wider threshold is inside longer, so it
        // accumulates at least as much, and both retain their stable ids in
        // declaration order.
        let scenario = scenario_with_bands(
            "{ id: 'inner', threshold_m: 0.3, violation: true }, \
             { id: 'outer', threshold_m: 1.5, violation: false }",
        );
        let tracker = run_two_circles(&scenario, 80, 3.0, 1.2, 2.0, 1.0);
        let observed = tracker.overtakes();
        assert_eq!(observed.len(), 1);
        let observation = &observed[0];
        assert_eq!(
            observation
                .bands
                .iter()
                .map(|band| band.band)
                .collect::<Vec<_>>(),
            vec![
                ClearanceBandId::from_index(0),
                ClearanceBandId::from_index(1)
            ],
            "declaration order and stable ids are retained"
        );
        let inner = observation.bands[0].duration_s;
        let outer = observation.bands[1].duration_s;
        assert!(inner > 0.0, "the inner band is crossed");
        assert!(
            outer > inner,
            "the wider band accumulates at least as long: {outer} vs {inner}"
        );
    }

    #[test]
    fn a_band_whose_threshold_the_clearance_only_crosses_accumulates_a_sub_interval() {
        // Passing 1.2 m apart laterally, the clearance dips from ~0.56 m at the
        // interval edge to 0.2 m at closest approach, so a 0.4 m band is crossed
        // for a strict subset of the interval while a 0.6 m band spans all of it.
        let scenario = scenario_with_bands(
            "{ id: 'crossed', threshold_m: 0.4, violation: true }, \
             { id: 'spanned', threshold_m: 0.6, violation: false }",
        );
        let tracker = run_two_circles(&scenario, 80, 3.0, 1.2, 2.0, 1.0);
        let observed = tracker.overtakes();
        assert_eq!(observed.len(), 1);
        let observation = &observed[0];
        let crossed = observation.bands[0].duration_s;
        let spanned = observation.bands[1].duration_s;
        assert!(crossed > 0.0, "the crossed band accumulated a duration");
        assert!(
            crossed < spanned,
            "a boundary the clearance crosses mid-interval accumulates strictly \
             less than one it stays inside: {crossed} vs {spanned}"
        );
    }

    #[test]
    fn an_abreast_convoy_is_not_an_overtake() {
        // Equal speeds: neither body's progress passes the other's, so no
        // interval opens however close the pair comes.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let tracker = run_two_circles(&scenario, 40, 1.0, 1.2, 1.0, 1.0);
        assert!(tracker.overtakes().is_empty());
    }

    #[test]
    fn a_head_on_approach_is_not_an_overtake() {
        // The bodies travel the reference in opposite directions, so they do
        // not share a travel direction and the pass is a meeting, not an
        // overtake.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let mut store = AgentStore::default();
        let path = PathId::from_index(0);
        push_circle(&mut store, path, 0.0, 1.0, DVec2::new(0.0, 0.0), 0.5, 1.0);
        push_circle(&mut store, path, 4.0, -1.0, DVec2::new(4.0, 1.2), 0.5, 1.0);
        let mut tracker = ClosePassTracker::default();
        for tick in 1..=80 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        assert!(tracker.overtakes().is_empty());
    }

    #[test]
    fn bodies_on_different_paths_are_not_an_overtake() {
        // The same pass on two different references has no shared reference to
        // measure progress on, so it is not detected.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let mut store = AgentStore::default();
        push_circle(
            &mut store,
            PathId::from_index(0),
            0.0,
            1.0,
            DVec2::new(0.0, 0.0),
            0.5,
            2.0,
        );
        push_circle(
            &mut store,
            PathId::from_index(1),
            3.0,
            1.0,
            DVec2::new(3.0, 1.2),
            0.5,
            1.0,
        );
        let mut tracker = ClosePassTracker::default();
        for tick in 1..=80 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        assert!(tracker.overtakes().is_empty());
    }

    #[test]
    fn the_mode_gate_excludes_a_band_that_names_no_present_mode() {
        let scenario = scenario_with_bands(
            "{ id: 'named', threshold_m: 0.5, violation: true, applies_to_modes: [ 'bike' ] }, \
             { id: 'every', threshold_m: 1.0, violation: false }",
        );
        let bands = scenario.clearance_bands();
        let bike = mode_id(&scenario, "bike");
        let car = mode_id(&scenario, "car");
        // A band naming a mode is excluded for a pair that carries no named
        // mode, while a band declaring none applies to every pair.
        assert!(!band_applies(&bands[0], None, None));
        assert!(!band_applies(&bands[0], Some(car), None));
        assert!(band_applies(&bands[1], None, None));
        // A present matching mode on either body is enough.
        assert!(band_applies(&bands[0], Some(bike), None));
        assert!(band_applies(&bands[0], None, Some(bike)));
    }

    #[test]
    fn the_report_order_is_the_pair_key_and_not_the_query_order() {
        // Two independent overtakes in one run close on their own ticks; the
        // completed observations are ordered by close tick and pair, so two
        // runs of the same states report identically.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let first = run_two_circles(&scenario, 80, 3.0, 1.2, 2.0, 1.0);
        let second = run_two_circles(&scenario, 80, 3.0, 1.2, 2.0, 1.0);
        assert_eq!(first.overtakes(), second.overtakes());
        let observation = &first.overtakes()[0];
        assert!(observation.start_tick <= observation.end_tick);
    }

    #[test]
    fn the_minimum_records_its_time_and_relative_speed() {
        // Two half-metre circles 1.2 m apart pass, so the least surface
        // clearance is 0.2 m, at some tick inside the interval, with the 1.0 m/s
        // speed difference recorded then.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let tracker = run_two_circles(&scenario, 80, 3.0, 1.2, 2.0, 1.0);
        let observation = &tracker.overtakes()[0];
        assert!(
            (observation.min_clearance_m - 0.2).abs() < 1e-9,
            "the least clearance is the exact 0.2 m, got {}",
            observation.min_clearance_m
        );
        assert!(
            observation.min_clearance_time_s > 0.0
                && observation.min_clearance_time_s <= 80.0 * DT + 1e-9,
            "the minimum's time lies inside the run: {}",
            observation.min_clearance_time_s
        );
        assert!(
            (observation.relative_speed_mps - 1.0).abs() < 1e-9,
            "the relative speed at the minimum is the 1.0 m/s difference, got {}",
            observation.relative_speed_mps
        );
    }

    #[test]
    fn a_violating_band_is_reported_only_when_the_minimum_falls_below_it() {
        let close = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let observed = run_two_circles(&close, 80, 3.0, 1.2, 2.0, 1.0);
        assert_eq!(
            observed.overtakes()[0].violating_bands,
            vec![ClearanceBandId::from_index(0)],
            "0.2 m clear of a 0.5 m violating band is a violation"
        );
        let safe = run_two_circles(&close, 80, 3.0, 2.0, 2.0, 1.0);
        assert!(
            safe.overtakes()[0].violating_bands.is_empty(),
            "1 m clear of a 0.5 m band is not a violation"
        );
        let study = scenario_with_bands("{ id: 'study', threshold_m: 0.5, violation: false }");
        let labelled = run_two_circles(&study, 80, 3.0, 1.2, 2.0, 1.0);
        assert!(
            labelled.overtakes()[0].violating_bands.is_empty(),
            "a non-violating band defines a duration, not a violation"
        );
    }

    #[test]
    fn the_minimum_is_symmetric_in_pair_order_while_the_roles_follow_the_speeds() {
        // The same two bodies, once with agent 0 behind and faster, once with
        // agent 1 behind and faster. The pair geometry per tick is identical, so
        // the symmetric minimum is identical, while the passing agent and passed
        // body swap with the speeds.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let pass = |zero_behind: bool| {
            let mut store = AgentStore::default();
            let path = PathId::from_index(0);
            let (zero_x, one_x, zero_speed, one_speed) = if zero_behind {
                (0.0, 3.0, 2.0, 1.0)
            } else {
                (3.0, 0.0, 1.0, 2.0)
            };
            push_circle(
                &mut store,
                path,
                zero_x,
                1.0,
                DVec2::new(zero_x, 0.0),
                0.5,
                zero_speed,
            );
            push_circle(
                &mut store,
                path,
                one_x,
                1.0,
                DVec2::new(one_x, 1.2),
                0.5,
                one_speed,
            );
            let mut tracker = ClosePassTracker::default();
            for tick in 1..=80 {
                advance_and_observe(&mut tracker, &mut store, &scenario, tick);
            }
            tracker
        };
        let first = pass(true);
        let second = pass(false);
        assert_eq!(
            (first.overtakes()[0].agent, first.overtakes()[0].partner),
            (AgentId::from_index(0), AgentId::from_index(1))
        );
        assert_eq!(
            (second.overtakes()[0].agent, second.overtakes()[0].partner),
            (AgentId::from_index(1), AgentId::from_index(0))
        );
        assert_eq!(
            first.overtakes()[0].min_clearance_m,
            second.overtakes()[0].min_clearance_m,
            "the minimum does not depend on the pair query order"
        );
    }

    #[test]
    fn a_boundary_crossing_is_evidence_on_the_observation() {
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let run = |crossing: bool| {
            let mut store = AgentStore::default();
            let path = PathId::from_index(0);
            push_circle(&mut store, path, 0.0, 1.0, DVec2::new(0.0, 0.0), 0.5, 2.0);
            push_circle(&mut store, path, 3.0, 1.0, DVec2::new(3.0, 1.2), 0.5, 1.0);
            let mut tracker = ClosePassTracker::default();
            let records = if crossing {
                vec![boundary_crossing(AgentId::from_index(0))]
            } else {
                Vec::new()
            };
            for tick in 1..=80 {
                tracker.begin_tick(&store);
                advance_store(&mut store);
                tracker.observe(&store, &scenario, &records, tick, Seconds::from_secs(DT));
            }
            tracker
        };
        assert!(
            run(true).overtakes()[0].crossed_boundary,
            "a participant's boundary crossing is recorded"
        );
        assert!(
            !run(false).overtakes()[0].crossed_boundary,
            "no crossing records no evidence"
        );
    }

    #[test]
    fn an_aborted_pass_closes_exactly_one_observation() {
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let mut store = AgentStore::default();
        let path = PathId::from_index(0);
        push_circle(&mut store, path, 0.0, 1.0, DVec2::new(0.0, 0.0), 0.5, 2.0);
        push_circle(&mut store, path, 3.0, 1.0, DVec2::new(3.0, 1.2), 0.5, 1.0);
        let mut tracker = ClosePassTracker::default();
        // Come alongside and hold the interval open...
        for tick in 1..=30 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        assert!(
            tracker.overtakes().is_empty(),
            "the interval is still open while the pair is alongside"
        );
        // ...then the faster body brakes below the passed body's speed and drops
        // back, so the pass aborts and the pair separates.
        store.speed_mps[0] = 0.2;
        for tick in 31..=80 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        let observed = tracker.overtakes();
        assert_eq!(
            observed.len(),
            1,
            "one observation closes for the aborted pass"
        );
        assert!(
            observed[0].end_tick < 80,
            "the abort closed it before the end"
        );
    }

    #[test]
    fn a_despawn_closes_exactly_one_observation() {
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let mut store = AgentStore::default();
        let path = PathId::from_index(0);
        push_circle(&mut store, path, 0.0, 1.0, DVec2::new(0.0, 0.0), 0.5, 2.0);
        push_circle(&mut store, path, 3.0, 1.0, DVec2::new(3.0, 1.2), 0.5, 1.0);
        let mut tracker = ClosePassTracker::default();
        for tick in 1..=30 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        // The passed body leaves the world, so the interval terminates.
        store.alive[1] = false;
        for tick in 31..=60 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        assert_eq!(
            tracker.overtakes().len(),
            1,
            "a participant's despawn closes exactly one observation"
        );
    }

    #[test]
    fn close_open_closes_a_still_open_interval_once() {
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let mut store = AgentStore::default();
        let path = PathId::from_index(0);
        push_circle(&mut store, path, 0.0, 1.0, DVec2::new(0.0, 0.0), 0.5, 2.0);
        push_circle(&mut store, path, 3.0, 1.0, DVec2::new(3.0, 1.2), 0.5, 1.0);
        let mut tracker = ClosePassTracker::default();
        for tick in 1..=30 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        tracker.close_open();
        let observed = tracker.overtakes();
        assert_eq!(observed.len(), 1, "the run end closes the open interval");
        assert_eq!(
            observed[0].end_tick, 30,
            "it closes at its last observed tick"
        );
        tracker.close_open();
        assert_eq!(
            tracker.overtakes().len(),
            1,
            "a second run-end close adds no second observation"
        );
    }

    #[test]
    fn a_body_on_the_opposing_traversal_records_its_evidence() {
        // The scenario's facility is authored forward, but both bodies ride it
        // in reverse, the faster behind the slower. The pass is still detected
        // on the shared reference, and each body's traversal opposes the
        // facility's nominal direction, so the observation records it.
        let scenario = scenario_with_bands("{ id: 'close', threshold_m: 0.5, violation: true }");
        let facility = FacilityId::from_index(0);
        let mode = mode_id(&scenario, "car");
        let mut store = AgentStore::default();
        push_rider(
            &mut store,
            &scenario,
            facility,
            mode,
            3.0,
            -1.0,
            DVec2::new(3.0, 0.0),
            0.5,
            2.0,
        );
        push_rider(
            &mut store,
            &scenario,
            facility,
            mode,
            0.0,
            -1.0,
            DVec2::new(0.0, 1.2),
            0.5,
            1.0,
        );
        let mut tracker = ClosePassTracker::default();
        for tick in 1..=80 {
            advance_and_observe(&mut tracker, &mut store, &scenario, tick);
        }
        let observed = tracker.overtakes();
        assert_eq!(
            observed.len(),
            1,
            "the reverse-travelling pair still passes"
        );
        let observation = &observed[0];
        assert_eq!(observation.agent, AgentId::from_index(0));
        assert_eq!(observation.facility, Some(facility));
        assert_eq!(observation.side, Some(PassSide::Left));
        assert!(
            observation.entered_opposing,
            "travelling against the nominal direction is an opposing traversal"
        );
    }
}
