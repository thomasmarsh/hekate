//! Documented contextual wrong-way (opposing-traversal) decision.
//!
//! `docs/schema-v2-contract.md` *Contextual wrong-way traversal* fixes this
//! decision. The nominal direction, the permitted direction, and the physically
//! possible direction stay separate, and a wrong-way traversal is an ordinary
//! traversal the ordinary routing, steering, collision, yielding, and event
//! paths carry. This module owns the decision over the observer-stage context
//! the contract fixes there: it returns the perceived rule, the selected option,
//! a reason code, the affected facility and movement, and the context values it
//! read. It adds no visibility-error or perception subsystem, chooses no
//! trajectory, routes nothing, and emits nothing.
//!
//! It also owns the **violation interval** the contract fixes from that decision
//! ([`OpposingTraversalTracker`]): the maximal run of observed steps on which a
//! body's centre is inside one object's extent while its traversal direction on
//! that object is against the object's rule direction. The tracker is a pure
//! observer like [`crate::close_pass::ClosePassTracker`] — it borrows the agent
//! store and the compiled scenario and writes only its own records — and the
//! kernel emits one [`crate::Event::OpposingTraversal`] when an interval opens
//! and one when it closes.
//!
//! # Model card
//!
//! ## Inputs
//!
//! [`WrongWayInputs`] carries exactly the decision-instant inputs of the
//! contract, already resolved by the caller from the immutable observation and
//! the compiled policy:
//!
//! - physical connectivity of the opposing traversal — the reverse traversal is
//!   physically possible, or a connected opposing traversal exists through an
//!   adjacency or connector ([`WrongWayInputs::opposing_connected`]);
//! - the object's authored nominal direction, read from
//!   [`tangle_model::CompiledFacility::nominal_direction`], which must not be
//!   `either` for an opposing option to exist;
//! - the nominal and opposing remaining lengths and expected speeds, from which
//!   the estimated time saving is
//!   `nominal_remaining_length_m / nominal_expected_speed_mps -
//!   opposing_remaining_length_m / opposing_expected_speed_mps`. The expected
//!   speed is the agent's own desired free-flow speed capped by the effective
//!   facility limit, and the caller performs that cap; this module never
//!   re-derives it;
//! - the observed opposing density within the target traversal ahead;
//! - the applicable `permissions[]` effect for `(holder, target)`, read through
//!   [`tangle_model::CompiledScenario::permission_effect`] or the traversal's
//!   own [`tangle_model::FacilityTraversalPolicy::nominal_effect`], or `None`
//!   when no statement binds the pair;
//! - the agent's sampled `compliance`, the agent's desired speed, and the
//!   scenario's `wrong_way` thresholds, read through
//!   [`tangle_model::CompiledScenario::wrong_way_policy`];
//! - the affected facility and the optional affected movement.
//!
//! The random draw is the second parameter of [`decide`] rather than an input
//! field: the contract takes one draw `u` in `[0, 1)` from the versioned
//! `maneuver` stream keyed by the root seed, the run ID, the stable `AgentId`,
//! and the agent's decision ordinal, and the keyed draw is supplied by the
//! caller so this procedure stays a pure function of its context. This module
//! owns that keyed draw as [`maneuver_draw`], a pure function of
//! `(root_seed, agent, ordinal)` built on the crate's named-stream derivation;
//! the run-id component of the key has no field in [`crate::RunConfig`] yet and
//! is documented rather than invented there.
//!
//! ## Rule
//!
//! The procedure is total and deterministic, and follows the contract's order:
//!
//! 1. A physically disconnected opposing traversal rejects as
//!    [`WrongWayReason::NoOpposingPath`], and an `either` nominal direction
//!    rejects as [`WrongWayReason::NoNominalDirection`]. Neither takes a draw,
//!    and both select [`WrongWayOption::Nominal`]. Together these are the
//!    physical precondition [`WrongWayInputs::physical_rejection`] reports.
//!    It is deliberately separate from legality: a connected traversal a
//!    `prohibit` statement forbids still admits an opposing option, so the
//!    decision reaches the draw and a selected opposing option yields
//!    [`WrongWayReason::NoncompliantChoice`] rather than a physical rejection.
//! 2. An estimated time saving strictly below `min_time_saving_s` rejects as
//!    [`WrongWayReason::InsufficientTimeSaving`], and an observed opposing
//!    density strictly above `max_opposing_density_per_km` rejects as
//!    [`WrongWayReason::OpposingDensityTooHigh`]. Both are non-random
//!    rejections: they take no draw and select [`WrongWayOption::Nominal`].
//!    The saving is compared first, so a decision that fails both names the
//!    saving.
//! 3. Otherwise the agent selects the opposing option when
//!    `u < urgency * (1 - compliance)`. An exact tie selects the nominal
//!    option, so a fully compliant agent under any urgency, and any agent under
//!    zero urgency, keeps the nominal option.
//! 4. A selected nominal option is [`WrongWayReason::CompliantChoice`]. A
//!    selected opposing option's legality follows from the applicable effect
//!    alone: `permit` gives [`WrongWayReason::LegalPermission`], `obligate`
//!    gives [`WrongWayReason::LegalObligation`], and an absent or `prohibit`ed
//!    statement gives [`WrongWayReason::NoncompliantChoice`]. No other input
//!    changes legality.
//!
//! An occupied opposing corridor is not a precondition failure, so this
//! procedure never reads occupancy beyond the density threshold: the selected
//! traversal is left to the ordinary claim, prediction, yielding, and collision
//! machinery.
//!
//! ## Preconditions on the caller
//!
//! The decision is reached only by an agent whose compiled capability carries
//! `reverse_direction` and only when the scenario authors
//! `maneuver_policy.wrong_way`; a caller that has neither never calls
//! [`decide`]. Inputs are read at the decision instant and are not
//! re-validated here: `compliance` and `urgency` are the `[0, 1]` values the
//! source schema guarantees, and the lengths, speeds, and density are the
//! observation's own estimates.
//!
//! ## Reproducibility
//!
//! [`decide`] is pure: the same inputs and the same draw always yield the same
//! record. The draw is the only random term, and it is taken once per decision
//! evaluation, after the non-random preconditions pass, so an agent without an
//! opposing option cannot perturb another agent's draws. A rejected decision
//! takes no draw at all, which is why every rejection is a function of its
//! inputs alone.

use std::collections::{BTreeMap, BTreeSet};

use tangle_model::{
    CompiledScenario, FacilityId, MovementDirection, MovementId, NominalDirection,
    PermissionEffect, WrongWayPolicySource,
};

use crate::agent::{AgentId, AgentStore};
use crate::rng::{STREAM_MANEUVER, derive_stream, uniform01};
use crate::time::SimTime;
use crate::units::Seconds;

