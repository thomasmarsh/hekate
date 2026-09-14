//! Online metrics: the interaction metrics of metric definition v1 (time to
//! collision, minimum surface separation, and conflict-region post-encroachment
//! time) and the operational metrics metric definition v2 adds (throughput,
//! delay, and queues).
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
//! ## Operational metrics
//!
//! Metric definition v2 adds three operational families. They are derived from
//! the same per-tick event stream the safety pass produces and from the live
//! agent store, so they add no second predicate about stopped, waiting, or
//! served state and cannot disagree with the records they read:
//!
//! - **Throughput**: `throughput_agents_per_s = served_agents / elapsed_s`,
//!   the agents whose [`Event::Despawned`] completed a trip inside the run per
//!   simulated second. `DespawnReason::ExitedPath` is the only despawn reason
//!   the kernel reports, so every despawn is a completed trip.
//! - **Travel time**: `travel_time_s(agent) = despawn_s - spawn_s`, the
//!   simulated time a served agent's trip took. A bucket reports the mean and
//!   the total over its served agents.
//! - **Stopped delay**: the seconds a served agent spent in a stopped state,
//!   the predicate [`Event::Queue`] reports (speed at or below
//!   [`QUEUE_STOP_SPEED_MPS`](crate::QUEUE_STOP_SPEED_MPS)): a stopped state
//!   opens on the joining record and closes on the departing record. A bucket
//!   reports the mean and the total over its served agents.
//! - **Control delay**: the seconds a served agent spent with a recorded
//!   waiting controller state active, the predicate [`Event::ControlTransition`]
//!   reports. The state follows the agent's own signal-compliance decision, so it
//!   begins when that decision turns to wait — while the body is still braking —
//!   and ends when the decision turns away: it is the time the control device
//!   held the agent, not a subset of the stopped delay. A bucket reports the
//!   mean and the total over its served agents.
//! - **Queue length**: the number of the bucket's agents standing at one tick
//!   end. The bucket reports the most it ever held.
//! - **Queue duration**: `depart_s - join_s` of one stopped state. The bucket
//!   reports the longest and the mean of the states that closed inside the run;
//!   a state still open at the end of the run is not a duration, exactly as an
//!   open region occupancy is not a post-encroachment time.
//!
//! Spawn time is the end of the tick that admitted the agent, which this pass
//! reads from the agent store rather than from a second hook: an agent live at
//! the start of tick `t` was admitted at the end of tick `t - 1`, and the
//! initial population is live at tick zero. Demand admission therefore stays
//! untouched, and its [`Event::Spawned`] records need not be observed at all.
//! A despawn closes the agent's stream, so a state still open then ends at the
//! despawn; the closing record the safety pass emits for a despawned agent
//! carries the same tick, so the two agree exactly rather than double-counting.
//!
//! ### Disaggregation
//!
//! The operational families are observed per agent and reported at three
//! levels: over the whole run, per [`AgentMode`] (`vehicle`, `pedestrian`), and
//! per movement — the [`MovementKey`] tagged union of `MovementId` for vehicles
//! and `PedestrianRouteId` for pedestrians that metric definition v1 chose for
//! the interaction metrics. A key is the scenario's dense identifier; resolving
//! it to the authored name is the output layer's step, as it is for the v1
//! movement buckets. An agent with no assigned movement (the initial static
//! population) contributes to the run-level and mode buckets and to no movement
//! bucket, matching v1.
//!
//! ### Statuses and tie-breaks
//!
//! Every operational value carries the v1 statuses, and a reported zero is a
//! value rather than a marker for an absent one:
//!
//! - **not observed** — the bucket made no observation. For a rate or a length
//!   that is a bucket that never held a live agent; for a delay or a queue
//!   duration it is a bucket that served no agent, or closed no stopped state.
//! - **not applicable** — the predicate that would define the value is false:
//!   a throughput has none before the run has elapsed any time, and no
//!   operational metric is applicable without elapsed time to divide by.
//! - **reported** — the value holds, including the true zero of a bucket whose
//!   served agents were never stopped, never waited, or never queued.
//!
//! A rate, a sum, or a mean selects nothing, so no tie-break applies: the sums
//! accumulate in tick order and, within a tick, in ascending [`AgentId`] order,
//! which makes the floating-point total a property of the run and not of an
//! iteration order. The two maximum statistics select a record, and both update
//! on a strictly-greater comparison, so a tie keeps the first: the earliest
//! tick that held a queue length, and the earliest closed stopped state, which
//! within one tick is the lowest [`AgentId`].
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
use tangle_model::{MovementId, PedestrianRouteId};

