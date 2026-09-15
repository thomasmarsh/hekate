---
status: resolved
context_rev: 2
priority: P1
updated: 2026-09-15T11:30:39Z
summary: Add continuous lateral motion, passing evidence, and contextual wrong-way travel.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 2:

- Continuous lateral targeting, lane/facility transitions, gap prediction,
  maneuver commitment, abort, and return states, with lateral tactics
  specifying a target clearance and feasible horizon rather than teleporting
  between lane centers.
- Bicycle/scooter passing, motor-vehicle overtaking of narrow users, and
  configured lane changes around slower leaders.
- Close-pass observations and violations with exact clearance and
  relative-speed evidence, configurable clearance bands, and boundary or
  opposing-facility evidence.
- Contextual wrong-way route selection with ordinary routing, steering,
  collision, yielding, and event systems, plus perceived rule, decision reason,
  violation interval, and affected movement IDs.
- Viewer overlays for usable corridor, target offset, predicted gap, maneuver
  state, and wrong-way rule state.

# Done when

- No maneuver teleports, snaps laterally, crosses a forbidden boundary without
  an event, or exceeds motion limits.
- Analytic and fine-step fixtures bound the error in predicted and observed
  minimum clearance.
- Simultaneous gap claims resolve deterministically and unsafe commits follow
  the documented braking/abort policy.
- Wrong-way agents use normal routing, collision, and metric paths and cannot
  bypass an occupied opposing corridor.

Parent [[TAS-017-phase-2-mixed-traffic]].

# Execution structure

This node coordinates six independently acceptable workstreams, executed in
this order:

1. [[TAS-082-increment-2-authored-and-compiled-contract]] fixes and implements
   the authored and compiled Increment 2 policy surface.
2. [[TAS-087-continuous-lateral-motion-and-gap-machinery]] supplies
   route-relative state, bounded steering, prediction, and deterministic claims.
3. [[TAS-092-passing-and-lane-transition-behavior]] integrates the three
   required passing families and safe abort/return behavior.
4. [[TAS-096-contextual-wrong-way-travel]] selects and executes physically
   connected opposing traversal through ordinary systems.
5. [[TAS-099-increment-2-events-metrics-and-output]] records the versioned
   maneuver, close-pass, and wrong-way evidence.
6. [[TAS-103-increment-2-acceptance-evidence-and-presenters]] closes the
   analytic, fine-step, adversarial, reproducibility, performance, matrix, and
   viewer gates.

The leaves below those coordinators are the session-scoped handoff units. A
worker takes one leaf, preserves its exclusions and owned paths, records exact
evidence in that leaf, and does not roll up a coordinator unless explicitly
assigned that coordination write set.

# Constraints

- World pose remains collision and output truth; route coordinates are tactical
  state and must be projected back after integration.
- Shared interaction, collision, routing, event, and presenter code dispatches
  on components or physical families, never a named scenario or mode.
- Increment 1 longitudinal behavior and every Phase 1 baseline remain available.
  Any intentional output union change bumps its definition version and updates
  only the required goldens with a versioned explanation.
- No navigation mesh, balance/lean/fall model, sidewalk-riding special case,
  detailed visibility-error model, or mode-specific crate is in Increment 2.
- Every new scenario or model-card artifact names the repository-wide suite
  that enumerates its directory before the path is introduced.

# Result

All six workstreams resolved: [[TAS-082-increment-2-authored-and-compiled-contract]],
[[TAS-087-continuous-lateral-motion-and-gap-machinery]],
[[TAS-092-passing-and-lane-transition-behavior]],
[[TAS-096-contextual-wrong-way-travel]],
[[TAS-099-increment-2-events-metrics-and-output]], and
[[TAS-103-increment-2-acceptance-evidence-and-presenters]]. Their children carry the
Increment 2 acceptance evidence: no maneuver teleports or crosses a forbidden boundary
without an event ([[TAS-092-passing-and-lane-transition-behavior]],
[[TAS-104-bound-predicted-versus-executed-clearance]]); analytic and fine-step fixtures
bound predicted versus observed clearance (TAS-104); simultaneous gap claims resolve
deterministically under the documented braking/abort policy
([[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]]); wrong-way agents use
ordinary routing, collision, and metric paths and cannot bypass an occupied opposing
corridor ([[TAS-107-check-in-contextual-wrong-way-fixtures]]); and the viewer renders all
five overlays with backend parity ([[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]).
Five gates green at `083d6f8`: fmt, clippy `-D warnings`, workspace tests (1049 passed, 0
failed, 3 ignored), dependency direction, and `tangle check` (195 nodes).
