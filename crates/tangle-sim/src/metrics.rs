//! Online interaction metrics: time to collision, minimum surface separation,
//! and conflict-region post-encroachment time.
//!
//! # Model card
//!
//! `PHASE_1_PLAN.md` Increment 4 ("Online TTC and minimum-separation tracking;
//! conflict-region occupancy intervals for PET") needs the simulator itself to
//! report how close its bodies came to a conflict, rather than a post-hoc
//! analysis of recorded trajectories. This module owns that observation pass:
//! the per-tick computation of time to collision and minimum surface separation
//! over the swept candidate pairs, the conflict-region occupancy intervals, and
//! the post-encroachment time between successive occupancies, together with the
//! run-level running minima a report, a trace, or a viewer reads.
//!
//! It is a pure observer. Every input is borrowed — the agent store, the region
//! edges the tick's safety records carry, and the tick's displacement — nothing
//! here writes agent state, and no metric emits an event. Adding the pass
//! therefore cannot change a simulated trajectory, an event stream, a trace
//! hash, or a golden.
//!
//! ## State
//!
//! One [`InteractionMetrics`] lives in the [`Simulation`](crate::Simulation) and
//! is fed twice per tick: [`InteractionMetrics::begin_tick`] records the
//! tick-start body shapes so the swept bodies span the whole tick, and
//! [`InteractionMetrics::observe`] computes the tick's observations and folds
//! them into the run-level records. Its columns grow with the agent store but
//! are indexed by stable agent slot, and its pair, region, and occupancy maps are
//! ordered, so no path iterates a hash map. Reused buffers keep the pass
//! allocation-free after warmup.
//!
//! ## Candidate set
//!
//! The metrics observe the ordered pairs a swept broad phase
//! ([`SweptBroadPhase`]) returns, with both bodies grown by half
//! [`INTERACTION_RANGE_M`] exactly as [`crate::safety`] grows its band by half
//! [`crate::safety::NEAR_MISS_THRESHOLD_M`]. Growing both bodies by half the
//! range shrinks a pair's surface clearance by the whole range
//! ([`BodyShape::inflated`](crate::query::BodyShape::inflated) documents the
//! containment), so every pair whose surfaces come within the range at some time
//! in the tick is a candidate and none inside the range is missed; the metric
//! tests then read the true, un-grown swept bodies. Pairs are ascending
//! `(AgentId, AgentId)` with `first < second`, so a pair has one canonical
//! spelling and the same pair identifier the collision and near-miss records
//! use. Because [`INTERACTION_RANGE_M`] exceeds the near-miss threshold, the
//! safety pass's candidate set is a subset of this one.
//!
//! A pair that stays outside the range for a whole run therefore has no recorded
//! separation and no recorded time to collision; the range is a declared
//! reporting constant, not a physical property.
//!
//! ## Time to collision
//!
//! [`time_to_collision`] predicts contact from the state the tick produced: the
//! tick-end poses together with the tick's own displacement, which is each
//! body's velocity over one step. Both bodies are extrapolated on straight lines
//! at that velocity and the query reports the first time in seconds at which
//! their closed surfaces are predicted to touch:
//!
//! - `Some(0.0)` when the pair already touches or overlaps at the observed
//!   state: the predicted conflict is now. The overlap itself is reported by
//!   [`Event::Collision`], not by this metric;
//! - `Some(seconds)` when the pair is closing and the predicted contact is
//!   within [`TTC_HORIZON_S`];
//! - `None` when the pair is not closing — receding, parallel, or keeping a
//!   constant separation — or when the predicted contact lies beyond the
//!   reporting horizon or on a path whose closest approach stays clear of
//!   contact.
//!
//! "Closing" is the sign of the signed clearance's rate at the observed state:
//! the relative displacement projected onto the contact normal
//! ([`body_contact_normal`](crate::body_contact_normal)). The last `None` case is
//! what keeps the value honest: a pair on paths that cross but never come within
//! contact of each other reports no time to collision, because the query reads
//! the exact shapes instead of assuming any approaching pair will meet.
//!
//! The first contact fraction is found by bisection. The relative translations at
//! which two convex bodies touch form the Minkowski difference of their shapes, a
//! convex set, so the tick fractions at which the pair touches form a single
//! interval and the bisection locates its first entry however narrow the window.
//!
//! ## Minimum separation
//!
//! [`tick_minimum_clearance_m`] is the least signed surface clearance of a pair
//! over one tick, from the same exact query the geometry layer exposes
//! ([`body_clearance_m`](crate::body_clearance_m)): positive is the exact
//! disjoint distance, negative is `-penetration_depth`, and zero is contact. Both
//! tick endpoints are evaluated and, when the pair closes and then separates
//! inside the tick, the closest approach is located by bisecting the clearance
//! rate. A pair that stays disjoint has a signed clearance that is convex in the
//! tick fraction, so the located turning point is the exact minimum; a pair that
//! touches instead has one interval of contact, and the located turning point is
//! its entry, so a pair that contacts never reports a clear separation. The
//! metric does not report a mid-tick penetration deeper than the contact entry:
//! the contact family ([`Event::Collision`]) carries that.
//!
//! Because the reported value is each tick's own minimum over a swept pair, a
//! run's minimum over its ticks is the trajectory's minimum and a sub-tick
//! approach is not sampled away.
//!
//! ## Post-encroachment time
//!
//! A conflict region is a crossing region or an authored conflict region
//! ([`RegionKey`]), and a body occupies it between the tick that reports its
//! [`Event::Entry`] and the tick that reports its [`Event::Exit`]. Occupancies
//! are read from that existing event state, so this module adds no second
//! occupancy predicate and cannot disagree with [`crate::safety`] about when a
//! body is in a region.
//!
//! An occupancy's entry time is the end of the tick that reported the entry and
//! its exit time the end of the tick that reported the exit, so both boundaries
//! are quantized to the fixed step: the metric reports PET to within a tick, and
//! a finer step converges. A body that despawns while inside closes its occupancy
//! at the despawn's tick, because [`Event::Despawned`] closes the agent's stream.
//!
//! [`PostEncroachment`] is recorded between two successive recorded occupancies
//! of one region when they are by different bodies and do not overlap, with
//! `seconds = following_entry_s - preceding_exit_s`. Two occupancies that overlap
//! in time have no post-encroachment time — the bodies were in the region
//! together, which is a conflict of a different family — so that succession
//! records nothing. A body's re-entry is an occupancy like any other: it forms a
//! succession with the occupancy it follows and again with the occupancy that
//! follows it, and only the different-body successions yield a value.
//! Occupancies are stored in the order they closed, which is tick order and,
//! within one tick, the ascending-[`AgentId`] order the safety pass emits a
//! region's edges in, so the successions, their records, and the minimum are
//! deterministic.
//!
//! ## Units, tolerances, and tie-breaks
//!
//! Times are seconds and separations metres, over `f64`/`glam::DVec2` state. The
//! declared tolerances a caller reads values with are:
//!
//! - [`TTC_TIME_TOLERANCE_S`] is the resolution of a reported time to collision,
//!   and [`SEPARATION_RESOLUTION_M`] the resolution of a reported minimum
//!   separation: both locate a bisected boundary within that much of the true
//!   value, because a pair's clearance can only change by its relative
//!   displacement over one tick;
//! - [`INTERACTION_RANGE_M`] is the relevance range described above;
//! - [`TTC_HORIZON_S`] is the reporting horizon: a closing pair whose predicted
//!   contact is later is reported as not applicable (`None`) rather than as a
//!   large number;
//! - [`CONTACT_EPSILON_M`](crate::CONTACT_EPSILON_M) is the geometric contact
//!   band the clearance query already applies.
//!
//! Every running minimum keeps the first value it saw on a tie, because the
//! update is strictly-less: the run minimum of a metric is the earliest tick that
//! achieved the least value, with the lowest pair order among the pairs of that
//! tick, since candidates are ascending.
//!
//! ## Determinism
//!
//! Candidate pairs come from a [`SweptBroadPhase`], a pure function of the
//! indexed bodies in ascending [`AgentId`] order. Occupancy successions follow
//! the event stream's order and close order, and nothing here draws a random
//! number, reads a clock, or iterates a hash map, so the same scenario and seed
//! report the same metrics.

