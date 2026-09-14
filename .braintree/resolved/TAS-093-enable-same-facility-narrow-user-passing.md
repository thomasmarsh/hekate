---
context_rev: 1
priority: P1
updated: 2026-09-14T04:41:03Z
summary: Enable bicycle and scooter passing of slower narrow users on one shared facility.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

A bicycle or scooter with pass capability can overtake a slower narrow leader
inside one shared continuous-width facility using the common predictor, claim,
state-machine, steering, collision, and longitudinal-control paths.

# Done when

- Eligibility requires capability, applicable permission, route benefit,
  sufficient usable width, visible slower leader, target clearance, and a
  feasible horizon; each rejected precondition has an inspectable reason.
- The target offset selects a deterministic side from geometry and policy, the
  passing agent claims a body corridor, clears the leader, and returns without
  changing route identity or teleporting.
- Leader/follower longitudinal behavior remains active throughout; a pass is
  not implemented as speed or position scripting.
- Bicycle-passes-scooter, scooter-passes-bicycle, no-width, prohibited, and
  no-benefit focused tests cover both travel directions and stable decisions.
- No named bicycle or scooter branch enters shared controller, query, collision,
  or output modules.

# Context

Gated on [[TAS-091-resolve-gap-claims-and-maneuver-transitions]]. Owns the
narrow passing eligibility/target tactic seam and focused tests. Do not add
motor-vehicle overtaking, connector transitions, close-pass metrics, or broad
acceptance fixtures.

# Result

TAS-093 is complete. A bicycle or scooter with the compiled `pass` capability
overtakes a slower narrow leader inside one shared continuous-width facility
through the ordinary predictor, claim, state-machine, steering, collision, and
longitudinal paths, in **both travel directions**. No public event is emitted
(TAS-100 owns that).

**Changed files** (uncommitted when this node resolved):

- `crates/tangle-sim/src/agent.rs` — `RouteState` gains `mode_template`, so a
  tactical leaf reads the mode's compiled capabilities and policy without ever
  naming a mode id, and `pass_reason`, the inspectable eligibility outcome.
- `crates/tangle-sim/src/stage.rs` — the public `ManeuverReason` code set (the
  selection code `SlowerLeader` and the rejection codes `Capability`,
  `NoPermission`, `NoBenefit`, `InsufficientWidth`, `NoCorridor`) and the
  resolved `PassSide`, each with a stable lowercase label.
- `crates/tangle-sim/src/narrow.rs` — `pass_side(policy, left_clearance_m,
  right_clearance_m)`, the deterministic side rule (policy first, then the
  greater predicted clearance, an exact `most_clearance` tie resolving left).
- `crates/tangle-sim/src/sim.rs` — the narrow pass tactic
  (`record_narrow_pass_intents`, `narrow_pass_decision`, `narrow_pass_leader`,
  `narrow_pass_clearance`, `pass_candidate`), the
  `Simulation::narrow_pass_reason` accessor, and the passed-body exclusion from
  `nearest_leader` while a maneuver is `committed`, `returning`, or `aborted`.
- `crates/tangle-sim/src/lib.rs` — re-exports `ManeuverReason` and `PassSide`.
- `crates/tangle-sim/tests/narrow_passing.rs` (new, 7 tests) — drives the whole
  tactic through the public seam.

**Reverse-direction defect fixed in this run.** With the slice as inherited,
forward passes completed but every reverse pass was rejected as `NoCorridor`:
`predict_maneuver` reported `Infeasible { limiting: BandEdge }` for the
candidate. Root cause: a wheeled agent's stored `heading_rad` is its
**reference-tangent** heading (the Phase 1 convention — `predicted_bodies`
turns that tangent into a velocity with the travel sign), while
`bounded_steering_step` and the prediction corridor are expressed in the
agent's **travel** heading frame. For a forward traveller the two coincide; for
a reverse traveller they differ by `pi`, so the candidate motion was integrated
from the wrong heading, spiralled out of the facility band, and lost the band
edge. The fix converts between the two frames only at the steering boundary:
`travel_heading` when building the predicted body in `predict_maneuver` and the
`SteeringRequest` in `command_motion`, and `reference_heading` when
`advance_physics` stores a `RouteSteering` result back. Reverse travel-frame
geometry (target offset, resolved side, and `passed_body_cleared`) was already
correct; only the heading frame was wrong. The change is inert for forward
travellers (`travel_heading`/`reference_heading` are identities), so Phase 1 and
Increment 1 behavior is unchanged.