/// Why a wrong-way decision selected its option.
///
/// The set is closed and every rejection an eligibility check can produce has
/// one of these codes, so a rejected precondition has an inspectable reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrongWayReason {
    /// The opposing traversal is not physically connected, so no opposing
    /// option exists.
    NoOpposingPath,
    /// The object's authored nominal direction is `either`, so the object has
    /// no rule direction and no opposing option exists.
    NoNominalDirection,
    /// The opposing option saves less than `min_time_saving_s` of estimated
    /// travel time.
    InsufficientTimeSaving,
    /// The observed opposing density exceeds `max_opposing_density_per_km`.
    OpposingDensityTooHigh,
    /// The agent kept the nominal option: the draw did not clear the
    /// non-compliance threshold.
    CompliantChoice,
    /// The agent selected the opposing option and no statement makes it legal.
    NoncompliantChoice,
    /// The agent selected an opposing option a `permit` statement makes legal.
    LegalPermission,
    /// The agent selected an opposing option an `obligate` statement makes the
    /// obligated direction.
    LegalObligation,
}

impl WrongWayReason {
    /// Every reason of the closed set, in contract order.
    pub const ALL: [Self; 8] = [
        Self::NoOpposingPath,
        Self::NoNominalDirection,
        Self::InsufficientTimeSaving,
        Self::OpposingDensityTooHigh,
        Self::CompliantChoice,
        Self::NoncompliantChoice,
        Self::LegalPermission,
        Self::LegalObligation,
    ];

    /// The stable lowercase code for inspectors, traces, and metrics.
    ///
    /// Each code is the contract's own name for the reason, so a recorded code
    /// is stable across runs and readable without this crate's source.
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoOpposingPath => "no_opposing_path",
            Self::NoNominalDirection => "no_nominal_direction",
            Self::InsufficientTimeSaving => "insufficient_time_saving",
            Self::OpposingDensityTooHigh => "opposing_density_too_high",
            Self::CompliantChoice => "compliant_choice",
            Self::NoncompliantChoice => "noncompliant_choice",
            Self::LegalPermission => "legal_permission",
            Self::LegalObligation => "legal_obligation",
        }
    }
}

/// The traversal option a wrong-way decision selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrongWayOption {
    /// The object's nominal direction: the ordinary traversal.
    Nominal,
    /// Against the rule direction: the opposing traversal.
    Opposing,
}

/// The decision-instant context of one wrong-way decision.
///
/// Every field is read at the decision instant from the immutable observation
/// and the compiled policy, and the struct is `Copy` so an observer snapshot
/// can carry the context cheaply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrongWayInputs {
    /// Whether the opposing traversal is physically connected: the reverse
    /// traversal is in the object's physically possible directions, or a
    /// connected opposing traversal exists through an adjacency or connector.
    pub opposing_connected: bool,
    /// The object's authored nominal direction. `Either` means the object has
    /// no rule direction and therefore no opposing option.
    pub nominal_direction: NominalDirection,
    /// Remaining length of the nominal option along the compiled route, metres.
    pub nominal_remaining_length_m: f64,
    /// Expected speed of the nominal option, metres per second: the agent's
    /// desired free-flow speed capped by the effective facility limit.
    pub nominal_expected_speed_mps: f64,
    /// Remaining length of the opposing option along the compiled route, metres.
    pub opposing_remaining_length_m: f64,
    /// Expected speed of the opposing option, metres per second, capped by the
    /// same rule as the nominal one.
    pub opposing_expected_speed_mps: f64,
    /// Observed opposing-travelling bodies within the target traversal ahead,
    /// per kilometre of that traversal.
    pub observed_opposing_density_per_km: f64,
    /// The applicable `permissions[]` `nominal_direction` effect for
    /// `(holder, target)`, or `None` when no statement binds the pair. A
    /// `permit` makes the opposing traversal legal, an `obligate` makes it the
    /// obligated direction, and an absent or `prohibit`ed statement makes it a
    /// violation.
    pub permission: Option<PermissionEffect>,
    /// The agent's sampled compliance propensity in `[0, 1]`.
    pub compliance: f64,
    /// The agent's desired free-flow speed, metres per second.
    ///
    /// It contributes no term of its own: the caller caps it by the effective
    /// facility limit to produce [`Self::nominal_expected_speed_mps`] and
    /// [`Self::opposing_expected_speed_mps`], which are the speeds the
    /// procedure divides by. It is carried so an inspector can audit that cap
    /// against the agent's own profile.
    pub desired_speed_mps: f64,
    /// The scenario's `wrong_way` thresholds, read from
    /// `maneuver_policy.wrong_way`.
    pub policy: WrongWayPolicySource,
    /// The facility the decision is about.
    pub facility: FacilityId,
    /// The movement the decision is about, when the traversal carries one.
    pub movement: Option<MovementId>,
}

impl WrongWayInputs {
    /// Estimated travel-time saving of the opposing option over the nominal
    /// option, in seconds.
    ///
    /// This is the contract's own formula, evaluated from the supplied
    /// remaining lengths and expected speeds; a non-positive expected speed
    /// yields an infinite or undefined term rather than a panic.
    pub fn time_saving_s(&self) -> f64 {
        self.nominal_remaining_length_m / self.nominal_expected_speed_mps
            - self.opposing_remaining_length_m / self.opposing_expected_speed_mps
    }

    /// The explicit rejection a physically impossible opposing option produces
    /// before any draw, or `None` when an opposing option exists and the
    /// decision must proceed to the keyed draw.
    ///
    /// This is the contract's physical precondition — a connected opposing
    /// traversal and a nominal direction that is not `either` — and it is
    /// deliberately separate from legality: a connected traversal that a
    /// `prohibit` statement forbids still admits an opposing option, so it
    /// returns `None` and a selected opposing option yields
    /// [`WrongWayReason::NoncompliantChoice`]. The disconnected case is named
    /// first, so an `either` object that is also disconnected reports
    /// [`WrongWayReason::NoOpposingPath`].
    pub fn physical_rejection(&self) -> Option<WrongWayReason> {
        if !self.opposing_connected {
            Some(WrongWayReason::NoOpposingPath)
        } else if self.nominal_direction == NominalDirection::Either {
            Some(WrongWayReason::NoNominalDirection)
        } else {
            None
        }
    }
}

/// Record of one wrong-way decision.
///
/// The record carries the perceived rule, the selected option, the reason code,
/// the affected facility and movement, and the context values the decision
/// read, so a consumer can read why the agent chose nominal or opposing travel.
/// The context values are the ones in force at the decision instant, including
/// for a rejection, so an inspector sees the context that produced the code even
/// when the code names a precondition rather than a threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrongWayDecision {
    /// The perceived rule: the applicable permission or obligation effect, or
    /// `None` when no statement binds `(holder, target)`.
    pub perceived_rule: Option<PermissionEffect>,
    /// The option the decision selected.
    pub option: WrongWayOption,
    /// Why the option was selected.
    pub reason: WrongWayReason,
    /// The facility the decision is about.
    pub facility: FacilityId,
    /// The movement the decision is about, when the traversal carries one.
    pub movement: Option<MovementId>,
    /// The estimated travel-time saving the decision read, seconds.
    pub time_saving_s: f64,
    /// The observed opposing density the decision read, per kilometre.
    pub opposing_density_per_km: f64,
    /// The scenario's `wrong_way` urgency the decision read.
    pub urgency: f64,
    /// The agent's compliance propensity the decision read.
    pub compliance: f64,
}

