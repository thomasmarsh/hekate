---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Complete narrow longitudinal tactics and compiled-facility route completion.
---

Parent [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]].

# Outcome

Complete the narrow longitudinal behavior TAS-077 wired: route one narrow agent
through the shared four stages so it accelerates, follows a leader, holds a stop
line, yields at a signal, and completes its route along a compiled facility, and
prove command envelopes and body bounds hold in each state. Narrow modes select
and follow a compiled facility route and complete it with no named-mode branch in
shared code.

# Done when

- A test proves command envelopes and body bounds are respected for one narrow
  agent accelerating, following a leader, holding a stop line, yielding at a
  signal, and completing its route along a compiled facility.
- Narrow modes select and follow a compiled facility route and complete it, with
  no named-mode branch in shared code.
- `cargo test -p hekate-sim` and `cargo test --workspace` pass with every Phase 1
  golden trace unchanged.

# Context

The remainder of [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]],
split because TAS-077 landed the spawn wiring, narrow model, profile sampling,
and model cards but the longitudinal-tactic suite and facility route completion
did not fit one session. TAS-077 delivered: narrow agents spawn from their
compiled mode template through `AgentFamily::WheeledCapsule` family dispatch, the
narrow wheeled model (`crate::narrow`) reached through the controller seam,
`sample_narrow_profile`, the bicycle and scooter model cards, and the
no-shared-mode-branch guard.

What remains is behavior, not wiring: following, stop lines, and signal yields
already flow through the shared stages for any path-following profile, so the
work is a checked-in narrow fixture carrying a leader, a stop line, and a signal,
plus compiled-facility route selection and completion, and the single Done-when
test over them. Read [[TAS-068-controller-stage-interfaces]],
[[TAS-074-compile-version-2-facility-and-connector-shapes]], and
[[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]].

# Result

The narrow longitudinal behavior TAS-077 wired is proven end to end on a
checked-in fixture, so both behavior `Done when` bullets hold and the third holds
unchanged. No production code changed.

## Fixture

`crates/hekate-sim/tests/fixtures/narrow_longitudinal_v2.json5` (new): one
`bicycle` capsule template with constant ranges (so the sampled profile is
seed-independent), a compiled `bikeway` facility whose `reference_path` is the
same `guide` path the demand's `through` movement follows, an authored
`stop_line_m` of 34 m on that movement, and a fixed-time signal on the movement
(12 s red then 120 s green). Demand is a 1800/h bicycle rate from the entry
portal, so a queue forms at the red line and several narrow agents ride the
facility route.

## Compiled facility route selection and completion

The narrow template's demand selects the compiled `through` movement; the test
asserts that movement's path is the compiled facility's reference path and that
the facility permits the narrow mode template (`CompiledFacility::permits_mode`).
Each step it checks every narrow agent's world position against the compiled
reference geometry (`CompiledReferencePath::project`): the lateral offset is zero
and the projected arc equals the reported path distance, so the agent follows the
compiled facility route exactly. A narrow `Despawned { reason: ExitedPath }`
event on that reference path is the route completion. The mechanism is shared:
route selection, the movement path, and the four stages are the ones a car uses,
and no production module branches on a mode name — the TAS-077 guard
`narrow_mode_no_branch.rs` still covers `sim.rs`, `stage.rs`, `controller.rs`,
`control.rs`, `narrow.rs`, `event.rs`, `metrics.rs`, and `safety.rs`.

## Test

`crates/hekate-sim/tests/narrow_longitudinal.rs` (new),
`a_narrow_agent_accelerates_follows_holds_yields_and_completes_a_compiled_facility_route`,
60 s at seed 20_260_913. Every tick, for every narrow agent, it asserts:

- **Body bounds** — `body_kind == Capsule`, and the reported body length and
  width equal the sampled narrow profile's `length_m` and `2 * radius_m`.
- **Command envelope** — speed in `[0, v0]`, implied per-step acceleration
  `<= max_accel_mps2`, and deceleration `<= comfortable_brake_mps2`; the run ends
  with `emergency_cap_steps() == 0`, so the anti-overlap backstop never braked
  beyond the model bound.

and accumulates the five behaviors (all observed; peak live narrow count 10):

- **Accelerating** — a narrow agent's positive per-step change.
- **Following a leader** — two narrow agents on the same path with a front-to-rear
  gap `<= 8 m` where the follower is no faster than its leader and slower than
  its own desired speed; every body pair on a shared path stays non-overlapping.
- **Holding a stop line** — a narrow agent at rest (speed `< 0.25 m/s`) with its
  front bumper within `0.5 m` of the authored `stop_line_m`.
- **Yielding at a signal** — a narrow agent whose recorded signal decision is
  `SignalAction::Stop` under a non-green head.
- **Completing its route** — a narrow agent despawns `ExitedPath` on the compiled
  facility reference path.

## Files changed

Added `crates/hekate-sim/tests/fixtures/narrow_longitudinal_v2.json5` and
`crates/hekate-sim/tests/narrow_longitudinal.rs`. No `hekate-sim` production
module, `hekate-model` accessor, `crates/hekate-present/**`, `apps/**`,
`baselines/**`, `schemas/**`, `docs/**`, or other node file changed, and no Phase
1 golden trace, event, metric, or baseline changed.

## Acceptance

- `cargo test -p hekate-sim` — passes (147 lib + all suites, including the new
  `narrow_longitudinal` and the existing `narrow_spawn` and
  `narrow_mode_no_branch`).
- `cargo test --workspace` — passes; Phase 1 golden traces unchanged
  (`cargo test -p hekate-cli --test golden_trace --test baseline --test
  migration_regression` passes: 4 + 2 + 4).
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `cargo fmt -p hekate-sim -- --check` and
  `RUSTFLAGS="-D warnings" cargo clippy -p hekate-sim --all-targets --all-features`
  — clean.
- `tangle check` — `graph check: passed (120 nodes)` while this node was still
  in `proposed/`.

## Note for the coordinator

"Compiled-facility route selection" is a property of the scenario, not a runtime
branch: the demand selects a compiled movement whose path is the facility's
reference path, and the shared stages follow it. If a later increment needs the
kernel to *choose* among several candidate facilities, that is new routing work
outside this node.

Post-move note: after this file moves to `resolved/`, `tangle check`
transiently reports `next-resolved-node` on
[[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]] because TAS-077's
`next` still names this node; the coordinator advances TAS-077's `next`. No
parent or sibling node file was edited and no child node was created.