use crate::agent::{AgentId, AgentMode, AgentStore};
use crate::config::RunConfig;
use crate::event::{ControlTransitionKind, Event, RegionKey};
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
    /// The operational metrics of metric definition v2: throughput, delay, and
    /// queues, observed from the same tick and the same event records.
    operation: OperationMetrics,
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
            operation: OperationMetrics::default(),
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

    /// The operational metrics of the run so far: throughput, delay, and
    /// queues, at the run level and per mode and movement.
    ///
    /// Observed by the same pass, from the same tick, and against the same
    /// event records as the interaction metrics. See [`OperationMetrics`] and
    /// the module card for the formulas, units, statuses, and tie-breaks.
    pub fn operation(&self) -> &OperationMetrics {
        &self.operation
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
        self.operation.observe(agents, tick, config.step(), events);
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

/// The movement identity of one agent for the operational metrics: the tagged
/// union of a vehicle's [`MovementId`] and a pedestrian's
/// [`PedestrianRouteId`] that metric definition v1 chose and v2 carries
/// forward.
///
/// The two identifiers live in separate id spaces, so the variant tag is part
/// of the key's stable order. The dense identifier is all the kernel needs;
/// the output layer resolves it to the scenario's authored name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MovementKey {
    /// A vehicle movement connector.
    Vehicle(MovementId),
    /// A pedestrian route.
    Pedestrian(PedestrianRouteId),
}

/// The most standing agents one disaggregation bucket held at one observed tick
/// end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueLength {
    /// Standing agents at that tick end.
    pub agents: u64,
    /// The completed tick the count was observed on, counted from one as
    /// [`SimTime::tick`](crate::SimTime::tick) counts.
    pub tick: u64,
}

/// One agent's stopped-and-waiting interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueueDuration {
    /// The agent that stood.
    pub agent: AgentId,
    /// Seconds from the joining record to the departing record.
    pub seconds: f64,
    /// The completed tick whose departing record closed the interval.
    pub tick: u64,
}

/// The operational values of one disaggregation bucket, in metric definition
/// v2's units.
///
/// `None` marks a value the bucket has no observation for; a reported zero is
/// the value `0.0` or `0`, never a marker for an absent one. The status rule
/// each `None` stands for is in the field's own doc and in the module card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperationValues {
    /// End of the last observed tick, in simulated seconds: the elapsed time a
    /// rate divides by, and the not-applicable test of every rate.
    pub elapsed_s: f64,
    /// Agents the bucket observed live in at least one tick, which is what
    /// distinguishes a reported zero from no observation at all.
    pub observed_agents: u64,
    /// Agents the bucket served: their despawn completed a trip inside the run.
    pub served_agents: u64,
    /// Throughput in served agents per simulated second,
    /// `served_agents / elapsed_s`. `None` when the bucket observed no agent or
    /// when the run elapsed no time.
    pub throughput_agents_per_s: Option<f64>,
    /// Mean trip time in seconds of a served agent. `None` when the bucket
    /// served none.
    pub mean_travel_time_s: Option<f64>,
    /// Total trip time in seconds over the bucket's served agents, `0.0` when it
    /// served none.
    pub total_travel_time_s: f64,
    /// Mean stopped delay in seconds per served agent. `None` when the bucket
    /// served none.
    pub mean_stopped_delay_s: Option<f64>,
    /// Total stopped delay in seconds over the bucket's served agents, `0.0`
    /// when it served none or none of them stopped.
    pub total_stopped_delay_s: f64,
    /// Mean control delay in seconds per served agent. `None` when the bucket
    /// served none.
    pub mean_control_delay_s: Option<f64>,
    /// Total control delay in seconds over the bucket's served agents, `0.0`
    /// when it served none or none of them waited.
    pub total_control_delay_s: f64,
    /// Most standing agents the bucket ever held at a tick end. `None` when the
    /// bucket observed no agent.
    pub maximum_queue_length: Option<QueueLength>,
    /// Longest stopped state the bucket closed. `None` when it closed none.
    pub maximum_queue_duration: Option<QueueDuration>,
    /// Mean duration in seconds of the stopped states the bucket closed. `None`
    /// when it closed none.
    pub mean_queue_duration_s: Option<f64>,
}