impl WrongWayDecision {
    /// Build the record of one decision from its context, option, and reason.
    fn record(
        inputs: WrongWayInputs,
        option: WrongWayOption,
        reason: WrongWayReason,
        time_saving_s: f64,
    ) -> Self {
        Self {
            perceived_rule: inputs.permission,
            option,
            reason,
            facility: inputs.facility,
            movement: inputs.movement,
            time_saving_s,
            opposing_density_per_km: inputs.observed_opposing_density_per_km,
            urgency: inputs.policy.urgency,
            compliance: inputs.compliance,
        }
    }
}

/// Decide whether an eligible agent takes the opposing traversal or keeps the
/// nominal one.
///
/// `draw` is the keyed draw `u` in `[0, 1)` the contract's `maneuver` stream
/// supplies; it is read only after the non-random preconditions pass, so a
/// rejection ignores it. See the module model card for the rule, the inputs,
/// and the tie-breakers. The function is pure, so the same context and draw
/// always yield the same decision.
pub fn decide(inputs: WrongWayInputs, draw: f64) -> WrongWayDecision {
    let time_saving_s = inputs.time_saving_s();

    if let Some(reason) = inputs.physical_rejection() {
        return WrongWayDecision::record(inputs, WrongWayOption::Nominal, reason, time_saving_s);
    }
    if time_saving_s < inputs.policy.min_time_saving_s {
        return WrongWayDecision::record(
            inputs,
            WrongWayOption::Nominal,
            WrongWayReason::InsufficientTimeSaving,
            time_saving_s,
        );
    }
    if inputs.observed_opposing_density_per_km > inputs.policy.max_opposing_density_per_km {
        return WrongWayDecision::record(
            inputs,
            WrongWayOption::Nominal,
            WrongWayReason::OpposingDensityTooHigh,
            time_saving_s,
        );
    }

    let acceptance = inputs.policy.urgency * (1.0 - inputs.compliance);
    if draw < acceptance {
        let reason = opposing_reason(inputs.permission);
        WrongWayDecision::record(inputs, WrongWayOption::Opposing, reason, time_saving_s)
    } else {
        WrongWayDecision::record(
            inputs,
            WrongWayOption::Nominal,
            WrongWayReason::CompliantChoice,
            time_saving_s,
        )
    }
}

/// The decision reason code a selected opposing option carries under the
/// applicable permission effect.
///
/// This is the contract's legality rule for the opposing option and nothing
/// else: a `permit` makes it legal, an `obligate` makes it the obligated
/// direction, and an absent or `prohibit`ed statement makes it a violation. The
/// decision and the interval record both read their reason through it, so a
/// traversal's recorded code has one spelling.
fn opposing_reason(perceived_rule: Option<PermissionEffect>) -> WrongWayReason {
    match perceived_rule {
        Some(PermissionEffect::Permit) => WrongWayReason::LegalPermission,
        Some(PermissionEffect::Obligate) => WrongWayReason::LegalObligation,
        Some(PermissionEffect::Prohibit) | None => WrongWayReason::NoncompliantChoice,
    }
}

/// The keyed `maneuver` draw `u` in `[0, 1)` for one wrong-way decision.
///
/// The value is a pure function of `(root_seed, `[`STREAM_MANEUVER`]`, agent,
/// ordinal)` and of nothing else: it is the `ordinal`-th draw of the agent's own
/// `maneuver` substream, so an agent's value never depends on how many other
/// agents drew, on their identifiers, or on the order the decisions were
/// evaluated. `ordinal` is the agent's own zero-based decision ordinal, so two
/// agents' first decisions both carry ordinal `0` and draw independently.
///
/// The contract's draw key is the root seed, the run ID, the stable `AgentId`,
/// and the agent's decision ordinal. [`crate::RunConfig`] carries only a root
/// seed and a step and has no run-id field today, so this key omits the run ID:
/// adding one would first widen the run configuration and the run provenance,
/// rather than being invented here.
pub fn maneuver_draw(root_seed: u64, agent: AgentId, ordinal: u32) -> f64 {
    let mut rng = derive_stream(root_seed, STREAM_MANEUVER, agent.get());
    for _ in 0..ordinal {
        let _ = uniform01(&mut rng);
    }
    uniform01(&mut rng)
}

/// The compiled traversal direction an agent's stored travel sign names.
///
/// `AgentStore::direction` is the longitudinal travel sign (`1.0` toward the
/// path end, `-1.0` toward its start), the same sign the kernel's own traversal
/// direction reads.
fn travel_direction(sign: f64) -> MovementDirection {
    if sign < 0.0 {
        MovementDirection::Reverse
    } else {
        MovementDirection::Forward
    }
}

/// One opposing-traversal interval: the record the kernel emits once when the
/// interval opens and once when it closes.
///
/// The body travels `facility` in `direction` while the facility's authored
/// `nominal_direction` is the rule direction, so the traversal is against it.
/// `perceived_rule` is the applicable `permissions[]` effect, absent when no
/// statement binds the pair; `reason` is the decision reason code that effect
/// produces; and `violating` is `false` exactly when a `permit` or `obligate`
/// statement makes the traversal legal, so a legal opposing traversal is
/// recorded under its rule and never counted as a violation. `movement` is the
/// movement connector the traversal carries, absent for a bare facility
/// traversal.
#[derive(Debug, Clone, PartialEq)]
pub struct OpposingTraversalObservation {
    /// The traversing agent.
    pub agent: AgentId,
    /// The facility traversed against its rule direction.
    pub facility: FacilityId,
    /// The movement connector the traversal carries, absent for a facility
    /// traversal.
    pub movement: Option<MovementId>,
    /// The direction the agent actually travels.
    pub direction: MovementDirection,
    /// The facility's authored nominal direction, which is its rule direction.
    pub nominal_direction: NominalDirection,
    /// The permission statement the agent acted under, absent when none
    /// applies.
    pub perceived_rule: Option<PermissionEffect>,
    /// Why the traversal is an opposing one: the decision's own reason code.
    pub reason: WrongWayReason,
    /// `true` for a traversal the applicable rule does not permit.
    pub violating: bool,
    /// Tick the interval opened on.
    pub start_tick: u64,
    /// Tick the interval closed on: the first later step that left the extent,
    /// left the rule direction, entered a different object, or ended the run.
    pub end_tick: u64,
    /// Simulated time of [`Self::start_tick`], in seconds.
    pub start_time_s: f64,
    /// Simulated time of [`Self::end_tick`], in seconds.
    pub end_time_s: f64,
}

impl OpposingTraversalObservation {
    /// Seconds the interval was open: the difference of its two boundary times.
    pub fn duration_s(&self) -> f64 {
        self.end_time_s - self.start_time_s
    }
}