use std::collections::BTreeMap;

use glam::DVec2;

use crate::agent::{AgentId, AgentMode, AgentStore};
use crate::config::RunConfig;
use crate::event::{Event, RegionKey};
use crate::index::SweptBroadPhase;
use crate::query::{self, BodyShape, body_clearance_m};
use crate::swept::{SweptBody, clearance_rate, first_fraction};
use crate::time::SimTime;
use crate::units::Seconds;

/// Distance in metres within which two bodies are relevant to the online
/// interaction metrics.
///
/// A reporting constant, not a physical property: it is well above the
/// [`NEAR_MISS_THRESHOLD_M`](crate::NEAR_MISS_THRESHOLD_M) band, so a pair that
/// merely passes close is observed with a few seconds of closing motion left,
/// and well below any scenario scale, so a pair that meets by coincidence at the
/// far end of a long path is not reported as an interaction. Pairs whose
/// surfaces stay outside this range for a whole run have no recorded separation
/// and no recorded time to collision.
pub const INTERACTION_RANGE_M: f64 = 20.0;

/// Longest predicted time to contact, in seconds, a reported time to collision
/// may carry.
///
/// A declared reporting horizon: a closing pair whose predicted contact is later
/// is reported as not applicable rather than as a very large number. Five
/// seconds covers every closing motion this kernel produces at the speeds its
/// profiles allow, so the horizon only bounds the slow cases.
pub const TTC_HORIZON_S: f64 = 5.0;

/// Resolution in seconds of a reported time to collision.
///
/// A reported time is within this of the first predicted contact: the bisection
/// stops once its bracket of tick fractions is narrower than this over the fixed
/// step. A microsecond of simulated time is far below any physical scale the
/// metric describes and keeps the per-pair work bounded.
pub const TTC_TIME_TOLERANCE_S: f64 = 1e-6;

/// Resolution in metres of a reported minimum separation.
///
/// The located closest approach is within this of the true minimum: the
/// bisection stops once its bracket of tick fractions is narrower than this over
/// the tick's relative displacement, the rate at which a pair's clearance can
/// change. A micrometre is far below any physical scale a separation report
/// needs.
pub const SEPARATION_RESOLUTION_M: f64 = 1e-6;

/// Which pair of agent modes an interaction is between.
///
/// The cross-mode pair ([`ModePair::VehiclePedestrian`]) is one arm of this
/// table, so a report can separate mixed interaction from same-mode interaction
/// without re-deriving the modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModePair {
    /// Two vehicles.
    VehicleVehicle,
    /// A vehicle and a pedestrian, whichever order the pair is stored in.
    VehiclePedestrian,
    /// Two pedestrians.
    PedestrianPedestrian,
}

impl ModePair {
    /// Number of mode pairs: the length of a per-mode-pair table.
    pub const COUNT: usize = 3;

    /// The mode pair of two agents, whichever order they are given in.
    pub const fn of(first: AgentMode, second: AgentMode) -> Self {
        match (first, second) {
            (AgentMode::Vehicle, AgentMode::Vehicle) => Self::VehicleVehicle,
            (AgentMode::Vehicle, AgentMode::Pedestrian)
            | (AgentMode::Pedestrian, AgentMode::Vehicle) => Self::VehiclePedestrian,
            (AgentMode::Pedestrian, AgentMode::Pedestrian) => Self::PedestrianPedestrian,
        }
    }

    /// Position of this pair in a per-mode-pair table.
    pub const fn index(self) -> usize {
        match self {
            Self::VehicleVehicle => 0,
            Self::VehiclePedestrian => 1,
            Self::PedestrianPedestrian => 2,
        }
    }

    /// Short stable label for reports, traces, and inspectors.
    pub const fn label(self) -> &'static str {
        match self {
            Self::VehicleVehicle => "vehicle_vehicle",
            Self::VehiclePedestrian => "vehicle_pedestrian",
            Self::PedestrianPedestrian => "pedestrian_pedestrian",
        }
    }
}

/// The least value one online interaction metric has taken, with the pair and
/// tick that produced it.
///
/// A time to collision carries seconds in `value` and a separation carries
/// metres; the field keeps its unit suffix at the call site instead, because one
/// record type serves both metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricMinimum {
    /// The metric's value: seconds, or metres for a separation.
    pub value: f64,
    /// Lower [`AgentId`] of the pair, spelled as [`Event::Collision`] spells it.
    pub agent: AgentId,
    /// Higher [`AgentId`] of the pair.
    pub other: AgentId,
    /// The modes of the pair.
    pub mode_pair: ModePair,
    /// Tick the value was observed on, counted from one as
    /// [`SimTime::tick`](crate::SimTime::tick) counts.
    pub tick: u64,
}

/// One body's occupancy of one conflict region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionOccupancy {
    /// The region occupied.
    pub region: RegionKey,
    /// The occupying body.
    pub agent: AgentId,
    /// End of the tick that reported the entry, in simulated seconds.
    pub entry_s: f64,
    /// End of the tick that reported the exit, in simulated seconds.
    pub exit_s: f64,
}

