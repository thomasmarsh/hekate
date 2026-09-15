---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Enable component-driven motor-vehicle overtaking of narrow users.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

A single-body motor vehicle can pass a slower bicycle or scooter with lateral
displacement only when authored permission, body-aware geometry, visibility,
and predicted clearance allow it.

# Done when

- Eligibility and rejection reasons use body/motion/tactic/access components,
  not passenger_car, bicycle, or scooter identifiers.
- The predictor accounts for the motor vehicle box, narrow capsule, boundary or
  opposing-facility use, relative speed, front/rear traffic, and target
  clearance throughout displacement and return.
- Prohibited boundary crossing cannot occur silently; an allowed use of an
  opposing facility remains an ordinary connected transition with a claim.
- Focused tests cover safe pass, insufficient lateral room, insufficient rear
  gap, opposing occupancy, permission prohibition, and deterministic side
  choice.
- Existing longitudinal car following and vehicle-yielding fixtures remain
  unchanged when no overtake capability/policy is authored.

# Context

Gated on [[TAS-093-enable-same-facility-narrow-user-passing]]. Owns the
component-driven motor-over-narrow tactic seam and focused tests. Do not
implement generic adjacent-facility transition completion, events, or metrics.

# Result

TAS-094 is complete. A wheeled **box** motor mode that declares the compiled
`overtake` capability and authors a `lateral` policy overtakes a slower narrow
user inside one shared continuous-width facility through the ordinary
eligibility, target, predictor, claim, state-machine, steering, collision, and
longitudinal paths. The tactic is the same component-driven seam
[[TAS-093-enable-same-facility-narrow-user-passing]] landed, generalized from an
explicit `pass` to an explicit `pass` **or** `overtake`; no code path names
`passenger_car`, `bicycle`, or `scooter`, and no boundary-crossing special case
is introduced. No public event is emitted (TAS-100 owns that).

**Changed files** (uncommitted when this node resolved):

- `crates/hekate-sim/src/sim.rs` — the generalized lateral maneuver tactic (the
  seam renamed from `narrow_pass_*` to `maneuver_*`, one capability check
  accepting `Pass | Overtake`, the same deterministic target, and the
  `Simulation::lateral_maneuver_reason` accessor); the box-aware
  `route_state_for`/`try_admit` wiring that seeds a bounded-steering envelope
  from a wheeled mode's sampled lateral limits; and the Phase 2 guard that skips
  a route-state agent with no maneuver instead of requiring every route-state
  agent to carry a target clearance.
- `crates/hekate-sim/src/profile.rs` — `WheeledLateralLimits` and
  `sample_wheeled_lateral_limits`, which sample a wheeled box's three authored
  lateral parameters from its own compiled mode template.
- `crates/hekate-sim/src/narrow.rs` — `NarrowProfile::lateral_limits`, the
  capsule's projection onto the shared `WheeledLateralLimits`, so both families
  seed the envelope from one shape.
- `crates/hekate-sim/src/agent.rs` — `RouteState::pass_reason` renamed
  `maneuver_reason` for the shared seam.
- `crates/hekate-sim/src/stage.rs` — the `ManeuverReason::Capability` doc now
  names `Pass` or `Overtake`.
- `crates/hekate-sim/tests/motor_overtaking.rs` (new, 6 tests) — the focused
  suite through the public seam.
- `crates/hekate-sim/tests/narrow_passing.rs` — the one accessor rename
  (compile-forced closure).
- `crates/hekate-sim/src/sim.rs` and `src/profile.rs` `#[cfg(test)]` unit tests —
  the box route-state seed and the box lateral-limit sampler.
- `crates/hekate-model/src/mode_template.rs`, `src/components.rs`,
  `tests/increment2_compiled.rs` — **resolved-sibling closure fix**, see below.