/// The operational metrics of one run: throughput, delay, and queues.
///
/// Fed by the same per-tick call as the interaction metrics and read through
/// [`InteractionMetrics::operation`], at the run level and per mode and
/// movement. See the module card for the formulas, units, statuses,
/// tie-breaks, and disaggregation.
#[derive(Debug, Default)]
pub struct OperationMetrics {
    /// End of the last observed tick, in simulated seconds.
    elapsed_s: f64,
    /// Trip state of every agent the pass still observes, ascending by id.
    trips: BTreeMap<AgentId, AgentTrip>,
    /// Start of every open stopped state, ascending by agent.
    open_stops: BTreeMap<AgentId, f64>,
    /// Start of every open control state, ascending by `(agent, kind)`.
    open_controls: BTreeMap<(AgentId, ControlTransitionKind), f64>,
    /// Reused buffer of the agents a despawn closed in the tick being observed.
    despawning: Vec<AgentId>,
    /// Reused buffer of the control kinds a despawn left open.
    closing_controls: Vec<ControlTransitionKind>,
    /// The run-level bucket.
    run: Bucket,
    /// The per-mode buckets, indexed by [`mode_slot`].
    modes: [Bucket; 2],
    /// The per-movement buckets that observed an agent, ascending by key.
    movements: BTreeMap<MovementKey, Bucket>,
}

impl OperationMetrics {
    /// End of the last observed tick, in simulated seconds.
    pub fn elapsed_s(&self) -> f64 {
        self.elapsed_s
    }

    /// The operational values over the whole run.
    pub fn values(&self) -> OperationValues {
        self.run.values(self.elapsed_s)
    }

    /// The operational values of one mode, over the run.
    pub fn mode_values(&self, mode: AgentMode) -> OperationValues {
        self.modes[mode_slot(mode)].values(self.elapsed_s)
    }

    /// The operational values of one movement, absent until an agent of that
    /// movement has been observed.
    pub fn movement_values(&self, key: MovementKey) -> Option<OperationValues> {
        self.movements
            .get(&key)
            .map(|bucket| bucket.values(self.elapsed_s))
    }

