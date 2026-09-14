---
context_rev: 1
priority: P1
updated: 2026-09-14T03:30:52Z
summary: Resolve simultaneous gap claims and the complete lateral maneuver state machine.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

The tactical stage advances exactly one explicit maneuver lifecycle per eligible
agent and resolves simultaneous claims on shared corridor space with a stable,
documented priority and AgentId tie-break.

# Done when

- following enters preparing only with a target and candidate corridor;
  preparing commits only after a successful claim; committed reaches returning
  after clearing the passed obstacle; returning reaches following at its target
  position; preparing or committed can enter aborted under the fixed policy.
- Claims are collected from one immutable observation, arbitrated as a batch
  before commands, and cannot depend on mutable agent iteration order.
- Loss of predicted clearance after commitment produces the documented
  brake/hold/abort choice without revoking another winner mid-step.
- Timeouts and target disappearance have explicit deterministic transitions;
  all state transitions expose one record for later TAS-100 event emission.
- Unit tests cover the legal transition table, rejected claims, same-gap ties,
  reversed insertion order, target despawn, timeout, and committed hazard.

# Context

Gated on [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] and
[[TAS-090-predict-maneuver-corridors-and-clearance]]. Owns the tactical-choice
and claim-resolution seams in crates/tangle-sim plus focused tests. It supplies
mechanism only; tactic eligibility for the passing families is TAS-093 through
TAS-095.

# Result

TAS-091 is complete. The tactical stage advances exactly one explicit maneuver
lifecycle per eligible agent and arbitrates each step's shared-corridor claims
as one immutable batch before any command, with a stable documented tie-break.
Mechanism only: tactic eligibility for the passing families stays TAS-093
through TAS-095.

**Changed files** (uncommitted when this node resolved):

- `crates/tangle-sim/src/stage.rs` (uncommitted) — the public
  `LateralManeuverRequest`, `ManeuverEdge`, `ManeuverAbortReason`,
  `ManeuverTransition`, and `SETTLE_TOLERANCE_M`; the `CorridorClaim` record,
  `arbitrate_claims`, and `beats`.
- `crates/tangle-sim/src/agent.rs` (uncommitted) — `RouteState` gains `intent`,
  `corridor`, `passed_body`, `pre_maneuver_offset_m`, `state_since`,
  `braking`, `hold_since`, and `settled_since`, seeded to no-maneuver values at
  spawn.
- `crates/tangle-sim/src/sim.rs` (uncommitted) — `request_lateral_maneuver`,
  the four-phase `resolve_maneuvers` pass, and `StepOutput::transitions`.
- `crates/tangle-sim/src/lib.rs` (uncommitted) — re-exports the new seam.
- `crates/tangle-sim/tests/maneuver_lifecycle.rs` (new, 5 tests) — the public
  seam driven through `request_lateral_maneuver` and `StepOutput::transitions`.

**Public seam and the maneuver claim winner key.** The entry point is
`Simulation::request_lateral_maneuver(agent, LateralManeuverRequest { target_offset_m, passed_body }) -> bool`;
each step reports its decided edges once through `StepOutput::transitions()`.
In `stage::beats`, the winner of two conflicting claims is the first in the
lexicographic key **(1) committed before preparing; (2) smaller
`entry_distance_m`, the claimant's own-direction distance to the contested
corridor entry; (3) smaller `AgentId`** — every clause reads only immutable
tick-start inputs, so request insertion order cannot change a winner.

**Done-when -> test mapping** (all existing; no test was added by this
finishing run, because every bullet already maps):

- *following -> preparing only with a target and candidate corridor; preparing
  commits only after a successful claim; committed -> returning on clearing the
  passed obstacle; returning -> following at its offset; preparing or committed
  -> aborted under the fixed policy*: `sim.rs`
  `a_maneuver_follows_the_legal_transition_table_to_following` (the full table,
  one record per edge) plus the abort tests below; the public seam is pinned by
  `maneuver_lifecycle.rs`
  `a_requested_maneuver_prepares_then_commits_with_one_record_per_edge` and
  `an_aborted_claimant_returns_to_following_at_its_own_offset`.
