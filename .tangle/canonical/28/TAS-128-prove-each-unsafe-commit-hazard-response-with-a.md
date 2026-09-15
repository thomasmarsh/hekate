---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T04:10:33Z
summary: Prove each unsafe-commit hazard response with a falsification probe.
next: Verify each hazard case and the falsification probe against the Done-when mapping, then resolve the node.
---

Parent [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]].

# Outcome

Adversarial tests prove a committed maneuver responds to each newly unsafe
corridor with the documented bounded brake, abort, hold, or return action, and a
probe shows the suite fails when a response is removed.

# Done when

- Front intrusion, rear intrusion, disappearing connector, narrowing corridor, and
  blocked return each reach the required state and bounded motion response.
- A falsification probe changes the tie-break key or removes one hazard response
  and demonstrates that the suite fails for the intended reason.
- No case teleports, overlaps silently, exceeds a motion limit, or crosses a
  forbidden boundary without the TAS-100 event fact.

# Result

Test-only slice in `crates/hekate-sim/src/sim.rs` (`mod tests`): no production
change, no fixture, and no file under `scenarios/`. The hazards live at the
in-crate seam because a hazard body must be placed exactly (a classified front,
rear, or side fact), which the public API cannot inject.

Every new case steps through one harness that asserts the contract's limits on a
response — the world step inside the motion limit, no overlapping body pair, no
boundary crossed, and no kernel position cap — and that the recorded response is
the documented ordered response read from the step's own predicted swept
clearance, so the clause list the probe mutates is this kernel's behavior.

- `a_committed_front_intrusion_brakes_within_the_comfort_bound_and_holds`: the
  front fact is the limiting object; the response is the brake-and-hold inside
  the profile's comfortable braking.
- `a_committed_rear_intrusion_brakes_within_the_comfort_bound_and_holds`: the
  rear fact is the limiting object; the same response.
- `a_committed_corridor_narrowed_by_a_side_body_brakes_and_holds_then_aborts`:
  the side fact is the limiting object; a closest approach inside the
  brake-and-hold window holds, and one below the policy minimum aborts with
  `clearance_lost`.
- `a_committed_crossing_whose_connector_disappears_aborts_without_crossing`: a
  committed change of lane whose destination carries no compiled adjacency aborts
  with `corridor_infeasible`, never crosses, and settles back to `following`.
- `a_returning_leg_blocked_by_a_body_holds_within_the_motion_limits`: the
  returning leg holds its offset with no transition, no overlap, and no position
  cap, and settles once the corridor clears.
- `removing_the_abort_response_from_the_kernel_is_falsified_by_the_hazard_suite`:
  the probe. It removes the checked-in abort clause
  (`sim.rs (Simulation::committed_plan)`) from the kernel's own source text,
  checks the mutant keeps the brake-and-hold clause, and shows the documented
  response aborts on the narrowing case's own predicted fact while the mutant
  only brakes, so the required `aborted` state is unreachable and the suite fails
  for the intended reason. Run by hand: with the abort clause deleted from
  `sim.rs`, the narrowing case and the probe both fail on the recorded-response
  assertion (`Brake` against `Abort(ClearanceLost)`); restoring the clause
  returns the suite to green.

Existing coverage this node maps instead of duplicating: the front intrusion's
abort below the policy minimum
(`a_committed_maneuver_aborts_below_the_policy_minimum`), the front intrusion's
hold (`a_committed_maneuver_brakes_within_the_comfort_bound_and_holds`), and the
blocked return and blocked aborted leg
(`a_blocked_return_leg_holds_and_re_decides_until_the_corridor_clears`,
`a_blocked_aborted_leg_holds_until_the_corridor_clears`).

Verified: `cargo fmt --all --check` clean; `cargo clippy -p hekate-sim
--all-targets -- -D warnings` clean; `cargo test -p hekate-sim` 252 lib tests
plus every suite, 0 failed; `cargo test -p hekate-sim --test maneuver_lifecycle`
6 passed; `scripts/check-dependency-direction.sh` `dependency direction OK`. No
seam defect was found, so no production line and no landed seam changed.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Extends [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]]; reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the hazard
responses and the falsification probe.