    /// Every observed movement's operational values, ascending by key.
    pub fn movements(&self) -> impl Iterator<Item = (MovementKey, OperationValues)> + '_ {
        self.movements
            .iter()
            .map(|(key, bucket)| (*key, bucket.values(self.elapsed_s)))
    }

    /// Observe one integrated tick: the live agents, then the state records the
    /// tick produced.
    ///
    /// The live agents are read first, so the trip state a tick's records close
    /// already exists: an agent live at the start of tick `t` was admitted at
    /// the end of tick `t - 1`, and the agent store is what reveals it.
    fn observe(&mut self, agents: &AgentStore, tick: u64, step: Seconds, events: &[Event]) {
        let time_s = SimTime::from_tick(tick, step).seconds();
        self.elapsed_s = time_s;
        let spawn_s = SimTime::from_tick(tick.saturating_sub(1), step).seconds();
        for index in 0..agents.len() {
            if agents.alive[index] {
                self.ensure_trip(agents, AgentId::from_index(index), spawn_s);
            }
        }
        for event in events {
            match *event {
                Event::Queue { agent, joined } => match joined {
                    true => self.join_queue(agents, agent, spawn_s, time_s),
                    false => self.close_stop(agent, tick, time_s),
                },
                Event::ControlTransition {
                    agent,
                    control,
                    active,
                } => match active {
                    true => {
                        self.ensure_trip(agents, agent, spawn_s);
                        self.open_controls.insert((agent, control), time_s);
                    }
                    false => {
                        if let Some(start_s) = self.open_controls.remove(&(agent, control))
                            && let Some(trip) = self.trips.get_mut(&agent)
                        {
                            trip.control_s += time_s - start_s;
                        }
                    }
                },
                Event::Despawned { agent, .. } => {
                    // A trip the pass has not seen yet is recorded from the
                    // store here, so an agent admitted and served between two
                    // observations still contributes its trip.
                    self.ensure_trip(agents, agent, spawn_s);
                    self.despawning.push(agent);
                }
                _ => {}
            }
        }
        // A despawn is recorded before the closing records of its own states,
        // because the tick's stream is ordered by agent then kind, so a trip is
        // served only once every state of that tick has been read.
        for position in 0..self.despawning.len() {
            let agent = self.despawning[position];
            self.serve(agent, tick, time_s);
        }
        self.despawning.clear();
        // The tick-end queue length of every bucket, which is the state the tick
        // integrated: a body that was standing at the tick start and is moving
        // at its end has already left the queue by its own record.
        self.run.observe_length(tick);
        for bucket in &mut self.modes {
            bucket.observe_length(tick);
        }
        for bucket in self.movements.values_mut() {
            bucket.observe_length(tick);
        }
    }

    /// The trip state of one agent, recorded on first sight.
    ///
    /// An agent the pass has not seen before was admitted at the end of the
    /// previous tick, so `spawn_s` is its admission time exactly; the same rule
    /// covers the initial population, which is live at tick zero.
    fn ensure_trip(&mut self, agents: &AgentStore, agent: AgentId, spawn_s: f64) -> AgentTrip {
        if let Some(trip) = self.trips.get(&agent) {
            return *trip;
        }
        let index = agent.index();
        let mode = *agents
            .mode
            .get(index)
            .expect("an observed agent has a store slot");
        let movement = movement_key(agents, index, mode);
        let trip = AgentTrip {
            mode,
            movement,
            spawned_s: spawn_s,
            stopped_s: 0.0,
            control_s: 0.0,
        };
        self.trips.insert(agent, trip);
        self.run.observed_agents += 1;
        self.modes[mode_slot(mode)].observed_agents += 1;
        if let Some(key) = movement {
            self.movements.entry(key).or_default().observed_agents += 1;
        }
        trip
    }

    /// Open one agent's stopped state.
    fn join_queue(&mut self, agents: &AgentStore, agent: AgentId, spawn_s: f64, time_s: f64) {
        let trip = self.ensure_trip(agents, agent, spawn_s);
        self.open_stops.insert(agent, time_s);
        self.for_each_bucket(trip, |bucket| bucket.standing += 1);
    }

    /// Close one agent's stopped state at `time_s`, if it is open, and record
    /// its duration against every bucket the agent belongs to.
    fn close_stop(&mut self, agent: AgentId, tick: u64, time_s: f64) {
        let Some(start_s) = self.open_stops.remove(&agent) else {
            return;
        };
        let Some(mut trip) = self.trips.get(&agent).copied() else {
            return;
        };
        let duration = QueueDuration {
            agent,
            seconds: time_s - start_s,
            tick,
        };
        trip.stopped_s += duration.seconds;
        self.trips.insert(agent, trip);
        self.for_each_bucket(trip, |bucket| bucket.close_stop(duration));
    }

    /// Complete one agent's trip and remove its state.
    ///
    /// A despawn closes the agent's stream, so a stopped or control state still
    /// open ends here, as a duration like any other close. The closing records
    /// the safety pass emits for a despawned agent carry this same tick, so they
    /// close the state at the same instant and the two never double-count.
    fn serve(&mut self, agent: AgentId, tick: u64, time_s: f64) {
        self.close_stop(agent, tick, time_s);
        let Some(mut trip) = self.trips.remove(&agent) else {
            return;
        };
        self.closing_controls.clear();
        self.closing_controls.extend(
            self.open_controls
                .keys()
                .filter(|(open_agent, _)| *open_agent == agent)
                .map(|(_, control)| *control),
        );
        for position in 0..self.closing_controls.len() {
            let control = self.closing_controls[position];
            if let Some(start_s) = self.open_controls.remove(&(agent, control)) {
                trip.control_s += time_s - start_s;
            }
        }
        let seconds = time_s - trip.spawned_s;
        self.for_each_bucket(trip, |bucket| {
            bucket.served_agents += 1;
            bucket.travel_time_s += seconds;
            bucket.stopped_s += trip.stopped_s;
            bucket.control_s += trip.control_s;
        });
    }

    /// Apply one update to every bucket an agent belongs to: the run bucket,
    /// its mode bucket, and its movement bucket when it has one.
    fn for_each_bucket(&mut self, trip: AgentTrip, update: impl FnMut(&mut Bucket)) {
        let mut update = update;
        update(&mut self.run);
        update(&mut self.modes[mode_slot(trip.mode)]);
        if let Some(key) = trip.movement {
            update(self.movements.entry(key).or_default());
        }
    }
}

/// One observed agent's trip state.
#[derive(Debug, Clone, Copy)]
struct AgentTrip {
    /// The mode bucket the agent belongs to.
    mode: AgentMode,
    /// The movement bucket, absent for an agent with no assigned movement.
    movement: Option<MovementKey>,
    /// End of the tick that admitted the agent, in simulated seconds.
    spawned_s: f64,
    /// Stopped seconds the agent has accumulated.
    stopped_s: f64,
    /// Control-waiting seconds the agent has accumulated.
    control_s: f64,
}

/// The running operational values of one disaggregation bucket.
///
/// Every field is a running total, a tick-end count, or a first-wins maximum,
/// so the bucket's size does not grow with the run.
#[derive(Debug, Default, Clone, Copy)]
struct Bucket {
    /// Agents observed live in this bucket.
    observed_agents: u64,
    /// Agents whose trip completed inside the run.
    served_agents: u64,
    /// Total trip seconds over the served agents.
    travel_time_s: f64,
    /// Total stopped seconds over the served agents.
    stopped_s: f64,
    /// Total control-waiting seconds over the served agents.
    control_s: f64,
    /// Agents standing at the current tick end.
    standing: u64,
    /// Most agents held at a tick end, first tick wins a tie.
    maximum_length: Option<QueueLength>,
    /// Stopped states this bucket closed.
    closed_stops: u64,
    /// Total seconds of the closed stopped states.
    stop_total_s: f64,
    /// Longest closed stopped state, first closure wins a tie.
    maximum_duration: Option<QueueDuration>,
}