/// The post-encroachment time between two successive occupancies of one region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostEncroachment {
    /// The region both bodies occupied.
    pub region: RegionKey,
    /// The body that left first.
    pub preceding: AgentId,
    /// The body that entered next.
    pub following: AgentId,
    /// When the preceding body left the region, in simulated seconds.
    pub preceding_exit_s: f64,
    /// When the following body entered the region, in simulated seconds.
    pub following_entry_s: f64,
    /// The time between the two, `following_entry_s - preceding_exit_s`.
    pub seconds: f64,
}

/// The first time in seconds beyond the end of the tick at which two bodies'
/// surfaces are predicted to touch, or `None` when no contact is predicted.
///
/// The observed state is the tick end and the velocity is the tick's own
/// displacement per step, so the prediction uses the state the tick integrated.
/// Args are symmetric: swapping the two swept bodies keeps the time. See the
/// module card for the definition, the horizon, and the applicability cases.
pub fn time_to_collision(first: &SweptBody, second: &SweptBody, step: Seconds) -> Option<f64> {
    let step_s = step.as_secs();
    if !step_s.is_finite() || step_s <= 0.0 {
        return None;
    }
    if body_clearance_m(&first.end_shape(), &second.end_shape()) <= 0.0 {
        // Already touching or overlapping at the observed state.
        return Some(0.0);
    }
    // One step of displacement is one step of extrapolation, so the reporting
    // horizon in tick fractions is the horizon over the step.
    let horizon = 1.0 + TTC_HORIZON_S / step_s;
    let relative_m = second.displacement_m - first.displacement_m;
    if !centres_can_touch(first, second, relative_m, horizon) {
        // The bodies' centres never come within the sum of their circumradii,
        // and each body lies inside its circumcircle, so contact is impossible
        // anywhere in the window: no time to collision exists.
        return None;
    }
    if clearance_at(first, second, horizon) <= 0.0 {
        // The pair touches at or before the horizon's end, so the first entry is
        // inside the window: the fractions at which two convex bodies touch are a
        // single interval, and `first_fraction` locates its first entry.
        let fraction = first_fraction(1.0, horizon, ttc_fraction_tolerance(step_s), |fraction| {
            clearance_at(first, second, fraction) <= 0.0
        });
        return Some((fraction - 1.0) * step_s);
    }
    // Clear at both ends of the window, so an entry needs the clearance rate to
    // turn non-negative inside it. The rate is non-decreasing while the pair is
    // disjoint, so its sign is the monotone predicate to bisect.
    if clearance_rate(first, second, relative_m, 1.0) >= 0.0
        || clearance_rate(first, second, relative_m, horizon) < 0.0
    {
        // Either the pair is not closing at the observed state — receding,
        // parallel, or keeping a constant separation — or it is still closing at
        // the horizon's end, so the window's least clearance is its last one,
        // which is clear. Both mean no predicted contact within the horizon.
        return None;
    }
    let tolerance = ttc_fraction_tolerance(step_s);
    let turning = first_fraction(1.0, horizon, tolerance, |fraction| {
        clearance_rate(first, second, relative_m, fraction) >= 0.0
    });
    if clearance_at(first, second, turning) > 0.0 {
        // The closest approach of the window is still clear: the pair closes and
        // separates without their surfaces touching, so no time to collision
        // exists.
        return None;
    }
    let fraction = first_fraction(1.0, turning, tolerance, |fraction| {
        clearance_at(first, second, fraction) <= 0.0
    });
    Some((fraction - 1.0) * step_s)
}

/// The tick-fraction resolution of a reported time to collision: the declared
/// resolution over the step, since one tick of fraction is one step of seconds.
fn ttc_fraction_tolerance(step_s: f64) -> f64 {
    TTC_TIME_TOLERANCE_S / step_s
}

/// Whether two bodies' centres can come within the sum of their circumradii
/// anywhere in `[1, horizon]` of the extrapolation.
///
/// Each body lies inside its circumcircle, so two bodies can touch only while
/// their centres are within the sum of those radii; the condition is therefore
/// necessary for contact and never rules out a contact that exists. The centre
/// offset is linear in the tick fraction, so the squared centre distance is
/// convex and its least value over the window sits at the clamped
/// closest-approach fraction, which costs a handful of dot products rather than
/// a search. That is the fast path for the common case of a pair whose paths
/// pass each other clear of any contact.
fn centres_can_touch(
    first: &SweptBody,
    second: &SweptBody,
    relative_m: DVec2,
    horizon: f64,
) -> bool {
    let offset = second.end_shape().centre() - first.end_shape().centre();
    let speed_squared = relative_m.length_squared();
    let fraction = if speed_squared > 0.0 {
        (-offset.dot(relative_m) / speed_squared).clamp(1.0, horizon)
    } else {
        1.0
    };
    let least_centre_m = (offset + relative_m * fraction).length();
    let reach_m = first.shape.circumradius_m() + second.shape.circumradius_m();
    least_centre_m <= reach_m
}

/// The least signed surface clearance in metres between two swept bodies over
/// the tick.
///
/// Positive is the exact disjoint distance, negative is `-penetration_depth`,
/// and zero is contact, exactly as
/// [`body_clearance_m`](crate::body_clearance_m) reports those. See the module
/// card for what is exact and what a contacting pair reports.
pub fn tick_minimum_clearance_m(first: &SweptBody, second: &SweptBody) -> f64 {
    let start_m = body_clearance_m(&first.shape, &second.shape);
    let end_m = body_clearance_m(&first.end_shape(), &second.end_shape());
    let mut minimum_m = start_m.min(end_m);
    if minimum_m <= 0.0 {
        // An endpoint already contacts, so it bounds the tick's minimum.
        return minimum_m;
    }
    let relative_m = second.displacement_m - first.displacement_m;
    if relative_m.length() <= SEPARATION_RESOLUTION_M {
        // The clearance can change by at most the tick's relative displacement,
        // so an endpoint holds the tick's minimum within the declared
        // resolution. This is the common case of two bodies keeping formation,
        // such as a lane at a constant speed.
        return minimum_m;
    }
    let start_rate = clearance_rate(first, second, relative_m, 0.0);
    let end_rate = clearance_rate(first, second, relative_m, 1.0);
    if start_rate >= 0.0 || end_rate < 0.0 {
        // The clearance rate is non-decreasing over the tick, so a pair that is
        // not closing at the start, or is still closing at the end, has its
        // least clearance at an endpoint.
        return minimum_m;
    }
    // The pair closes and then separates inside the tick, so the least clearance
    // is at the turning point of the rate. The bracket is the whole tick, and the
    // resolution is the declared separation resolution over the rate at which a
    // pair's clearance can change, its relative displacement.
    let tolerance = (SEPARATION_RESOLUTION_M / relative_m.length()).min(1.0);
    let turning = first_fraction(0.0, 1.0, tolerance, |fraction| {
        clearance_rate(first, second, relative_m, fraction) >= 0.0
    });
    minimum_m = minimum_m.min(clearance_at(first, second, turning));
    minimum_m
}