**Resolved-sibling closure fix (defect named).** `compiled_profile` in
`crates/hekate-model/src/mode_template.rs` built `AgentBehaviorProfile::wheeled`
for **every** non-capsule `single_body_wheeled` body and then attached only
`lateral_accel_max_mps2`; a lateral **box** therefore compiled to a bundle that
validation required three lateral parameters for (`steering_rate_max_rad_s`,
`lateral_accel_max_mps2`, `lateral_clearance_m`) but that carried only one, so
`template.profile().steering_rate_max_rad_s()` and
`template.lateral_clearance_m()` were `None`/`0` for a box. The fix attaches the
box's steering rate and lateral clearance through the two new
`AgentBehaviorProfile::with_steering_rate_max_rad_s` / `with_lateral_clearance_m`
setters, mirroring `with_lateral_accel_max_mps2`, when the template declares
`lateral`. It is behavior-preserving: a template without `lateral` carries none
of them, and the capsule path is unchanged (`narrow_wheeled` already reads them,
so re-attaching is idempotent). No `context_rev` bump: no consumer assumption
changes, the compiler now matches validation. A focused test
`a_lateral_box_carries_all_three_lateral_profile_parameters` proves a lateral box
carries all three and a plain box carries none.

## Eligibility, reasons, and the deterministic side rule

`maneuver_decision` checks the preconditions in a fixed order and
`record_maneuver_intents` writes the outcome to `RouteState::maneuver_reason`,
read back by `Simulation::lateral_maneuver_reason`. Every read is a component, a
compiled policy value, or a measured geometric fact:

1. **capability** — `template.tactics().supports(Pass)` or `supports(Overtake)`
   (else `capability`); a mode with neither never qualifies;
2. **lateral policy** — the mode's compiled `lateral.target_clearance_m` and
   `horizon_s`, plus a sampled `bounded_steering` envelope (else `no_corridor` /
   `capability`);
3. **applicable permission** — the resolved `overtake` effect is not `Prohibit`
   and the facility names a passing side (else `no_permission`);
4. **route benefit** — the nearest visible same-direction leader on the facility
   is slower than the agent and inside `speed * horizon` (else `no_benefit`);
5. **deterministic target** — the side resolved by `narrow::pass_side` from the
   authored policy, then the offset `u = (W/2 + leader_u + leader_half) / 2`
   that equalizes the clearance to the leader and the clearance to the band
   edge. The `self_half_m` term scales the required room with the agent's own
   body width, so a wide motor box needs a wider facility; a `most_clearance`
   policy reads the predictor's swept clearance for a target on each side, an
   exact tie resolving left;
6. **sufficient usable width** — the balanced clearance reaches the target
   clearance and the target lies inside the usable corridor (else
   `insufficient_width`);
7. **feasible horizon** — the ordinary predictor's candidate corridor is
   feasible over the whole horizon (else `no_corridor`).

The selection reason is `SlowerLeader`; the renames give `ManeuverReason` codes
`capability`, `no_permission`, `no_benefit`, `insufficient_width`, `no_corridor`
their labels unchanged (`stage.rs`).

## Predictor facts

`predict_maneuver` already feeds the ordinary TAS-090 predictor the motor's own
**box** envelope (`query::agent_body`, re-oriented with the travel heading), the
facility band width (boundary), every other world body by exact shape (the
narrow capsule target, opposing traffic, and front/rear/side bodies), the bodies'
velocities (relative speed), and the mode's target clearance. The new fixtures
exercise it directly: a clear corridor past a slower narrow leader is feasible
and settles the box against the leader as a **front** fact; a wide body closing
from behind is an **insufficient rear gap** rejected on that body; a body
travelling the other way in the corridor the motor would occupy is an
**occupied opposing corridor** rejected on that body; and a target beyond the
usable corridor is rejected on the **band edge**.

## Boundary crossing

A prohibited boundary crossing cannot occur silently: the maneuver target is
bounded by the facility's usable interval, and `bounded_steering_step` returns
`None` rather than clipping, so `predict_maneuver` reports
`Infeasible { BandEdge }` and the tactic records `no_corridor` (or
`insufficient_width`) rather than displacing past the band. The leaf introduces
no boundary-crossing path of its own, so an allowed lateral use remains the
ordinary TAS-091 corridor claim. The **adjacent-facility / opposing-facility
transition** that would exercise that clause through an authoured adjacency is
`[[TAS-095-complete-lane-transitions-and-safe-aborts]]`'s by this node's
Context, and is not authored here.

## Done-when -> test mapping (public seam unless noted)

