//! Documented contextual wrong-way (opposing-traversal) decision.
//!
//! `docs/schema-v2-contract.md` *Contextual wrong-way traversal* fixes this
//! decision. The nominal direction, the permitted direction, and the physically
//! possible direction stay separate, and a wrong-way traversal is an ordinary
//! traversal the ordinary routing, steering, collision, yielding, and event
//! paths carry. This module owns only the decision over the observer-stage
//! context the contract fixes there: it returns the perceived rule, the
//! selected option, a reason code, the affected facility and movement, and the
//! context values it read. It adds no visibility-error or perception subsystem,
//! chooses no trajectory, routes nothing, and emits nothing.
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

use tangle_model::{
    FacilityId, MovementId, NominalDirection, PermissionEffect, WrongWayPolicySource,
};

use crate::agent::AgentId;
use crate::rng::{STREAM_MANEUVER, derive_stream, uniform01};

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
        let reason = match inputs.permission {
            Some(PermissionEffect::Permit) => WrongWayReason::LegalPermission,
            Some(PermissionEffect::Obligate) => WrongWayReason::LegalObligation,
            Some(PermissionEffect::Prohibit) | None => WrongWayReason::NoncompliantChoice,
        };
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