/// The signed clearance in metres between two swept bodies at `fraction` of the
/// tick.
fn clearance_at(first: &SweptBody, second: &SweptBody, fraction: f64) -> f64 {
    body_clearance_m(&first.shape_at(fraction), &second.shape_at(fraction))
}

/// Online interaction metrics for one run.
///
/// Fed once per tick from the integrated state by
/// [`Simulation`](crate::Simulation); read through
/// [`Simulation::interaction_metrics`](crate::Simulation::interaction_metrics).
/// Fields are private: every value has an accessor, and the pass itself is
/// crate-internal, so no caller can inject a metric the tick did not produce.
/// See the module card for the metric definitions and the declared tolerances.
#[derive(Debug)]
pub struct InteractionMetrics {
    /// Body shape of every live agent at the tick start, by slot.
    start_shapes: Vec<Option<BodyShape>>,
    /// Tick-swept body and mode of every live agent, ascending by [`AgentId`].
    indexed: Vec<(AgentId, AgentMode, SweptBody)>,
    /// The same bodies grown by half [`INTERACTION_RANGE_M`], which is what the
    /// candidate grid indexes; see the module candidate-set note.
    inflated: Vec<(AgentId, SweptBody)>,
    /// Swept broad phase over this tick's grown live bodies.
    grid: SweptBroadPhase,
    /// Reused candidate-pair buffer, ascending `(AgentId, AgentId)`.
    pairs: Vec<(AgentId, AgentId)>,
    /// Least recorded separation in metres of each observed pair.
    pair_minimum_separation_m: BTreeMap<(AgentId, AgentId), f64>,
    /// Least recorded time to collision in seconds of each observed closing
    /// pair. A candidate pair that never closes records no entry, so this map is
    /// a subset of the one above; see [`Self::pair_minimum_ttc_s`].
    pair_minimum_ttc_s: BTreeMap<(AgentId, AgentId), f64>,
    /// Least recorded separation in metres of each mode pair.
    mode_pair_minimum_separation_m: [Option<MetricMinimum>; ModePair::COUNT],
    /// Least recorded separation in metres over the run.
    minimum_separation_m: Option<MetricMinimum>,
    /// Least recorded time to collision in seconds over the run.
    minimum_ttc_s: Option<MetricMinimum>,
    /// The last observed tick's least separation in metres.
    tick_minimum_separation_m: Option<MetricMinimum>,
    /// The last observed tick's least time to collision in seconds.
    tick_minimum_ttc_s: Option<MetricMinimum>,
    /// Entry time in seconds of every open occupancy, ascending by key.
    open_occupancies: BTreeMap<(AgentId, RegionKey), f64>,
    /// Completed occupancies of every region, in the order they closed.
    occupancies: BTreeMap<RegionKey, Vec<RegionOccupancy>>,
    /// Every recorded post-encroachment time, in the order it was recorded.
    post_encroachments: Vec<PostEncroachment>,
    /// The least recorded post-encroachment time in seconds.
    minimum_post_encroachment_s: Option<PostEncroachment>,
}

impl Default for InteractionMetrics {
    fn default() -> Self {
        Self {
            start_shapes: Vec::new(),
            indexed: Vec::new(),
            inflated: Vec::new(),
            grid: SweptBroadPhase::default(),
            pairs: Vec::new(),
            pair_minimum_separation_m: BTreeMap::new(),
            pair_minimum_ttc_s: BTreeMap::new(),
            mode_pair_minimum_separation_m: [None; ModePair::COUNT],
            minimum_separation_m: None,
            minimum_ttc_s: None,
            tick_minimum_separation_m: None,
            tick_minimum_ttc_s: None,
            open_occupancies: BTreeMap::new(),
            occupancies: BTreeMap::new(),
            post_encroachments: Vec::new(),
            minimum_post_encroachment_s: None,
        }
    }
}

impl InteractionMetrics {
    /// Least surface separation in metres over the run so far, with the pair
    /// and tick that produced it. `None` before the first observed pair.
    pub fn minimum_separation_m(&self) -> Option<MetricMinimum> {
        self.minimum_separation_m
    }

    /// Least surface separation in metres of one mode pair over the run so far.
    /// `None` until a pair of those modes has been observed.
    pub fn mode_pair_minimum_separation_m(&self, pair: ModePair) -> Option<MetricMinimum> {
        self.mode_pair_minimum_separation_m[pair.index()]
    }

    /// Least recorded surface separation in metres of one observed pair over the
    /// run so far, in either argument order.
    ///
    /// `None` for a pair that has never been a candidate, which includes a pair
    /// whose surfaces never came within [`INTERACTION_RANGE_M`].
    pub fn pair_minimum_separation_m(&self, agent: AgentId, other: AgentId) -> Option<f64> {
        let key = if agent <= other {
            (agent, other)
        } else {
            (other, agent)
        };
        self.pair_minimum_separation_m.get(&key).copied()
    }

    /// Least time to collision in seconds over the run so far. `None` until a
    /// closing pair has been observed.
    pub fn minimum_ttc_s(&self) -> Option<MetricMinimum> {
        self.minimum_ttc_s
    }

    /// Least recorded time to collision in seconds of one observed pair over
    /// the run so far, in either argument order.
    ///
    /// Mirrors [`Self::pair_minimum_separation_m`]: the pair set is the same
    /// swept candidate set and the minimum keeps the first value on a tie. The
    /// applicability differs, because a time to collision is not defined for
    /// every candidate pair: `None` for a pair that has never been a candidate,
    /// whose surfaces never came within [`INTERACTION_RANGE_M`], and for a
    /// candidate pair that never closed with a predicted contact inside
    /// [`TTC_HORIZON_S`], even though such a pair still records a separation.
    pub fn pair_minimum_ttc_s(&self, agent: AgentId, other: AgentId) -> Option<f64> {
        let key = if agent <= other {
            (agent, other)
        } else {
            (other, agent)
        };
        self.pair_minimum_ttc_s.get(&key).copied()
    }

    /// The last observed tick's least surface separation in metres.
    pub fn tick_minimum_separation_m(&self) -> Option<MetricMinimum> {
        self.tick_minimum_separation_m
    }

    /// The last observed tick's least time to collision in seconds.
    pub fn tick_minimum_ttc_s(&self) -> Option<MetricMinimum> {
        self.tick_minimum_ttc_s
    }

    /// Every recorded post-encroachment time, in the order it was recorded.
    pub fn post_encroachments(&self) -> &[PostEncroachment] {
        &self.post_encroachments
    }

    /// The least recorded post-encroachment time in seconds.
    pub fn minimum_post_encroachment_s(&self) -> Option<PostEncroachment> {
        self.minimum_post_encroachment_s
    }