/// Whether two observations are the same traversal of the same object.
///
/// An interval closes and a new one opens when the object or the travel
/// direction changes, which the contract's boundaries name: a different
/// facility or movement is a different object, and the rule direction is a
/// different traversal.
fn same_traversal(
    first: &OpposingTraversalObservation,
    second: &OpposingTraversalObservation,
) -> bool {
    first.facility == second.facility
        && first.movement == second.movement
        && first.direction == second.direction
}

/// The opposing-traversal interval one body's observed state opens, or `None`
/// when the body carries no interval.
///
/// The boundaries are the contract's own, read from the immutable observation
/// and the compiled scenario:
///
/// - the object's **extent** is the facility traversal's compiled reference
///   `[0, length]`; a facility without a reference path has no extent and can
///   carry no interval;
/// - an `either` object has no rule direction, so it can carry no interval;
/// - the interval records a traversal whose direction is **against** the
///   object's rule direction, which is its authored nominal direction: nominal
///   travel is not counted at all, and a rejected decision therefore creates no
///   interval;
/// - the record's perceived rule, reason, and affected movement are the
///   decision's own when this traversal is the opposing option a recorded
///   decision selected, and are otherwise derived from the compiled policy, so
///   a traversal the decision did not select still carries an inspectable
///   reason.
fn traversal_record(
    scenario: &CompiledScenario,
    agents: &AgentStore,
    index: usize,
    tick: u64,
    time_s: f64,
) -> Option<OpposingTraversalObservation> {
    let state = agents.route_state[index]?;
    let facility = scenario.facility(state.facility)?;
    let length_m = facility.reference()?.geometry().length();
    if !(0.0..=length_m).contains(&state.s_m) {
        return None;
    }
    let rule_direction = match facility.nominal_direction() {
        NominalDirection::Forward => MovementDirection::Forward,
        NominalDirection::Reverse => MovementDirection::Reverse,
        NominalDirection::Either => return None,
    };
    let direction = travel_direction(agents.direction[index]);
    if direction == rule_direction {
        return None;
    }
    let decision = state.wrong_way_decision.filter(|decision| {
        decision.option == WrongWayOption::Opposing && decision.facility == state.facility
    });
    let (perceived_rule, reason, movement) = match decision {
        Some(decision) => (decision.perceived_rule, decision.reason, decision.movement),
        None => {
            let perceived_rule = scenario
                .traversal_policy(state.mode_template, state.facility, agents.movement[index])
                .and_then(|policy| policy.nominal_effect());
            (
                perceived_rule,
                opposing_reason(perceived_rule),
                agents.movement[index],
            )
        }
    };
    let violating = !matches!(
        perceived_rule,
        Some(PermissionEffect::Permit) | Some(PermissionEffect::Obligate)
    );
    Some(OpposingTraversalObservation {
        agent: AgentId::from_index(index),
        facility: state.facility,
        movement,
        direction,
        nominal_direction: facility.nominal_direction(),
        perceived_rule,
        reason,
        violating,
        start_tick: tick,
        end_tick: tick,
        start_time_s: time_s,
        end_time_s: time_s,
    })
}

/// Online opposing-traversal interval tracking for one run.
///
/// Fed once per tick from the integrated state by
/// [`Simulation`](crate::Simulation); read through
/// [`Simulation::opposing_traversal_tracker`](crate::Simulation::opposing_traversal_tracker).
/// Fields are private: the pass only borrows state, and no caller can inject an
/// interval a tick did not produce. See [`traversal_record`] for the boundaries
/// and the module card for the decision the record comes from.
///
/// An interval closes exactly once, on the first later observed step that leaves
/// the extent, leaves the rule direction, or enters a different object, and a
/// despawn closes it the same way: the agent is no longer observed on its
/// object. A run that ends with an interval still open closes it through
/// [`Self::close_open`], whose close time is the interval's last observed tick,
/// the run's final simulation time.
#[derive(Debug, Default)]
pub struct OpposingTraversalTracker {
    /// Open intervals, ascending by agent, each carrying its last observed tick
    /// and time.
    open: BTreeMap<AgentId, OpposingTraversalObservation>,
    /// Intervals opened on the tick just observed.
    opened: Vec<OpposingTraversalObservation>,
    /// Intervals closed on the tick just observed.
    closed: Vec<OpposingTraversalObservation>,
    /// Every interval closed so far, in close order: ascending close tick, then
    /// ascending agent.
    intervals: Vec<OpposingTraversalObservation>,
}

