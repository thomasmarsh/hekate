//! Close-pass tracking: overtaking-interval detection and clearance-band
//! duration accumulation.
//!
//! # Model card
//!
//! `docs/schema-v2-contract.md` *Increment 2 events and metrics* has the
//! `ClosePass` observation carry the exact minimum body clearance, its time and
//! relative speed, one duration per declared clearance band keyed by its stable
//! id, and the violating bands. This module owns the two facts the observation
//! is built from and that metric definition v1 did not produce: whether an
//! **executed overtake** happened, and how long the passing pair's exact
//! clearance sat inside each configured band while it happened.
//!
//! It is a pure observer. Every input is borrowed from the agent store and the
//! compiled scenario, nothing here writes agent state, and no observation emits
//! an event or a metric. Adding the pass therefore cannot change a simulated
//! trajectory, an event stream, a trace hash, or a golden.
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
//! lease clearance `<= threshold_m` adds the whole step to that band alone. The
//! bands are reported in declaration order, which is the deterministic report
//! order, and each carries its stable id.
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

use tangle_model::{
    ClearanceBandId, CompiledClearanceBand, CompiledScenario, ModeTemplateId, PathId,
};

use crate::agent::{AgentId, AgentStore};
use crate::metrics::tick_minimum_clearance_m;
use crate::query::{self, BodyShape};
use crate::swept::SweptBody;
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
    /// The participating bands, in declaration order, with their accumulated
    /// durations.
    bands: Vec<BandAccumulator>,
}

impl OpenOvertake {
    /// Record one observed tick of the interval: the tick that saw it alongside
    /// and the pair's exact clearance over that tick.
    fn observe(&mut self, tick: u64, clearance_m: f64, step: Seconds) {
        self.last_tick = tick;
        for band in &mut self.bands {
            band.accumulate(clearance_m, step);
        }
    }

    /// Convert the closed interval into its observation.
    fn into_observation(self) -> OvertakeObservation {
        OvertakeObservation {
            agent: self.agent,
            partner: self.partner,
            start_tick: self.start_tick,
            end_tick: self.last_tick,
            bands: self
                .bands
                .into_iter()
                .map(|band| ClosePassBandDuration {
                    band: band.id,
                    duration_s: band.seconds,
                })
                .collect(),
        }
    }
}

/// One clearance band's duration within one overtaking interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosePassBandDuration {
    /// The band's stable compiled identifier.
    pub band: ClearanceBandId,
    /// Seconds the passing pair's exact clearance was at or below the band's
    /// threshold while the pair was alongside.
    pub duration_s: f64,
}

/// One completed overtaking interval and the clearance-band durations it
/// accumulated.
///
/// `agent` is the passing body and `partner` the passed body, so the pair is
/// directional and never collapses to a symmetric distance. `bands` is in the
/// scenario's declaration order and omits any band whose `applies_to_modes` gate
/// excludes the pair.
#[derive(Debug, Clone, PartialEq)]
pub struct OvertakeObservation {
    /// The passing agent: the body with the greater speed along the shared
    /// reference.
    pub agent: AgentId,
    /// The passed body.
    pub partner: AgentId,
    /// First observed tick the pair was alongside.
    pub start_tick: u64,
    /// Last observed tick the pair was alongside.
    pub end_tick: u64,
    /// The participating bands in declaration order, each with its accumulated
    /// seconds.
    pub bands: Vec<ClosePassBandDuration>,
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
        tick: u64,
        step: Seconds,
    ) {
        self.build_bodies(agents);
        self.order_bodies();
        self.collect_candidates();
        self.advance_intervals(scenario, tick, step);
    }

    /// Completed overtaking intervals, in close order.
    ///
    /// A pair that is still alongside at the end of the run leaves its interval
    /// open and unrecorded; closing it on termination is the observation
    /// closure's decision, not this tracker's.
    pub fn overtakes(&self) -> &[OvertakeObservation] {
        &self.overtakes
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
    fn advance_intervals(&mut self, scenario: &CompiledScenario, tick: u64, step: Seconds) {
        self.seen.clear();
        for position in 0..self.candidates.len() {
            let (first_index, second_index) = self.candidates[position];
            let first = self.bodies[first_index];
            let second = self.bodies[second_index];
            let key = canonical_pair(first.agent, second.agent);
            self.seen.insert(key);
            let clearance_m = tick_minimum_clearance_m(&first.body, &second.body);
            if let Some(open) = self.open.get_mut(&key) {
                open.observe(tick, clearance_m, step);
                continue;
            }
            let Some((agent, partner)) = passing_pair(&first, &second) else {
                continue;
            };
            let mut open = OpenOvertake {
                agent,
                partner,
                start_tick: tick,
                last_tick: tick,
                bands: participating_bands(scenario, &first, &second),
            };
            open.observe(tick, clearance_m, step);
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
            seconds: 0.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use tangle_model::{CompiledScenario, parse_scenario_source_v2};

    use crate::agent::{AgentInit, AgentMode};

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
            route_state: None,
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
        for index in 0..store.len() {
            if !store.alive[index] {
                continue;
            }
            let direction = store.direction[index];
            let step_m = store.speed_mps[index] * DT * direction;
            store.distance_m[index] += step_m;
            store.position[index].x += step_m;
        }
        tracker.observe(store, scenario, tick, Seconds::from_secs(DT));
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
    fn only_band(observation: &OvertakeObservation) -> ClosePassBandDuration {
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
}