**Focused-suite speed.** The inherited fixture ran `TICKS = 1500` with a 4 s
lateral horizon (~118 s for the 7 tests). The pass returns to `following` by
tick ~790, so `TICKS` is now `900`, and the fixture horizon is 2 s, which keeps
the predictor's reach over the catch-up while cutting the per-candidate
prediction cost several-fold. The focused suite now runs in ~6.7 s.

**Done-when -> test mapping** (public seam unless noted):

- *Eligibility requires capability, applicable permission, route benefit,
  sufficient usable width, a visible slower leader, target clearance, and a
  feasible horizon; each rejected precondition has an inspectable reason*:
  `narrow_pass_decision` checks the preconditions in a fixed order and
  `record_narrow_pass_intents` writes the outcome to `RouteState::pass_reason`,
  read back by `Simulation::narrow_pass_reason`. `narrow_passing.rs`
  `a_narrow_facility_rejects_the_pass_as_insufficient_width`
  (`insufficient_width`), `a_prohibited_facility_rejects_the_pass_as_no_permission`
  (`no_permission`), and `a_leader_with_no_route_benefit_rejects_the_pass_as_no_benefit`
  (`no_benefit`) pin the rejected codes; `stage.rs`
  `eligibility_reasons_and_sides_have_stable_labels` pins every code and side.
- *The target offset selects a deterministic side from geometry and policy, the
  passing agent claims a body corridor, clears the leader, and returns without
  changing route identity or teleporting*: `narrow.rs`
  `the_pass_side_reads_policy_then_geometry_with_a_left_tie` pins the side rule;
  the corridor and lifecycle are the ordinary `attempt_maneuver` /
  `committed_plan` / `passed_body_cleared` paths. `narrow_passing.rs`
  `a_bicycle_passes_a_slower_scooter` and `a_scooter_passes_a_slower_bicycle`
  require the full `following -> preparing -> committed -> returning ->
  following` edge sequence with no abort and assert no agent's per-step
  displacement exceeds the free-flow bound (no teleport).
- *Both travel directions*: `narrow_passing.rs`
  `the_pass_side_is_the_travel_frame_side_in_both_directions` runs the same
  fixture forward and reverse and requires a completed pass and a
  `slower_leader` selection in each.
- *Leader/follower longitudinal behavior remains active throughout; a pass is
  not speed or position scripting*: the pass reuses `nearest_leader`, the IDM
  controller, and the same steering step as every other vehicle; the no-benefit
  fixture shows the follower following with no maneuver. Passes are driven only
  by the eligibility intent, never a scripted speed or position.
- *Stable decisions*: `narrow_passing.rs` `the_pass_decision_is_stable_for_a_seed`
  reproduces the same edges and reasons for a fixed seed.
- *No named bicycle or scooter branch enters shared controller, query,
  collision, or output modules*: eligibility reads only compiled components
  (`TacticalCapability::Pass`, `template.lateral()`, the traversal policy, the
  facility width, and measured leader geometry). `grep -rn '"bicycle"\|"scooter"'
  crates/tangle-sim/src/` returns only pre-existing module-card prose, no code
  branch.

**Acceptance** (exact commands, run from the repository root with the slice in
the working tree):

- `cargo test -p tangle-sim` — 336 passed, 0 failed across 26 suites;
  `narrow_passing` 7 passed, 0 failed in ~7 s.
- `cargo test --workspace` — 839 passed, 0 failed across 81 suites; no golden
  regenerated and no trace hash changed.
- `scripts/check-dependency-direction.sh` — `dependency direction OK` (exit 0).
- `braintree check` while TAS-093 was `proposed` — `graph check: passed (154
  nodes)` (exit 0).

**Friction** (for the session FBK; no FBK node was created): the stale heading
convention was invisible until a reverse lateral maneuver existed, and the
error surfaced as a prediction verdict (`BandEdge`) rather than a heading
error, so the diagnostic path ran through the predictor before the frame
mismatch was obvious. Also `cargo clippy --workspace --all-targets --all-features
-- -D warnings` failed on the inherited slice (`!(a < b)` on `f64`, whose left
operand is a non-finite-capable desired speed) even though `cargo test` was
green. Improvement: whichever API stores a wheeled heading should name the
frame it stores in its doc comment (`reference tangent` vs `travel`), because
two callers in this crate read the same field with two opposite assumptions and
nothing detects the divergence until reverse lateral motion exists.
