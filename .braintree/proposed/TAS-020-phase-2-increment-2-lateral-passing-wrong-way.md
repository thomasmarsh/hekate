---
context_rev: 1
priority: P1
updated: 2026-09-12T15:12:03Z
summary: Increment 2 adds continuous lateral motion, overtaking, close-pass evidence, and contextual wrong-way travel.
next: Implement continuous lateral targeting with the following/preparing/committed/returning maneuver state machine and bounded steering.
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