- *Eligibility and reasons use components, not mode identifiers*: `maneuver_decision`
  reads capability, the compiled lateral policy, the resolved traversal policy,
  the facility width, and measured leader geometry only;
  `tests/motor_overtaking.rs` drives a motor mode that authors `overtake` and
  `bicycle` drives the narrow user, while the kernel reads only their compiled
  components. `grep -n '"passenger_car"\|"bicycle"\|"scooter"'` over the
  non-test source of `sim.rs`, `stage.rs`, `profile.rs`, `agent.rs`, and
  `narrow.rs` finds fixtures only.
- *The predictor accounts for the box, the capsule, boundary/opposing use,
  relative speed, front/rear traffic, and target clearance*: `motor_overtaking.rs`
  `the_predictor_settles_the_motor_box_against_front_rear_and_opposing_bodies`
  and `a_target_outside_the_corridor_is_rejected_on_the_band_edge`.
- *Prohibited boundary crossing cannot occur silently; an allowed use of an
  opposing facility is an ordinary claim*: the boundary rejection is the
  `BandEdge` fixture above and the corridor bound in `bounded_steering_step`;
  the ordinary-claim clause is satisfied by construction (no boundary-crossing
  special case), with the adjacent-facility transition owned by
  [[TAS-095-complete-lane-transitions-and-safe-aborts]].
- *Focused tests cover safe pass, insufficient lateral room, insufficient rear
  gap, opposing occupancy, permission prohibition, deterministic side choice*:
  `motor_overtaking.rs` `a_motor_vehicle_overtakes_a_slower_bicycle`,
  `a_narrow_road_rejects_the_overtake_as_insufficient_width`,
  the rear/opposing predictor fixture above,
  `a_prohibited_road_rejects_the_overtake_as_no_permission`, and
  `the_overtake_side_is_deterministic_and_follows_the_policy` (which asserts a
  `left` policy commits to a positive-`d` target, a `right` policy to a
  negative-`d` target, and the same seed reproduces the edges and reasons).
  `narrow_passing.rs` still covers the capsule `pass` on the same seam.
- *Existing longitudinal car following and vehicle-yielding fixtures remain
  unchanged*: `cargo test --workspace` passes with `vehicle_flow`,
  `vehicle_yielding`, `longitudinal_control`, `narrow_*`, `synthetic_template_no_branch`,
  and the CLI golden suites unchanged; no golden was regenerated. The box path
  draws its lateral limits only when the mode authors `lateral`, so a box with
  no overtake capability or policy samples and behaves exactly as before.

## Acceptance

- `cargo test -p hekate-sim` — 344 passed, 0 failed across 27 suites
  (`motor_overtaking` 6 passed in ~12 s; lib 187).
- `cargo test --workspace` — 848 passed, 0 failed across 82 suites;
  `cargo test -p hekate-model` 168 passed; no golden regenerated.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `tangle check` while TAS-094 was `proposed` — `graph check: passed (154
  nodes)`.

## Friction (for the session FBK; no FBK node was created)

- Attempted: sample the box lateral limits where TAS-089 recorded the gap, in
  `profile.rs`. Friction: TAS-089's analysis was incomplete — `sample_profile`
  reads the scenario's `CompiledProfile`, which never carries a box's steering
  rate or clearance; the real loss is upstream, in `mode_template.rs`
  `compiled_profile`, which required three lateral parameters and kept one.
  Improvement: TAS-089's recorded gap should name the compiler, not
  `profile.rs`, as the place a box's lateral limits are lost.
- Attempted: run the fixture with a lateral motor and a non-lateral narrow user.
  Friction: `resolve_maneuvers` Phase 2 called `.expect("a maneuver state
  carries its mode's target clearance")` for **every** route-state agent, so any
  v2 scenario mixing a lateral mode with a lateral-incapable mode on a facility
  panicked. TAS-093 never hit it because its two narrow fixtures gave both modes
  a `lateral` object. Improvement: the contract or a test should state that a v2
  facility may carry lateral-incapable modes beside lateral ones, so the
  Phases-2/3 guards are read as the invariant they are.
- Attempted: keep the write set to `crates/hekate-sim`. Friction: the box
  lateral-mode defect is in `crates/hekate-model`, outside the delegated set;
  the coordinator authorized a tightly scoped compiler fix, which is recorded
  above as a resolved-sibling closure. Improvement: a leaf whose Done-when needs
  a compiled component should have its write set include the compiler that
  produces it, or name the resolved owner's closure explicitly up front.