- *Claims collected from one immutable observation, arbitrated as a batch
  before commands, independent of mutable iteration order*: `sim.rs`
  `a_batch_is_decided_independently_of_request_order`,
  `a_losing_claimant_aborts_with_the_rejected_reason`; `maneuver_lifecycle.rs`
  `the_same_claimant_wins_whichever_order_the_requests_arrive_in` (public API,
  forward vs reversed insertion order).
- *Loss of predicted clearance after commitment gives the documented
  brake/hold/abort choice without revoking another winner mid-step*: `sim.rs`
  `a_committed_maneuver_brakes_within_the_comfort_bound_and_holds`,
  `a_committed_maneuver_aborts_below_the_policy_minimum`,
  `a_committed_hold_timeout_aborts_the_maneuver`,
  `losing_clearance_does_not_revoke_another_winner_mid_step`; the committed -
  beats - preparing clause is `stage.rs`
  `a_committed_claim_beats_a_preparing_claim`.
- *Timeouts and target disappearance are explicit deterministic transitions;
  every transition exposes one record for TAS-100*: `sim.rs`
  `a_preparing_hold_timeout_aborts_the_maneuver`,
  `a_committed_hold_timeout_aborts_the_maneuver`,
  `a_disappearing_target_aborts_the_maneuver`; the one-record-per-edge
  assertion is in `a_maneuver_follows_the_legal_transition_table_to_following`;
  `stage.rs` `edges_and_reasons_have_stable_labels` fixes the labels TAS-100
  will name.
- *Unit tests cover the legal transition table, rejected claims, same-gap ties,
  reversed insertion order, target despawn, timeout, and committed hazard*:
  `sim.rs` `a_maneuver_follows_the_legal_transition_table_to_following`
  (table), `a_losing_claimant_aborts_with_the_rejected_reason` and
  `the_request_seam_refuses_an_agent_that_cannot_maneuver` (rejected claims),
  `a_same_gap_tie_is_won_by_the_lower_agent_id` (same-gap tie),
  `a_batch_is_decided_independently_of_request_order` (reversed insertion
  order), `a_disappearing_target_aborts_the_maneuver` (despawn),
  `a_preparing_hold_timeout_aborts_the_maneuver` and
  `a_committed_hold_timeout_aborts_the_maneuver` (timeout), and
  `a_committed_maneuver_brakes_within_the_comfort_bound_and_holds` /
  `a_committed_maneuver_aborts_below_the_policy_minimum` (committed hazard).
  `stage.rs` adds the winner-key clauses directly:
  `a_same_gap_tie_is_won_by_the_lower_agent_id`,
  `the_smaller_entry_distance_wins_between_preparing_claims`,
  `arbitration_is_invariant_to_insertion_order`,
  `disjoint_corridors_are_both_granted`,
  `a_corridor_within_the_target_clearance_conflicts`,
  `a_single_claim_is_always_granted`, and
  `the_entry_distance_is_measured_along_the_travel_direction`.

**Acceptance** (exact commands, run from the repository root with the slice in
the working tree):

- `cargo test -p tangle-sim` — 327 passed, 0 failed across 25 suites, including
  183 `src/lib.rs` unit tests and 5 `maneuver_lifecycle` tests.
- `cargo test --workspace` — 830 passed, 0 failed across 80 suites; no golden
  regenerated and no trace hash changed.
- `scripts/check-dependency-direction.sh` — `dependency direction OK` (exit 0).
- `braintree check` while TAS-091 was `proposed` — `graph check: passed (154
  nodes)` (exit 0).

**TAS-088 initial-state reconciliation.** Not applicable: this slice did not
implement a TAS-088 reconciliation. The new `RouteState` fields are seeded to
no-maneuver defaults in the existing spawn initializer, so TAS-088's route-state
spawn path already carries every field this leaf reads.

**Friction** (for the session FBK; no FBK node was created): the node's
`# Done when` bullet 5 already demanded a reversed-insertion-order unit test at
two levels, and the public API test that pins it lives only in the integration
file rather than the in-module tests the bullet names; the finishing check had to
inspect both levels to confirm the bullet was met rather than trusting the
in-module list. Improvement: state the seam (unit vs public API) each required
case must cover, so a reviewer does not re-derive it.