impl OpposingTraversalTracker {
    /// Observe one integrated tick: open, extend, or close every agent's
    /// opposing-traversal interval.
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
        let time_s = SimTime::from_tick(tick, step).seconds();
        self.opened.clear();
        self.closed.clear();
        let mut seen: BTreeSet<AgentId> = BTreeSet::new();
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            let agent = AgentId::from_index(index);
            let Some(traversal) = traversal_record(scenario, agents, index, tick, time_s) else {
                continue;
            };
            seen.insert(agent);
            if let Some(open) = self.open.get_mut(&agent)
                && same_traversal(open, &traversal)
            {
                open.end_tick = tick;
                open.end_time_s = time_s;
                continue;
            }
            match self.open.insert(agent, traversal.clone()) {
                Some(previous) => {
                    self.opened.push(traversal);
                    self.close_interval(previous, tick, time_s);
                }
                None => self.opened.push(traversal),
            }
        }
        let closing: Vec<AgentId> = self
            .open
            .keys()
            .filter(|agent| !seen.contains(agent))
            .copied()
            .collect();
        for agent in closing {
            if let Some(open) = self.open.remove(&agent) {
                self.close_interval(open, tick, time_s);
            }
        }
    }

    /// The intervals that opened on the tick just observed.
    pub fn opened(&self) -> &[OpposingTraversalObservation] {
        &self.opened
    }

    /// The intervals that closed on the tick just observed.
    pub fn closed(&self) -> &[OpposingTraversalObservation] {
        &self.closed
    }

    /// Every interval closed so far, in close order.
    ///
    /// An interval still open when the run's last tick has been observed is
    /// absent here; [`Self::close_open`] closes it at run end.
    pub fn intervals(&self) -> &[OpposingTraversalObservation] {
        &self.intervals
    }

    /// Close every interval still open at the run's end.
    ///
    /// The run ending is the contract's last close boundary and its close time
    /// is the final simulation time, which is the tick the interval was last
    /// observed on. A run-end closure emits no event, exactly as a run-end
    /// close-pass closure does: no tick remains to carry one, so the closed
    /// interval is read from here. The closures are appended in
    /// `(close tick, agent)` order after the tick closures, and closing twice is
    /// impossible because the interval is removed.
    pub fn close_open(&mut self) {
        let mut closed: Vec<OpposingTraversalObservation> =
            std::mem::take(&mut self.open).into_values().collect();
        closed.sort_by_key(|interval| (interval.end_tick, interval.agent));
        self.intervals.extend(closed);
    }

    /// Close one interval at `tick`, recording it in both close surfaces.
    fn close_interval(&mut self, open: OpposingTraversalObservation, tick: u64, time_s: f64) {
        let closed = OpposingTraversalObservation {
            end_tick: tick,
            end_time_s: time_s,
            ..open
        };
        self.closed.push(closed.clone());
        self.intervals.push(closed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context comfortably inside both thresholds: the saving is 40 s against
    /// a 10 s minimum, the density is 10 per km against a 40 per km maximum,
    /// and the non-compliance threshold is `0.5 * (1 - 0.0) = 0.5`.
    ///
    /// A test that wants a threshold tie overrides the one field it ties.
    fn base() -> WrongWayInputs {
        WrongWayInputs {
            opposing_connected: true,
            nominal_direction: NominalDirection::Forward,
            nominal_remaining_length_m: 400.0,
            nominal_expected_speed_mps: 5.0,
            opposing_remaining_length_m: 400.0,
            opposing_expected_speed_mps: 10.0,
            observed_opposing_density_per_km: 10.0,
            permission: None,
            compliance: 0.0,
            desired_speed_mps: 10.0,
            policy: WrongWayPolicySource {
                min_time_saving_s: 10.0,
                max_opposing_density_per_km: 40.0,
                urgency: 0.5,
            },
            facility: FacilityId::from_index(3),
            movement: Some(MovementId::from_index(7)),
        }
    }

    /// The draw a test uses to select the opposing option in `base`, and the
    /// draw that keeps the nominal one.
    const ACCEPTING_DRAW: f64 = 0.25;
    const REFUSING_DRAW: f64 = 0.75;

    /// Draw values spanning the whole `[0, 1)` domain, used to prove a
    /// rejection ignores the draw.
    const ALL_DRAWS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 0.999_999_999_999_999_9];

    /// A connected opposing traversal a `prohibit` statement forbids: an
    /// opposing option physically exists, so the decision reaches the draw.
    fn prohibited_but_connected() -> WrongWayInputs {
        let mut inputs = base();
        inputs.permission = Some(PermissionEffect::Prohibit);
        inputs
    }

    /// A physically disconnected opposing traversal: no opposing option exists.
    fn disconnected() -> WrongWayInputs {
        let mut inputs = base();
        inputs.opposing_connected = false;
        inputs
    }

    /// Evaluate the keyed-draw pipeline for one declaration order, returning
    /// each agent's decision keyed by its stable id so two orders are
    /// comparable regardless of evaluation order.
    fn evaluate(
        seed: u64,
        order: &[(AgentId, WrongWayInputs)],
    ) -> Vec<(AgentId, WrongWayDecision)> {
        let mut results: Vec<(AgentId, WrongWayDecision)> = order
            .iter()
            .map(|(agent, inputs)| (*agent, decide(*inputs, maneuver_draw(seed, *agent, 0))))
            .collect();
        results.sort_by_key(|(agent, _)| agent.get());
        results
    }

    #[test]
    fn reason_labels_are_the_contract_codes() {
        let expected = [
            "no_opposing_path",
            "no_nominal_direction",
            "insufficient_time_saving",
            "opposing_density_too_high",
            "compliant_choice",
            "noncompliant_choice",
            "legal_permission",
            "legal_obligation",
        ];
        let labels: Vec<&str> = WrongWayReason::ALL.iter().map(|r| r.label()).collect();
        assert_eq!(
            labels.len(),
            expected.len(),
            "the closed set and its labels must agree"
        );
        for label in &labels {
            assert!(
                label.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{label} is not a stable lowercase snake_case code"
            );
        }
        for want in expected {
            assert!(labels.contains(&want), "missing contract code {want}");
        }
    }

    #[test]
    fn a_disconnected_opposing_traversal_rejects_without_a_draw() {
        let mut inputs = base();
        inputs.opposing_connected = false;
        for draw in ALL_DRAWS {
            let decision = decide(inputs, draw);
            assert_eq!(decision.reason, WrongWayReason::NoOpposingPath);
            assert_eq!(decision.option, WrongWayOption::Nominal);
        }
    }

    #[test]
    fn an_either_nominal_direction_has_no_opposing_option() {
        let mut inputs = base();
        inputs.nominal_direction = NominalDirection::Either;
        for draw in ALL_DRAWS {
            let decision = decide(inputs, draw);
            assert_eq!(decision.reason, WrongWayReason::NoNominalDirection);
            assert_eq!(decision.option, WrongWayOption::Nominal);
        }
        // `Reverse` is a real rule direction, so the nominal option is simply
        // the reverse traversal and the opposing one is forward.
        inputs.nominal_direction = NominalDirection::Reverse;
        assert_eq!(
            decide(inputs, REFUSING_DRAW).reason,
            WrongWayReason::CompliantChoice
        );
    }

    #[test]
    fn a_missing_opposing_path_outranks_a_missing_nominal_direction() {
        let mut inputs = base();
        inputs.opposing_connected = false;
        inputs.nominal_direction = NominalDirection::Either;
        assert_eq!(
            decide(inputs, REFUSING_DRAW).reason,
            WrongWayReason::NoOpposingPath
        );
    }

    #[test]
    fn rejections_carry_the_context_they_read() {
        let mut inputs = base();
        inputs.observed_opposing_density_per_km = 99.0;
        let decision = decide(inputs, ACCEPTING_DRAW);
        assert_eq!(decision.reason, WrongWayReason::OpposingDensityTooHigh);
        assert_eq!(decision.option, WrongWayOption::Nominal);
        assert_eq!(decision.facility, FacilityId::from_index(3));
        assert_eq!(decision.movement, Some(MovementId::from_index(7)));
        assert_eq!(decision.time_saving_s, 40.0);
        assert_eq!(decision.opposing_density_per_km, 99.0);
        assert_eq!(decision.urgency, 0.5);
        assert_eq!(decision.compliance, 0.0);
    }

    #[test]
    fn the_saving_threshold_is_inclusive() {
        // 200 / 5 - 150 / 5 = 40 - 30 = 10 s exactly at the authored minimum.
        let cases = [
            (10.0, None),
            (9.0, Some(WrongWayReason::InsufficientTimeSaving)),
        ];
        for (saving, rejection) in cases {
            let mut inputs = base();
            inputs.nominal_remaining_length_m = 200.0;
            inputs.opposing_remaining_length_m = 150.0 + (10.0 - saving) * 5.0;
            inputs.opposing_expected_speed_mps = 5.0;
            assert_eq!(inputs.time_saving_s(), saving, "the case must be exact");
            for draw in ALL_DRAWS {
                let decision = decide(inputs, draw);
                match rejection {
                    Some(reason) => {
                        assert_eq!(decision.reason, reason, "saving {saving}");
                        assert_eq!(decision.option, WrongWayOption::Nominal);
                    }
                    None => assert_ne!(
                        decision.reason,
                        WrongWayReason::InsufficientTimeSaving,
                        "saving {saving} is at the minimum, which stays eligible"
                    ),
                }
            }
        }
    }

    #[test]
    fn the_density_threshold_is_inclusive() {
        let cases = [
            (40.0, None),
            (41.0, Some(WrongWayReason::OpposingDensityTooHigh)),
        ];
        for (density, rejection) in cases {
            let mut inputs = base();
            inputs.observed_opposing_density_per_km = density;
            for draw in ALL_DRAWS {
                let decision = decide(inputs, draw);
                match rejection {
                    Some(reason) => {
                        assert_eq!(decision.reason, reason, "density {density}");
                        assert_eq!(decision.option, WrongWayOption::Nominal);
                    }
                    None => assert_ne!(
                        decision.reason,
                        WrongWayReason::OpposingDensityTooHigh,
                        "density {density} is at the maximum, which stays eligible"
                    ),
                }
            }
        }
    }

    #[test]
    fn the_saving_is_checked_before_the_density() {
        let mut inputs = base();
        inputs.nominal_remaining_length_m = 100.0;
        inputs.opposing_remaining_length_m = 300.0;
        inputs.observed_opposing_density_per_km = 90.0;
        assert!(inputs.time_saving_s() < inputs.policy.min_time_saving_s);
        assert!(
            inputs.observed_opposing_density_per_km > inputs.policy.max_opposing_density_per_km
        );
        assert_eq!(
            decide(inputs, ACCEPTING_DRAW).reason,
            WrongWayReason::InsufficientTimeSaving
        );
    }

    #[test]
    fn a_selected_nominal_option_is_a_compliant_choice() {
        let decision = decide(base(), REFUSING_DRAW);
        assert_eq!(decision.option, WrongWayOption::Nominal);
        assert_eq!(decision.reason, WrongWayReason::CompliantChoice);
    }

    #[test]
    fn a_selected_opposing_option_is_reasoned_by_the_permission() {
        let cases = [
            (None, WrongWayReason::NoncompliantChoice),
            (
                Some(PermissionEffect::Prohibit),
                WrongWayReason::NoncompliantChoice,
            ),
            (
                Some(PermissionEffect::Permit),
                WrongWayReason::LegalPermission,
            ),
            (
                Some(PermissionEffect::Obligate),
                WrongWayReason::LegalObligation,
            ),
        ];
        for (permission, reason) in cases {
            let mut inputs = base();
            inputs.permission = permission;
            let decision = decide(inputs, ACCEPTING_DRAW);
            assert_eq!(decision.option, WrongWayOption::Opposing, "{permission:?}");
            assert_eq!(decision.reason, reason, "{permission:?}");
            assert_eq!(decision.perceived_rule, permission);
        }
    }

    #[test]
    fn the_draw_tie_selects_the_nominal_option() {
        // (urgency, compliance) pairs whose non-compliance threshold is exact.
        let cases = [(0.5, 0.0, 0.5), (1.0, 0.0, 1.0), (0.5, 0.5, 0.25)];
        for (urgency, compliance, threshold) in cases {
            let mut inputs = base();
            inputs.policy.urgency = urgency;
            inputs.compliance = compliance;
            let tie = decide(inputs, threshold);
            assert_eq!(tie.option, WrongWayOption::Nominal, "{threshold}");
            assert_eq!(tie.reason, WrongWayReason::CompliantChoice);
            let below = decide(inputs, threshold - 0.01);
            assert_eq!(below.option, WrongWayOption::Opposing, "{threshold}");
            assert_eq!(below.reason, WrongWayReason::NoncompliantChoice);
        }
    }

    #[test]
    fn a_fully_compliant_agent_keeps_the_nominal_option() {
        for urgency in [0.0, 0.25, 0.5, 1.0] {
            let mut inputs = base();
            inputs.compliance = 1.0;
            inputs.policy.urgency = urgency;
            for draw in ALL_DRAWS {
                let decision = decide(inputs, draw);
                assert_eq!(
                    decision.option,
                    WrongWayOption::Nominal,
                    "urgency {urgency}"
                );
                assert_eq!(decision.reason, WrongWayReason::CompliantChoice);
            }
        }
    }

    #[test]
    fn a_zero_urgency_agent_keeps_the_nominal_option() {
        let mut inputs = base();
        inputs.policy.urgency = 0.0;
        for compliance in [0.0, 0.5, 1.0] {
            inputs.compliance = compliance;
            for draw in ALL_DRAWS {
                let decision = decide(inputs, draw);
                assert_eq!(decision.option, WrongWayOption::Nominal);
                assert_eq!(decision.reason, WrongWayReason::CompliantChoice);
            }
        }
    }

    #[test]
    fn a_zero_compliance_agent_accepts_every_draw_at_full_urgency() {
        let mut inputs = base();
        inputs.compliance = 0.0;
        inputs.policy.urgency = 1.0;
        inputs.permission = Some(PermissionEffect::Permit);
        for draw in ALL_DRAWS {
            let decision = decide(inputs, draw);
            assert_eq!(decision.option, WrongWayOption::Opposing, "draw {draw}");
            assert_eq!(decision.reason, WrongWayReason::LegalPermission);
        }
    }

    #[test]
    fn the_decision_is_deterministic_for_the_same_inputs() {
        for draw in ALL_DRAWS {
            assert_eq!(decide(base(), draw), decide(base(), draw), "draw {draw}");
        }
    }

    #[test]
    fn the_maneuver_draw_is_a_reproducible_unit_interval_value() {
        for seed in [0u64, 7, u64::MAX] {
            for agent in [AgentId::from_index(0), AgentId::from_index(5)] {
                for ordinal in [0u32, 1, 9] {
                    let value = maneuver_draw(seed, agent, ordinal);
                    assert_eq!(value, maneuver_draw(seed, agent, ordinal), "reproducible");
                    assert!((0.0..1.0).contains(&value), "{value} is out of [0, 1)");
                }
            }
        }
    }

    #[test]
    fn the_maneuver_draw_is_keyed_by_seed_agent_and_ordinal() {
        let first = maneuver_draw(7, AgentId::from_index(3), 0);
        assert_ne!(first, maneuver_draw(8, AgentId::from_index(3), 0), "seed");
        assert_ne!(first, maneuver_draw(7, AgentId::from_index(4), 0), "agent");
        assert_ne!(
            first,
            maneuver_draw(7, AgentId::from_index(3), 1),
            "ordinal"
        );
    }

    #[test]
    fn unrelated_agents_cannot_change_each_others_draws() {
        let seed = 11;
        let a = AgentId::from_index(1);
        let b = AgentId::from_index(2);

        let a_first = maneuver_draw(seed, a, 0);
        let a_second = maneuver_draw(seed, a, 1);
        // Interleave B's whole stream between A's draws.
        let b_values: Vec<f64> = (0..8)
            .map(|ordinal| maneuver_draw(seed, b, ordinal))
            .collect();
        let b_before: Vec<f64> = (0..8)
            .map(|ordinal| maneuver_draw(seed, b, ordinal))
            .collect();

        assert_eq!(a_first, maneuver_draw(seed, a, 0), "A's first draw moved");
        assert_eq!(a_second, maneuver_draw(seed, a, 1), "A's second draw moved");
        assert_eq!(b_values, b_before, "B's stream moved");
        assert_ne!(
            a_first,
            maneuver_draw(seed, b, 0),
            "agents share a substream"
        );
    }

    #[test]
    fn a_reversed_declaration_order_does_not_change_a_decision() {
        let seed = 5;
        let forward = [
            (AgentId::from_index(3), base()),
            (AgentId::from_index(1), prohibited_but_connected()),
            (AgentId::from_index(2), disconnected()),
        ];
        let mut reversed = forward;
        reversed.reverse();
        assert_eq!(evaluate(seed, &forward), evaluate(seed, &reversed));
        // The same keyed pipeline on the same order is stable across runs.
        assert_eq!(evaluate(seed, &forward), evaluate(seed, &forward));
    }

    #[test]
    fn a_disconnected_opposing_option_rejects_before_any_draw() {
        let inputs = disconnected();
        assert_eq!(
            inputs.physical_rejection(),
            Some(WrongWayReason::NoOpposingPath)
        );
        // No draw selects the opposing option: the rejection precedes the draw.
        for draw in ALL_DRAWS {
            let decision = decide(inputs, draw);
            assert_eq!(
                decision.reason,
                WrongWayReason::NoOpposingPath,
                "draw {draw}"
            );
            assert_eq!(decision.option, WrongWayOption::Nominal);
        }
    }

    #[test]
    fn an_either_direction_is_a_physical_rejection() {
        let mut inputs = base();
        inputs.nominal_direction = NominalDirection::Either;
        assert_eq!(
            inputs.physical_rejection(),
            Some(WrongWayReason::NoNominalDirection)
        );
    }

    #[test]
    fn a_connected_prohibited_traversal_still_takes_the_draw() {
        let inputs = prohibited_but_connected();
        assert_eq!(
            inputs.physical_rejection(),
            None,
            "a legal prohibition does not remove the opposing option"
        );
        let selected = decide(inputs, ACCEPTING_DRAW);
        assert_eq!(selected.option, WrongWayOption::Opposing);
        assert_eq!(selected.reason, WrongWayReason::NoncompliantChoice);
        // The same prohibition is a compliant choice when the draw keeps the
        // nominal option: legality never becomes a physical rejection.
        let kept = decide(inputs, REFUSING_DRAW);
        assert_eq!(kept.option, WrongWayOption::Nominal);
        assert_eq!(kept.reason, WrongWayReason::CompliantChoice);
    }
}