impl Bucket {
    /// Record this bucket's standing count at one tick end.
    fn observe_length(&mut self, tick: u64) {
        let length = QueueLength {
            agents: self.standing,
            tick,
        };
        if self
            .maximum_length
            .is_none_or(|current| length.agents > current.agents)
        {
            self.maximum_length = Some(length);
        }
    }

    /// Record one stopped state that closed, and its duration.
    fn close_stop(&mut self, duration: QueueDuration) {
        self.standing -= 1;
        self.closed_stops += 1;
        self.stop_total_s += duration.seconds;
        if self
            .maximum_duration
            .is_none_or(|current| duration.seconds > current.seconds)
        {
            self.maximum_duration = Some(duration);
        }
    }

    /// The bucket's operational values, with each option's status rule.
    fn values(&self, elapsed_s: f64) -> OperationValues {
        let mean = |total: f64| -> Option<f64> {
            (self.served_agents > 0).then(|| total / self.served_agents as f64)
        };
        OperationValues {
            elapsed_s,
            observed_agents: self.observed_agents,
            served_agents: self.served_agents,
            throughput_agents_per_s: (self.observed_agents > 0 && elapsed_s > 0.0)
                .then(|| self.served_agents as f64 / elapsed_s),
            mean_travel_time_s: mean(self.travel_time_s),
            total_travel_time_s: self.travel_time_s,
            mean_stopped_delay_s: mean(self.stopped_s),
            total_stopped_delay_s: self.stopped_s,
            mean_control_delay_s: mean(self.control_s),
            total_control_delay_s: self.control_s,
            maximum_queue_length: (self.observed_agents > 0)
                .then_some(self.maximum_length)
                .flatten(),
            maximum_queue_duration: self.maximum_duration,
            mean_queue_duration_s: (self.closed_stops > 0)
                .then(|| self.stop_total_s / self.closed_stops as f64),
        }
    }
}

/// Position of a mode in the per-mode bucket table.
const fn mode_slot(mode: AgentMode) -> usize {
    match mode {
        AgentMode::Vehicle => 0,
        AgentMode::Pedestrian => 1,
    }
}