    /// Completed occupancies of one region, in the order they closed.
    pub fn region_occupancies(&self, region: RegionKey) -> &[RegionOccupancy] {
        self.occupancies.get(&region).map_or(&[], Vec::as_slice)
    }

    /// Record every live body's tick-start shape.
    ///
    /// Call once before the state-affecting loop, so the swept bodies this pass
    /// builds carry the whole tick's displacement.
    pub(crate) fn begin_tick(&mut self, agents: &AgentStore) {
        self.start_shapes.clear();
        self.start_shapes.resize(agents.len(), None);
        for index in 0..agents.len() {
            if agents.alive[index] {
                self.start_shapes[index] = Some(query::agent_body(agents, index));
            }
        }
    }

    /// Observe one integrated tick: the candidate pairs' metrics, then the
    /// region occupancy edges the tick's events report.
    ///
    /// Call once per tick, after the bodies have stepped and before new demand is
    /// admitted, so the bodies observed are exactly the ones the tick
    /// integrated. Within one tick the safety pass emits a region's occupancy
    /// edges in ascending [`AgentId`] order, which is the order the
    /// post-encroachment succession reads them in. The pass borrows everything
    /// it reads and writes only its own records.
    pub(crate) fn observe(
        &mut self,
        agents: &AgentStore,
        tick: u64,
        config: RunConfig,
        events: &[Event],
    ) {
        self.tick_minimum_separation_m = None;
        self.tick_minimum_ttc_s = None;
        self.index_bodies(agents);
        self.scan_pairs(tick, config.step());
        self.scan_events(SimTime::from_tick(tick, config.step()).seconds(), events);
    }

    /// Rebuild the tick-swept bodies and the candidate grid.
    ///
    /// The grid indexes each body grown by half [`INTERACTION_RANGE_M`], so a
    /// candidate pair is every pair whose surfaces could have come within the
    /// range at some time in the tick; see the module candidate-set note.
    fn index_bodies(&mut self, agents: &AgentStore) {
        self.indexed.clear();
        self.inflated.clear();
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            let end = query::agent_body(agents, index);
            let start = self.start_shapes[index].unwrap_or(end);
            let body = SweptBody {
                shape: start,
                displacement_m: end.centre() - start.centre(),
            };
            let agent = AgentId::from_index(index);
            self.indexed.push((agent, agents.mode[index], body));
            self.inflated.push((
                agent,
                SweptBody {
                    shape: start.inflated(INTERACTION_RANGE_M * 0.5),
                    displacement_m: body.displacement_m,
                },
            ));
        }
        self.grid.rebuild(&self.inflated);
    }

    /// The tick-swept body and mode of one live agent.
    fn body_of(&self, agent: AgentId) -> (AgentMode, SweptBody) {
        let position = self
            .indexed
            .binary_search_by_key(&agent, |(id, _, _)| *id)
            .expect("metrics state refers only to indexed live agents");
        let (_, mode, body) = self.indexed[position];
        (mode, body)
    }

    /// Record the metrics of every candidate pair of this tick.
    fn scan_pairs(&mut self, tick: u64, step: Seconds) {
        self.grid.candidate_pairs(&mut self.pairs);
        for position in 0..self.pairs.len() {
            let (first, second) = self.pairs[position];
            let (first_mode, first_body) = self.body_of(first);
            let (second_mode, second_body) = self.body_of(second);
            let mode_pair = ModePair::of(first_mode, second_mode);
            let separation_m = tick_minimum_clearance_m(&first_body, &second_body);
            let recorded = self
                .pair_minimum_separation_m
                .entry((first, second))
                .or_insert(separation_m);
            if separation_m < *recorded {
                *recorded = separation_m;
            }
            let separation = MetricMinimum {
                value: separation_m,
                agent: first,
                other: second,
                mode_pair,
                tick,
            };
            keep_minimum(&mut self.tick_minimum_separation_m, separation);
            keep_minimum(
                &mut self.mode_pair_minimum_separation_m[mode_pair.index()],
                separation,
            );
            keep_minimum(&mut self.minimum_separation_m, separation);
            if let Some(seconds) = time_to_collision(&first_body, &second_body, step) {
                let recorded = self
                    .pair_minimum_ttc_s
                    .entry((first, second))
                    .or_insert(seconds);
                if seconds < *recorded {
                    *recorded = seconds;
                }
                let ttc = MetricMinimum {
                    value: seconds,
                    ..separation
                };
                keep_minimum(&mut self.tick_minimum_ttc_s, ttc);
                keep_minimum(&mut self.minimum_ttc_s, ttc);
            }
        }
    }

    /// Read this tick's region occupancy edges from its sorted event stream.
    ///
    /// A despawn closes the agent's open occupancies, because
    /// [`Event::Despawned`] closes its stream.
    fn scan_events(&mut self, time_s: f64, events: &[Event]) {
        for event in events {
            match *event {
                Event::Entry { agent, region } => {
                    self.open_occupancies.insert((agent, region), time_s);
                }
                Event::Exit { agent, region } => {
                    self.close_occupancy(agent, region, time_s);
                }
                Event::Despawned { agent, .. } => {
                    self.close_agent_occupancies(agent, time_s);
                }
                _ => {}
            }
        }
    }

    /// Complete one open occupancy and record the post-encroachment time it
    /// forms with the occupancy it follows, if any.
    fn close_occupancy(&mut self, agent: AgentId, region: RegionKey, time_s: f64) {
        let Some(entry_s) = self.open_occupancies.remove(&(agent, region)) else {
            return;
        };
        let occupancy = RegionOccupancy {
            region,
            agent,
            entry_s,
            exit_s: time_s,
        };
        let occupancies = self.occupancies.entry(region).or_default();
        let preceding = occupancies.last().copied();
        occupancies.push(occupancy);
        let Some(preceding) = preceding else {
            return;
        };
        if preceding.agent == occupancy.agent || preceding.exit_s > occupancy.entry_s {
            // Occupancies by the same body, or occupancies that overlap in time,
            // have no post-encroachment time; see the module card.
            return;
        }
        let pet = PostEncroachment {
            region,
            preceding: preceding.agent,
            following: occupancy.agent,
            preceding_exit_s: preceding.exit_s,
            following_entry_s: occupancy.entry_s,
            seconds: occupancy.entry_s - preceding.exit_s,
        };
        self.post_encroachments.push(pet);
        if self
            .minimum_post_encroachment_s
            .is_none_or(|current| pet.seconds < current.seconds)
        {
            self.minimum_post_encroachment_s = Some(pet);
        }
    }

    /// Close every occupancy one agent holds, in ascending region order.
    fn close_agent_occupancies(&mut self, agent: AgentId, time_s: f64) {
        let regions: Vec<RegionKey> = self
            .open_occupancies
            .keys()
            .filter(|(open_agent, _)| *open_agent == agent)
            .map(|(_, region)| *region)
            .collect();
        for region in regions {
            self.close_occupancy(agent, region, time_s);
        }
    }
}