/// The opposing-traversal interval's own boundaries, driven from a hand-built
/// agent store so the geometric and rule boundaries the kernel's own runs do
/// not reach — an `either` object, a body centre outside the extent, a run that
/// ends mid-interval — are covered directly.
#[cfg(test)]
mod interval_tests {
    use super::*;
    use tangle_model::{ModeTemplateId, PathId, parse_scenario_source_v2};

    use crate::agent::{AgentInit, AgentMode, RouteState};

    /// The straight reference every fixture travels: the world x axis.
    const REFERENCE_LENGTH_M: f64 = 100.0;

    /// The fixed step the tracker tests advance by.
    const DT: f64 = 0.05;

    /// A version-2 document with one 100 m reference and one `rider` facility
    /// over it, whose authored nominal direction is `nominal` and whose
    /// permission statements are `permissions`. No demand: a test drives the
    /// tracker by hand.
    fn scenario(nominal: &str, permissions: &str) -> CompiledScenario {
        let source = parse_scenario_source_v2(&format!(
            "{{ schema_version: 2, id: 'wrong_way_interval', \
             coordinate_system: {{ x: 'east_m', y: 'north_m' }}, \
             paths: [ {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, \
             {{ x: {REFERENCE_LENGTH_M}, y: 0.0 }} ] }} ], \
             portals: [ {{ id: 'entry', path: 'guide', end: 'start', width_m: 3.0 }}, \
             {{ id: 'exit', path: 'guide', end: 'end', width_m: 3.0 }} ], \
             regions: [ {{ id: 'band', points: [ {{ x: 0.0, y: -1.5 }}, \
             {{ x: {REFERENCE_LENGTH_M}, y: -1.5 }}, \
             {{ x: {REFERENCE_LENGTH_M}, y: 1.5 }}, {{ x: 0.0, y: 1.5 }} ] }} ], \
             facilities: [ {{ id: 'rider_facility', region: 'band', \
             reference_path: 'guide', width_m: 3.0, nominal_direction: '{nominal}', \
             access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared', \
             speed_policy: {{ limit_mps: null }} }} ], \
             movements: [ {{ id: 'through', from: 'entry', to: 'exit', \
             path: 'guide', priority: 0, direction: 'forward' }} ], \
             mode_templates: [ {{ id: 'rider', \
             body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }}, \
             radius_m: {{ min: 0.35, max: 0.35 }} }}, motion: 'single_body_wheeled', \
             tactics: [ 'follow', 'stop', 'yield' ], \
             access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either', \
             speed_policy: {{ limit_mps: null }} }}, occupancy: 'operator_only', \
             profiles: {{ speed_mps: {{ min: 6.0, max: 6.0 }}, \
             max_accel_mps2: {{ min: 1.2, max: 1.2 }}, \
             comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }}, \
             time_gap_s: {{ min: 1.0, max: 1.0 }}, \
             steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }}, \
             lateral_clearance_m: {{ min: 0.3, max: 0.3 }}, \
             compliance: {{ min: 1.0, max: 1.0 }} }} }} ], \
             permissions: [{permissions}] }}"
        ))
        .expect("the document is version 2");
        CompiledScenario::compile_v2(source).expect("the scenario compiles")
    }