/// The movement key of one agent's store row, absent when the agent has no
/// assigned movement.
fn movement_key(agents: &AgentStore, index: usize, mode: AgentMode) -> Option<MovementKey> {
    match mode {
        AgentMode::Vehicle => agents.movement[index].map(MovementKey::Vehicle),
        AgentMode::Pedestrian => agents.pedestrian_route[index].map(MovementKey::Pedestrian),
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
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
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

    /// One live agent's store row for the operational fixtures.
    fn push_agent(
        store: &mut AgentStore,
        mode: AgentMode,
        movement: Option<MovementId>,
        pedestrian_route: Option<PedestrianRouteId>,
    ) -> AgentId {
        store.push(AgentInit {
            mode,
            path: PathId::from_index(0),
            distance_m: 0.0,
            speed_mps: 0.0,
            position: DVec2::ZERO,
            heading_rad: 0.0,
            body_length_m: 1.0,
            body_width_m: 1.0,
            direction: 1.0,
            movement,
            profile: None,
            narrow_profile: None,
            pedestrian_route,
            pedestrian_profile: None,
            route_state: None,
        })
    }

    /// One queue join or departure record.
    const fn queue(agent: AgentId, joined: bool) -> Event {
        Event::Queue { agent, joined }
    }

    /// One control-state edge.
    const fn control(agent: AgentId, kind: ControlTransitionKind, active: bool) -> Event {
        Event::ControlTransition {
            agent,
            control: kind,
            active,
        }
    }

    /// One despawn record.
    fn despawn(agent: AgentId) -> Event {
        Event::Despawned {
            agent,
            path: PathId::from_index(0),
            reason: crate::DespawnReason::ExitedPath,
        }
    }

    /// Assert two computed values agree to the arithmetic's own resolution.
    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-12,
            "{actual} is not {expected}"
        );
    }

    /// The vehicle movement and pedestrian route the operational fixtures use.
    fn vehicle_movement() -> MovementKey {
        MovementKey::Vehicle(MovementId::from_index(0))
    }

    fn pedestrian_route() -> MovementKey {
        MovementKey::Pedestrian(PedestrianRouteId::from_index(0))
    }

    /// The operational families over a scripted three-tick run: a vehicle and a
    /// pedestrian stand together, the vehicle moves off into a signal hold and
    /// is then served, and the pedestrian's stop is still open at the end.
    #[test]
    fn the_operation_pass_reports_throughput_delay_and_queues() {
        let step = Seconds::from_secs(1.0);
        let mut store = AgentStore::default();
        let vehicle = push_agent(
            &mut store,
            AgentMode::Vehicle,
            Some(MovementId::from_index(0)),
            None,
        );
        let walker = push_agent(
            &mut store,
            AgentMode::Pedestrian,
            None,
            Some(PedestrianRouteId::from_index(0)),
        );
        let mut metrics = OperationMetrics::default();

        // Tick 1: both agents are live and were admitted at tick zero; both
        // come to rest.
        metrics.observe(
            &store,
            1,
            step,
            &[queue(vehicle, true), queue(walker, true)],
        );
        let run = metrics.values();
        assert_eq!(run.elapsed_s, 1.0);
        assert_eq!(run.observed_agents, 2);
        assert_eq!(run.served_agents, 0);
        assert_eq!(
            run.throughput_agents_per_s,
            Some(0.0),
            "a rest is not a service, so the rate is a true zero"
        );
        assert_eq!(run.mean_travel_time_s, None, "no trip has completed");
        assert_close(run.total_travel_time_s, 0.0);
        assert_eq!(
            run.maximum_queue_length,
            Some(QueueLength { agents: 2, tick: 1 })
        );
        assert_eq!(run.maximum_queue_duration, None, "no stop has closed");
        assert_eq!(run.mean_queue_duration_s, None);

        // Tick 2: the vehicle moves off and holds at its signal; the pedestrian
        // keeps standing.
        metrics.observe(
            &store,
            2,
            step,
            &[
                queue(vehicle, false),
                control(vehicle, ControlTransitionKind::SignalStop, true),
            ],
        );
        let run = metrics.values();
        assert_eq!(
            run.maximum_queue_length,
            Some(QueueLength { agents: 2, tick: 1 }),
            "a tie keeps the earlier tick"
        );
        assert_eq!(
            run.maximum_queue_duration,
            Some(QueueDuration {
                agent: vehicle,
                seconds: 1.0,
                tick: 2,
            })
        );
        assert_close(run.mean_queue_duration_s.expect("one closed stop"), 1.0);

        // Tick 3: the signal hold ends and the vehicle is served.
        metrics.observe(
            &store,
            3,
            step,
            &[
                control(vehicle, ControlTransitionKind::SignalStop, false),
                despawn(vehicle),
            ],
        );
        let run = metrics.values();
        assert_eq!(run.elapsed_s, 3.0);
        assert_eq!(run.served_agents, 1);
        assert_close(
            run.throughput_agents_per_s
                .expect("the run observed agents"),
            1.0 / 3.0,
        );
        assert_close(run.mean_travel_time_s.expect("one trip"), 3.0);
        assert_close(run.total_travel_time_s, 3.0);
        assert_close(run.mean_stopped_delay_s.expect("one served agent"), 1.0);
        assert_close(run.total_stopped_delay_s, 1.0);
        assert_close(run.mean_control_delay_s.expect("one served agent"), 1.0);
        assert_close(run.total_control_delay_s, 1.0);

        // Disaggregation: the vehicle's trip is booked to its mode and to its
        // movement, the standing pedestrian's bucket has no trip yet, and each
        // bucket reports its own queue.
        let vehicles = metrics.mode_values(AgentMode::Vehicle);
        assert_eq!(vehicles.observed_agents, 1);
        assert_eq!(vehicles.served_agents, 1);
        assert_close(vehicles.mean_travel_time_s.expect("one trip"), 3.0);
        assert_eq!(
            vehicles.maximum_queue_length,
            Some(QueueLength { agents: 1, tick: 1 })
        );
        let walkers = metrics.mode_values(AgentMode::Pedestrian);
        assert_eq!(walkers.observed_agents, 1);
        assert_eq!(walkers.served_agents, 0);
        assert_eq!(walkers.throughput_agents_per_s, Some(0.0));
        assert_eq!(walkers.mean_travel_time_s, None);
        assert_eq!(
            walkers.maximum_queue_length,
            Some(QueueLength { agents: 1, tick: 1 })
        );
        assert_eq!(walkers.maximum_queue_duration, None);

        let movement = metrics
            .movement_values(vehicle_movement())
            .expect("the vehicle's movement was observed");
        assert_close(movement.total_stopped_delay_s, 1.0);
        assert_eq!(movement.served_agents, 1);
        let route = metrics
            .movement_values(pedestrian_route())
            .expect("the pedestrian's route was observed");
        assert_eq!(route.served_agents, 0);
        assert_eq!(
            route.maximum_queue_length,
            Some(QueueLength { agents: 1, tick: 1 })
        );
        assert_eq!(
            metrics.movement_values(MovementKey::Vehicle(MovementId::from_index(9))),
            None,
            "an unobserved movement has no bucket"
        );
        assert_eq!(
            metrics.movements().map(|(key, _)| key).collect::<Vec<_>>(),
            vec![vehicle_movement(), pedestrian_route()],
            "the buckets are in ascending key order"
        );
    }

    /// A trip is measured from the tick that admitted its agent, which the pass
    /// reads from the agent store rather than from a spawn record: an agent live
    /// at the start of tick `t` was admitted at the end of tick `t - 1`.
    #[test]
    fn a_trip_is_measured_from_the_admission_the_store_reveals() {
        let step = Seconds::from_secs(1.0);
        let mut store = AgentStore::default();
        let early = push_agent(&mut store, AgentMode::Vehicle, None, None);
        let mut metrics = OperationMetrics::default();
        metrics.observe(&store, 1, step, &[]);
        assert_eq!(metrics.values().observed_agents, 1);
        // An agent admitted at the end of tick 2 is first observed at tick 3.
        let late = push_agent(&mut store, AgentMode::Vehicle, None, None);
        metrics.observe(&store, 3, step, &[]);
        assert_eq!(metrics.values().observed_agents, 2);
        // Served at tick 5: five seconds for the early agent, whose admission is
        // tick zero, and three for the late one, admitted at the end of tick 2.
        metrics.observe(&store, 5, step, &[despawn(early), despawn(late)]);
        let run = metrics.values();
        assert_eq!(run.served_agents, 2);
        assert_close(run.total_travel_time_s, 5.0 + 3.0);
        assert_close(run.mean_travel_time_s.expect("two trips"), 4.0);
    }

    /// A despawn closes the agent's stream, so a stop still open then ends at
    /// the despawn even when the tick carries no closing record.
    #[test]
    fn a_despawn_closes_an_open_stop_state() {
        let step = Seconds::from_secs(1.0);
        let mut store = AgentStore::default();
        let agent = push_agent(
            &mut store,
            AgentMode::Vehicle,
            Some(MovementId::from_index(0)),
            None,
        );
        let mut metrics = OperationMetrics::default();
        metrics.observe(&store, 1, step, &[queue(agent, true)]);
        metrics.observe(&store, 2, step, &[despawn(agent)]);
        let run = metrics.values();
        assert_eq!(run.served_agents, 1);
        assert_close(run.mean_stopped_delay_s.expect("one served agent"), 1.0);
        assert_eq!(
            run.maximum_queue_duration,
            Some(QueueDuration {
                agent,
                seconds: 1.0,
                tick: 2,
            })
        );
    }

    /// The tick's stream is ordered agent-then-kind, so an agent's despawn
    /// record precedes the closing record of its own stop state. The two must
    /// not count the interval twice, in either order.
    #[test]
    fn a_despawn_and_the_closing_record_of_its_own_stop_count_once() {
        let step = Seconds::from_secs(1.0);
        let mut store = AgentStore::default();
        let agent = push_agent(
            &mut store,
            AgentMode::Vehicle,
            Some(MovementId::from_index(0)),
            None,
        );
        for events in [
            [despawn(agent), queue(agent, false)],
            [queue(agent, false), despawn(agent)],
        ] {
            let mut metrics = OperationMetrics::default();
            metrics.observe(&store, 1, step, &[queue(agent, true)]);
            metrics.observe(&store, 2, step, &events);
            let run = metrics.values();
            assert_eq!(run.served_agents, 1);
            assert_close(run.total_stopped_delay_s, 1.0);
            assert_close(run.total_travel_time_s, 2.0);
            assert_eq!(
                run.maximum_queue_length,
                Some(QueueLength { agents: 1, tick: 1 })
            );
        }
    }

    /// Two stops of equal length closed on different ticks tie, and the first
    /// closure wins: the earlier tick retains the maximum.
    #[test]
    fn an_equal_queue_duration_keeps_the_first_closure() {
        let step = Seconds::from_secs(1.0);
        let mut store = AgentStore::default();
        let first = push_agent(
            &mut store,
            AgentMode::Vehicle,
            Some(MovementId::from_index(0)),
            None,
        );
        let second = push_agent(
            &mut store,
            AgentMode::Vehicle,
            Some(MovementId::from_index(1)),
            None,
        );
        let mut metrics = OperationMetrics::default();
        metrics.observe(&store, 1, step, &[queue(first, true)]);
        metrics.observe(&store, 2, step, &[queue(first, false), queue(second, true)]);
        metrics.observe(&store, 3, step, &[queue(second, false)]);
        let run = metrics.values();
        assert_eq!(
            run.maximum_queue_duration,
            Some(QueueDuration {
                agent: first,
                seconds: 1.0,
                tick: 2,
            }),
            "the first closure keeps the tie"
        );
        assert_eq!(
            run.maximum_queue_length,
            Some(QueueLength { agents: 1, tick: 1 })
        );
        assert_close(run.mean_queue_duration_s.expect("two closed stops"), 1.0);
    }

    /// An agent admitted and served between two observations is still measured:
    /// its despawn record books the trip from the previous tick's end, and it
    /// counts as an observation of its buckets.
    #[test]
    fn an_agent_that_never_stood_live_is_still_measured() {
        let step = Seconds::from_secs(1.0);
        let mut store = AgentStore::default();
        let agent = push_agent(
            &mut store,
            AgentMode::Vehicle,
            Some(MovementId::from_index(0)),
            None,
        );
        store.alive[0] = false;
        let mut metrics = OperationMetrics::default();
        metrics.observe(&store, 1, step, &[despawn(agent)]);
        let run = metrics.values();
        assert_eq!(run.observed_agents, 1);
        assert_eq!(run.served_agents, 1);
        assert_close(run.mean_travel_time_s.expect("one trip"), 1.0);
    }

    /// A bucket that observed nothing reports no value at all, and the rate of
    /// a run with no elapsed time is not applicable rather than zero.
    #[test]
    fn an_unobserved_bucket_reports_no_observation() {
        let metrics = OperationMetrics::default();
        let run = metrics.values();
        assert_eq!(run.elapsed_s, 0.0);
        assert_eq!(run.observed_agents, 0);
        assert_eq!(run.served_agents, 0);
        assert_eq!(run.throughput_agents_per_s, None);
        assert_eq!(run.mean_travel_time_s, None);
        assert_eq!(run.mean_stopped_delay_s, None);
        assert_eq!(run.mean_control_delay_s, None);
        assert_eq!(run.maximum_queue_length, None);
        assert_eq!(run.maximum_queue_duration, None);
        assert_eq!(run.mean_queue_duration_s, None);
        // The sums of a bucket that served nobody are true zeros, not absences.
        assert_close(run.total_travel_time_s, 0.0);
        assert_close(run.total_stopped_delay_s, 0.0);
        assert_close(run.total_control_delay_s, 0.0);
        assert_eq!(
            metrics.mode_values(AgentMode::Pedestrian).observed_agents,
            0
        );
        assert_eq!(
            metrics
                .mode_values(AgentMode::Pedestrian)
                .throughput_agents_per_s,
            None
        );
    }

    /// The mean delay is the closed intervals over the served agents, and the
    /// throughput is the served count over the elapsed time: the two properties
    /// the definitions fix, checked against the records that produced them.
    #[test]
    fn the_delay_and_throughput_ratios_hold_over_a_scripted_run() {
        let step = Seconds::from_secs(2.0);
        let mut store = AgentStore::default();
        let agents: Vec<AgentId> = (0..3)
            .map(|_| push_agent(&mut store, AgentMode::Vehicle, None, None))
            .collect();
        let mut metrics = OperationMetrics::default();
        // The first agent stands from the end of tick 1 to the end of tick 2;
        // the second never stands and the third is still running at the end.
        metrics.observe(&store, 1, step, &[queue(agents[0], true)]);
        metrics.observe(&store, 2, step, &[queue(agents[0], false)]);
        metrics.observe(&store, 3, step, &[despawn(agents[0]), despawn(agents[1])]);
        let run = metrics.values();
        assert_eq!(run.elapsed_s, 6.0);
        assert_eq!(run.observed_agents, 3);
        assert_eq!(run.served_agents, 2);
        assert_close(run.total_travel_time_s, 6.0 + 6.0);
        assert_close(run.total_stopped_delay_s, 2.0);
        assert_close(run.throughput_agents_per_s.expect("two served"), 2.0 / 6.0);
        assert_close(
            run.mean_travel_time_s.expect("two trips"),
            run.total_travel_time_s / 2.0,
        );
        assert_close(
            run.mean_stopped_delay_s.expect("two served agents"),
            run.total_stopped_delay_s / 2.0,
        );
        assert_close(
            run.mean_queue_duration_s.expect("one closed stop"),
            run.maximum_queue_duration.expect("one closed stop").seconds,
        );
        assert_close(run.mean_queue_duration_s.expect("one closed stop"), 2.0);
    }
}