/// Keep the least value seen, preferring the first on a tie.
fn keep_minimum(slot: &mut Option<MetricMinimum>, candidate: MetricMinimum) {
    if slot.is_none_or(|current| candidate.value < current.value) {
        *slot = Some(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentInit;
    use tangle_model::{CompiledScenario, CrossingId, PathId, parse_scenario_source};

    /// The declared resolution of a reported time to collision, in seconds.
    const TIME_TOLERANCE_S: f64 = TTC_TIME_TOLERANCE_S;
    /// The declared resolution of a reported separation, in metres.
    const SEPARATION_TOLERANCE_M: f64 = SEPARATION_RESOLUTION_M;

    fn circle(x: f64, y: f64, radius_m: f64) -> BodyShape {
        BodyShape::Circle {
            centre: DVec2::new(x, y),
            radius_m,
        }
    }

    fn swept(shape: BodyShape, displacement_m: DVec2) -> SweptBody {
        SweptBody {
            shape,
            displacement_m,
        }
    }

    fn still(shape: BodyShape) -> SweptBody {
        swept(shape, DVec2::ZERO)
    }

    /// A pair of unit-radius circles on a crossing course: the first runs from
    /// `(-4, 0)` and the second from `(0, -4)`, each moving one metre along its
    /// own axis, so after the next second they sit at `(-3, 0)` and `(0, -3)`.
    ///
    /// The extrapolated surfaces touch when `sqrt(2) * (3 - t) = 1`, at
    /// `3 - 1/sqrt(2)` seconds, which the fixture below asserts directly.
    fn crossing_circles() -> (SweptBody, SweptBody) {
        (
            swept(circle(-4.0, 0.0, 0.5), DVec2::new(1.0, 0.0)),
            swept(circle(0.0, -4.0, 0.5), DVec2::new(0.0, 1.0)),
        )
    }

    #[test]
    fn time_to_collision_is_the_analytic_circle_crossing() {
        let (first, second) = crossing_circles();
        let expected = 3.0 - std::f64::consts::FRAC_1_SQRT_2;
        let seconds = time_to_collision(&first, &second, Seconds::from_secs(1.0))
            .expect("the crossing circles are predicted to touch");
        assert!(
            (seconds - expected).abs() <= TIME_TOLERANCE_S,
            "time to collision {seconds} not {expected}"
        );
        // The query is symmetric in its arguments.
        let mirrored =
            time_to_collision(&second, &first, Seconds::from_secs(1.0)).expect("touching");
        assert!((mirrored - seconds).abs() <= TIME_TOLERANCE_S);
    }

    #[test]
    fn time_to_collision_scales_with_the_step_and_not_the_units() {
        // The same physical state observed at a coarser and a finer step: each
        // body moves a quarter of the distance in a quarter of the time, so the
        // predicted time in seconds is unchanged. Both fixtures place the two
        // bodies at `(-3, 0)` and `(0, -3)` at the observed state.
        let coarse = time_to_collision(
            &swept(circle(-4.0, 0.0, 0.5), DVec2::new(1.0, 0.0)),
            &swept(circle(0.0, -4.0, 0.5), DVec2::new(0.0, 1.0)),
            Seconds::from_secs(1.0),
        )
        .expect("a closing pair");
        let fine = time_to_collision(
            &swept(circle(-3.25, 0.0, 0.5), DVec2::new(0.25, 0.0)),
            &swept(circle(0.0, -3.25, 0.5), DVec2::new(0.0, 0.25)),
            Seconds::from_secs(0.25),
        )
        .expect("a closing pair");
        assert!(
            (coarse - fine).abs() <= TIME_TOLERANCE_S,
            "{coarse} vs {fine}"
        );
    }

    #[test]
    fn time_to_collision_reports_touching_as_zero() {
        let still_body = still(circle(0.0, 0.0, 1.0));
        let step = Seconds::from_secs(1.0);
        // Exactly touching at the observed state, and overlapping at it.
        assert_eq!(
            time_to_collision(&still_body, &still(circle(0.0, 2.0, 1.0)), step),
            Some(0.0)
        );
        assert_eq!(
            time_to_collision(&still_body, &still(circle(0.0, 1.5, 1.0)), step),
            Some(0.0)
        );
    }

    #[test]
    fn time_to_collision_is_absent_for_motion_that_never_contacts() {
        let still_body = still(circle(0.0, 0.0, 1.0));
        let step = Seconds::from_secs(1.0);
        // Parallel motion keeps the gap.
        assert_eq!(
            time_to_collision(
                &swept(circle(0.0, 0.0, 1.0), DVec2::new(1.0, 0.0)),
                &swept(circle(4.0, 0.0, 1.0), DVec2::new(1.0, 0.0)),
                step
            ),
            None
        );
        // Motion perpendicular to the line of centres is a constant separation.
        assert_eq!(
            time_to_collision(
                &still_body,
                &swept(circle(-4.0, 4.0, 1.0), DVec2::new(1.0, 0.0)),
                step
            ),
            None
        );
        // Receding.
        assert_eq!(
            time_to_collision(
                &still_body,
                &swept(circle(3.0, 0.0, 1.0), DVec2::new(1.0, 0.0)),
                step
            ),
            None
        );
        // Closing, but the pass-by keeps 2 m of clearance: the exact shapes say
        // the paths never touch, so no time to collision is reported.
        assert_eq!(
            time_to_collision(
                &still_body,
                &swept(circle(-2.0, 4.0, 1.0), DVec2::new(1.0, 0.0)),
                step
            ),
            None
        );
        // Closing, but the predicted contact is beyond the horizon: an 11 m gap
        // closed at 1 m/s is 11 s against a 5 s horizon.
        assert_eq!(
            time_to_collision(
                &still_body,
                &swept(circle(0.0, 14.0, 1.0), DVec2::new(0.0, -1.0)),
                step
            ),
            None
        );
        // The same closing motion with 5 m of gap is inside the horizon.
        let seconds = time_to_collision(
            &still_body,
            &swept(circle(0.0, 5.0, 1.0), DVec2::new(0.0, -1.0)),
            step,
        )
        .expect("a 2 m clearance at 1 m/s is inside the horizon");
        assert!((seconds - 2.0).abs() <= TIME_TOLERANCE_S, "{seconds}");
    }

    #[test]
    fn tick_minimum_separation_locates_the_closest_approach() {
        // A still unit circle at the origin and a 6 m crossing pass at y = 3:
        // the centre of the mover is exactly above the origin at half the tick,
        // so the least clearance is the 3 m offset minus both radii.
        let still_body = still(circle(0.0, 0.0, 1.0));
        let passer = swept(circle(-3.0, 3.0, 1.0), DVec2::new(6.0, 0.0));
        let minimum_m = tick_minimum_clearance_m(&still_body, &passer);
        assert!(
            (minimum_m - 1.0).abs() <= SEPARATION_TOLERANCE_M,
            "closest approach {minimum_m}"
        );
        // Both endpoints are further away, so the located approach is the
        // tick's own minimum and not an endpoint.
        let endpoint_m = body_clearance_m(&still_body.shape, &passer.shape);
        assert!(endpoint_m > minimum_m + 1.0, "endpoint {endpoint_m}");
        // The query is symmetric in its arguments.
        assert!((tick_minimum_clearance_m(&passer, &still_body) - minimum_m).abs() <= 1e-12);
    }

    #[test]
    fn tick_minimum_separation_floors_at_a_contact() {
        // A circle driven straight through a still circle: the bodies overlap for
        // part of the tick, so the reported minimum is at most contact and never
        // a clear separation.
        let still_body = still(circle(0.0, 0.0, 1.0));
        let through = swept(circle(-5.0, 0.0, 1.0), DVec2::new(10.0, 0.0));
        let minimum_m = tick_minimum_clearance_m(&still_body, &through);
        assert!(
            minimum_m <= crate::CONTACT_EPSILON_M,
            "a contacting pair must not report a clear separation: {minimum_m}"
        );
    }

    #[test]
    fn tick_minimum_separation_keeps_a_monotone_pair_at_an_endpoint() {
        // Approaching without contact: the least clearance is the tick end.
        let still_body = still(circle(0.0, 0.0, 1.0));
        let approacher = swept(circle(-6.0, 0.0, 1.0), DVec2::new(1.0, 0.0));
        assert!((tick_minimum_clearance_m(&still_body, &approacher) - 3.0).abs() <= 1e-12);
        // Separating from a clear start: the least clearance is the tick start.
        let leaving = swept(circle(0.0, 0.0, 1.0), DVec2::new(-1.0, 0.0));
        let target = still(circle(4.0, 0.0, 1.0));
        assert!((tick_minimum_clearance_m(&leaving, &target) - 2.0).abs() <= 1e-12);
    }

    /// Append one circle body of radius `radius_m` at `position`.
    fn push_pedestrian(store: &mut AgentStore, position: DVec2, radius_m: f64) -> AgentId {
        store.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path: PathId::from_index(0),
            distance_m: 0.0,
            speed_mps: 0.0,
            position,
            heading_rad: 0.0,
            body_length_m: radius_m * 2.0,
            body_width_m: radius_m * 2.0,
            direction: 1.0,
            movement: None,
            profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
        })
    }

    /// A scenario with no region at all, so the event side of the pass stays
    /// empty.
    fn no_region_scenario() -> CompiledScenario {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'metrics', coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ], \
             population: { vehicle_count: 0, vehicle_speed_mps: 1.0, vehicle_spacing_m: 5.0, \
             vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    /// The tick-swept body a pair of fixtures indexes, grown as the pass grows
    /// it so the candidate grid pairs the two bodies.
    fn grown(body: SweptBody) -> SweptBody {
        SweptBody {
            shape: body.shape.inflated(INTERACTION_RANGE_M * 0.5),
            displacement_m: body.displacement_m,
        }
    }

    /// Observe one tick of two hand-built bodies through the candidate grid.
    fn observe_pair(
        metrics: &mut InteractionMetrics,
        first: (AgentId, AgentMode, SweptBody),
        second: (AgentId, AgentMode, SweptBody),
        tick: u64,
        step: Seconds,
    ) {
        metrics.indexed = vec![first, second];
        metrics.inflated = vec![(first.0, grown(first.2)), (second.0, grown(second.2))];
        metrics.grid.rebuild(&metrics.inflated);
        metrics.scan_pairs(tick, step);
    }

    #[test]
    fn the_pass_records_ticks_pairs_and_mode_pairs() {
        let step = Seconds::from_secs(1.0);
        let mut metrics = InteractionMetrics::default();
        let first = AgentId::from_index(0);
        let second = AgentId::from_index(1);
        observe_pair(
            &mut metrics,
            (
                first,
                AgentMode::Pedestrian,
                swept(circle(-4.0, 0.0, 0.5), DVec2::new(1.0, 0.0)),
            ),
            (
                second,
                AgentMode::Pedestrian,
                swept(circle(0.0, -4.0, 0.5), DVec2::new(0.0, 1.0)),
            ),
            1,
            step,
        );
        let separation = metrics
            .tick_minimum_separation_m()
            .expect("a candidate pair always reports a separation");
        assert_eq!(separation.agent, first);
        assert_eq!(separation.other, second);
        assert_eq!(separation.mode_pair, ModePair::PedestrianPedestrian);
        assert_eq!(separation.tick, 1);
        let ttc = metrics
            .tick_minimum_ttc_s()
            .expect("the crossing pair is closing");
        assert_eq!(ttc.agent, first);
        assert_eq!(ttc.tick, 1);
        // The run-level records mirror the tick's least values.
        assert_eq!(metrics.minimum_separation_m(), Some(separation));
        assert_eq!(metrics.minimum_ttc_s(), Some(ttc));
        assert_eq!(
            metrics.mode_pair_minimum_separation_m(ModePair::PedestrianPedestrian),
            Some(separation)
        );
        assert_eq!(
            metrics.mode_pair_minimum_separation_m(ModePair::VehiclePedestrian),
            None
        );
        assert_eq!(
            metrics.pair_minimum_separation_m(second, first),
            Some(separation.value)
        );
        assert_eq!(
            metrics.pair_minimum_separation_m(first, AgentId::from_index(9)),
            None
        );
    }

    /// The per-pair time to collision mirrors the per-pair separation
    /// accessor's pair set and symmetry, but only records a value for a closing
    /// pair; a candidate pair that never closes records a separation and no
    /// time to collision.
    #[test]
    fn the_pass_records_each_observed_pairs_least_time_to_collision() {
        let step = Seconds::from_secs(1.0);
        let first = AgentId::from_index(0);
        let second = AgentId::from_index(1);
        let mut crossing = InteractionMetrics::default();
        observe_pair(
            &mut crossing,
            (
                first,
                AgentMode::Pedestrian,
                swept(circle(-4.0, 0.0, 0.5), DVec2::new(1.0, 0.0)),
            ),
            (
                second,
                AgentMode::Pedestrian,
                swept(circle(0.0, -4.0, 0.5), DVec2::new(0.0, 1.0)),
            ),
            1,
            step,
        );
        let expected = 3.0 - std::f64::consts::FRAC_1_SQRT_2;
        let seconds = crossing
            .pair_minimum_ttc_s(first, second)
            .expect("the crossing pair is closing");
        assert!((seconds - expected).abs() <= TIME_TOLERANCE_S);
        // Symmetric in its arguments, as the separation accessor is.
        assert_eq!(crossing.pair_minimum_ttc_s(second, first), Some(seconds));
        // A pair that never came within the candidate range has no value.
        assert_eq!(
            crossing.pair_minimum_ttc_s(first, AgentId::from_index(9)),
            None
        );

        // A candidate pair in parallel motion is recorded as a separation but
        // never as a time to collision.
        let mut parallel = InteractionMetrics::default();
        observe_pair(
            &mut parallel,
            (
                first,
                AgentMode::Vehicle,
                swept(circle(0.0, 0.0, 1.0), DVec2::new(1.0, 0.0)),
            ),
            (
                second,
                AgentMode::Vehicle,
                swept(circle(4.0, 0.0, 1.0), DVec2::new(1.0, 0.0)),
            ),
            1,
            step,
        );
        assert_eq!(parallel.pair_minimum_ttc_s(first, second), None);
        assert_eq!(parallel.pair_minimum_separation_m(first, second), Some(2.0));
    }

    #[test]
    fn the_candidate_grid_covers_every_pair_inside_the_range() {
        // Two bodies 10 m apart are a candidate pair, so their exact separation
        // is recorded; a pair 80 m apart is outside the declared range and is
        // not. The scenario argument is only the store's shape; the region
        // events are empty, so no PET can be recorded here.
        let _ = no_region_scenario();
        let mut store = AgentStore::default();
        let near = push_pedestrian(&mut store, DVec2::new(0.0, 0.0), 0.5);
        let nearer = push_pedestrian(&mut store, DVec2::new(10.0, 0.0), 0.5);
        let far = push_pedestrian(&mut store, DVec2::new(80.0, 0.0), 0.5);
        let mut metrics = InteractionMetrics::default();
        metrics.begin_tick(&store);
        metrics.observe(&store, 1, RunConfig::new(0), &[]);
        assert_eq!(metrics.pair_minimum_separation_m(near, nearer), Some(9.0));
        assert_eq!(metrics.pair_minimum_separation_m(near, far), None);
        assert_eq!(metrics.pair_minimum_separation_m(nearer, far), None);
    }

    /// One entry event, for the PET fixtures.
    const fn entry(agent: AgentId, region: RegionKey) -> Event {
        Event::Entry { agent, region }
    }

    /// One exit event, for the PET fixtures.
    const fn exit(agent: AgentId, region: RegionKey) -> Event {
        Event::Exit { agent, region }
    }

    /// The conflict region the PET fixtures occupy.
    fn region() -> RegionKey {
        RegionKey::ConflictRegion(tangle_model::ConflictRegionId::from_index(0))
    }

    #[test]
    fn successive_occupancies_by_different_bodies_record_one_pet() {
        let region = region();
        let first = AgentId::from_index(0);
        let second = AgentId::from_index(1);
        let mut metrics = InteractionMetrics::default();
        // Body 0 occupies [1, 2] and body 1 [3, 4]: a one-second
        // post-encroachment time.
        metrics.scan_events(1.0, &[entry(first, region)]);
        metrics.scan_events(2.0, &[exit(first, region)]);
        metrics.scan_events(3.0, &[entry(second, region)]);
        metrics.scan_events(4.0, &[exit(second, region)]);
        assert_eq!(
            metrics.post_encroachments(),
            &[PostEncroachment {
                region,
                preceding: first,
                following: second,
                preceding_exit_s: 2.0,
                following_entry_s: 3.0,
                seconds: 1.0,
            }]
        );
        assert_eq!(
            metrics.minimum_post_encroachment_s().map(|pet| pet.seconds),
            Some(1.0)
        );
        assert_eq!(
            metrics.region_occupancies(region),
            &[
                RegionOccupancy {
                    region,
                    agent: first,
                    entry_s: 1.0,
                    exit_s: 2.0,
                },
                RegionOccupancy {
                    region,
                    agent: second,
                    entry_s: 3.0,
                    exit_s: 4.0,
                },
            ]
        );
        assert_eq!(
            metrics.region_occupancies(RegionKey::Crossing(CrossingId::from_index(0))),
            &[]
        );
    }

    #[test]
    fn same_body_re_entry_and_overlapping_successions_record_no_pet() {
        let region = region();
        let first = AgentId::from_index(0);
        let second = AgentId::from_index(1);
        let mut metrics = InteractionMetrics::default();
        // Body 0 enters twice in a row: a succession by the same body is not a
        // post-encroachment time, and body 1's later occupancy is measured
        // against the latest occupancy of body 0.
        metrics.scan_events(1.0, &[entry(first, region)]);
        metrics.scan_events(2.0, &[exit(first, region)]);
        metrics.scan_events(3.0, &[entry(first, region)]);
        metrics.scan_events(4.0, &[exit(first, region)]);
        assert!(metrics.post_encroachments().is_empty());
        metrics.scan_events(5.0, &[entry(second, region)]);
        metrics.scan_events(6.0, &[exit(second, region)]);
        assert_eq!(
            metrics.minimum_post_encroachment_s().map(|pet| pet.seconds),
            Some(1.0),
            "the re-entry is measured against the occupancy that follows it"
        );

        // Body 0 enters while body 1 still occupies the region and leaves after
        // it, so the successions of the completed occupancies overlap in time:
        // no post-encroachment time is defined for the overlap.
        let mut overlapping = InteractionMetrics::default();
        overlapping.scan_events(1.0, &[entry(second, region)]);
        overlapping.scan_events(2.0, &[entry(first, region)]);
        overlapping.scan_events(3.0, &[exit(second, region)]);
        overlapping.scan_events(4.0, &[exit(first, region)]);
        assert!(overlapping.post_encroachments().is_empty());
        assert_eq!(
            overlapping.region_occupancies(region).len(),
            2,
            "both overlapping occupancies are still recorded"
        );
    }

    #[test]
    fn a_despawn_closes_an_open_occupancy() {
        let region = region();
        let agent = AgentId::from_index(0);
        let mut metrics = InteractionMetrics::default();
        metrics.scan_events(1.0, &[entry(agent, region)]);
        metrics.scan_events(
            2.0,
            &[Event::Despawned {
                agent,
                path: PathId::from_index(0),
                reason: crate::DespawnReason::ExitedPath,
            }],
        );
        assert_eq!(
            metrics.region_occupancies(region),
            &[RegionOccupancy {
                region,
                agent,
                entry_s: 1.0,
                exit_s: 2.0,
            }]
        );
    }
}