    /// One rider body on the single facility at `s_m`, travelling `direction`.
    fn push_rider(store: &mut AgentStore, compiled: &CompiledScenario, s_m: f64, direction: f64) {
        let facility = FacilityId::from_index(0);
        let geometry = compiled
            .facility(facility)
            .and_then(|facility| facility.reference())
            .expect("the facility has a compiled reference")
            .geometry();
        let position = geometry.position_at(s_m);
        let route = RouteState::project(
            ModeTemplateId::from_index(0),
            facility,
            geometry,
            position,
            direction,
            None,
            None,
        );
        store.push(AgentInit {
            mode: AgentMode::Vehicle,
            path: PathId::from_index(0),
            distance_m: s_m,
            speed_mps: 5.0,
            position,
            heading_rad: 0.0,
            body_length_m: 1.8,
            body_width_m: 0.7,
            direction,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: Some(route),
        });
    }

    /// The rider's route state, for a test that changes what the tracker reads.
    fn route(store: &mut AgentStore) -> &mut RouteState {
        store.route_state[0]
            .as_mut()
            .expect("the rider carries route state")
    }

    fn observe(
        tracker: &mut OpposingTraversalTracker,
        store: &AgentStore,
        compiled: &CompiledScenario,
        tick: u64,
    ) {
        tracker.observe(store, compiled, tick, Seconds::from_secs(DT));
    }

    #[test]
    fn the_interval_records_only_a_traversal_against_the_rule_direction() {
        let compiled = scenario("forward", "");
        let mut store = AgentStore::default();
        push_rider(&mut store, &compiled, 50.0, 1.0);
        let mut tracker = OpposingTraversalTracker::default();

        // Nominal travel is not counted at all.
        observe(&mut tracker, &store, &compiled, 1);
        assert!(tracker.opened().is_empty());
        assert!(tracker.intervals().is_empty());

        // The opposing traversal opens one interval, against the authored
        // nominal direction and inside the reference extent.
        store.direction[0] = -1.0;
        observe(&mut tracker, &store, &compiled, 2);
        assert_eq!(tracker.opened().len(), 1);
        let opened = &tracker.opened()[0];
        assert_eq!(opened.agent, AgentId::from_index(0));
        assert_eq!(opened.facility, FacilityId::from_index(0));
        assert_eq!(opened.direction, MovementDirection::Reverse);
        assert_eq!(opened.nominal_direction, NominalDirection::Forward);
        assert_eq!(opened.perceived_rule, None);
        assert_eq!(opened.reason, WrongWayReason::NoncompliantChoice);
        assert!(opened.violating, "no statement makes it legal");
        assert_eq!(opened.start_tick, 2);
        assert_eq!(opened.movement, None);

        // A body centre outside the extent carries no interval.
        route(&mut store).s_m = REFERENCE_LENGTH_M + 1.0;
        observe(&mut tracker, &store, &compiled, 3);
        assert!(tracker.opened().is_empty());
        assert_eq!(tracker.closed().len(), 1, "leaving the extent closes it");
        assert_eq!(tracker.closed()[0].end_tick, 3);
        assert_eq!(tracker.intervals(), tracker.closed());
    }

    #[test]
    fn an_either_object_carries_no_interval() {
        let compiled = scenario("either", "");
        let mut store = AgentStore::default();
        push_rider(&mut store, &compiled, 50.0, -1.0);
        let mut tracker = OpposingTraversalTracker::default();
        observe(&mut tracker, &store, &compiled, 1);
        assert!(
            tracker.opened().is_empty(),
            "an object with no rule direction has no opposing traversal"
        );
    }

    #[test]
    fn a_permitted_opposing_traversal_is_recorded_legal() {
        let compiled = scenario(
            "forward",
            "{ id: 'contraflow', kind: 'nominal_direction', holder: 'rider', \
             target: 'rider_facility', effect: 'permit' }",
        );
        let mut store = AgentStore::default();
        push_rider(&mut store, &compiled, 50.0, -1.0);
        let mut tracker = OpposingTraversalTracker::default();
        observe(&mut tracker, &store, &compiled, 1);
        let opened = &tracker.opened()[0];
        assert_eq!(opened.perceived_rule, Some(PermissionEffect::Permit));
        assert_eq!(opened.reason, WrongWayReason::LegalPermission);
        assert!(
            !opened.violating,
            "a permitted opposing traversal is legal, never a violation"
        );
    }

    #[test]
    fn a_recorded_decision_supplies_the_recorded_rule_reason_and_movement() {
        let compiled = scenario("forward", "");
        let mut store = AgentStore::default();
        push_rider(&mut store, &compiled, 50.0, -1.0);
        store.movement[0] = None;
        route(&mut store).wrong_way_decision = Some(crate::wrong_way::WrongWayDecision {
            perceived_rule: Some(PermissionEffect::Obligate),
            option: WrongWayOption::Opposing,
            reason: WrongWayReason::LegalObligation,
            facility: FacilityId::from_index(0),
            movement: Some(MovementId::from_index(0)),
            time_saving_s: 12.0,
            opposing_density_per_km: 3.0,
            urgency: 0.5,
            compliance: 0.0,
        });
        let mut tracker = OpposingTraversalTracker::default();
        observe(&mut tracker, &store, &compiled, 1);
        let opened = &tracker.opened()[0];
        assert_eq!(opened.perceived_rule, Some(PermissionEffect::Obligate));
        assert_eq!(opened.reason, WrongWayReason::LegalObligation);
        assert!(!opened.violating, "an obligated traversal is legal");
        assert_eq!(
            opened.movement,
            Some(MovementId::from_index(0)),
            "the entry leaves the movement route behind, so the decision is \
             the only surviving spelling of the connector it entered on"
        );
    }

    #[test]
    fn a_direction_change_and_a_new_object_close_and_reopen_the_interval() {
        let compiled = scenario("forward", "");
        let mut store = AgentStore::default();
        push_rider(&mut store, &compiled, 50.0, -1.0);
        let mut tracker = OpposingTraversalTracker::default();
        observe(&mut tracker, &store, &compiled, 1);
        assert_eq!(tracker.opened().len(), 1);

        // The traversal becomes the rule direction: the interval closes and no
        // new one opens.
        store.direction[0] = 1.0;
        observe(&mut tracker, &store, &compiled, 2);
        assert!(tracker.opened().is_empty());
        assert_eq!(tracker.closed().len(), 1);
        assert_eq!(tracker.closed()[0].end_tick, 2);

        // Turning round again is a new interval on the same object.
        store.direction[0] = -1.0;
        observe(&mut tracker, &store, &compiled, 3);
        assert_eq!(tracker.opened().len(), 1);
        assert_eq!(tracker.opened()[0].start_tick, 3);
        assert_eq!(tracker.intervals().len(), 1, "the second one is still open");

        // A despawn is a close: the body is no longer observed on its object.
        store.alive[0] = false;
        observe(&mut tracker, &store, &compiled, 4);
        assert_eq!(tracker.closed().len(), 1);
        assert_eq!(tracker.closed()[0].end_tick, 4);
        assert_eq!(tracker.intervals().len(), 2);
    }

    #[test]
    fn a_run_that_ends_mid_interval_closes_it_at_its_last_observed_tick() {
        let compiled = scenario("forward", "");
        let mut store = AgentStore::default();
        push_rider(&mut store, &compiled, 50.0, -1.0);
        let mut tracker = OpposingTraversalTracker::default();
        for tick in 1..=3 {
            observe(&mut tracker, &store, &compiled, tick);
        }
        assert!(tracker.intervals().is_empty(), "still open");

        tracker.close_open();
        let intervals = tracker.intervals();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].start_tick, 1);
        assert_eq!(
            intervals[0].end_tick, 3,
            "the run ending closes at the final simulation time"
        );
        assert!((intervals[0].duration_s() - 2.0 * DT).abs() < 1e-12);
        assert!(
            tracker.closed().is_empty(),
            "a run-end close emits no event"
        );
    }
}
