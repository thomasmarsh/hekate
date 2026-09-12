---
context_rev: 1
priority: P1
updated: 2026-09-12T15:46:06Z
summary: Phase 1 Increment 2 turns portals into demand, gives cars an IDM longitudinal controller with stop-line, signal, leader, queue, and exit behavior, and records reproducible contextual red-light decisions.
next: Add portal demand generation, route assignment, profile sampling, and safe spawn admission on the compiled scenario.
---

# Outcome

Per `PHASE_1_PLAN.md` Increment 2, cars become demand-driven agents with
documented, reproducible longitudinal control:

- Portal demand generation, route assignment, physical and behavior profile
  sampling, and safe spawn admission.
- Path-distance tracking and a documented IDM-based longitudinal controller.
- Stop-line, signal, leader, following, queue, and exit behavior.
- A contextual red-light decision over signal state, distance, speed, urgency,
  and compliance profile.
- Decision-reason records visible in the inspector.

Increment 1 compiles the general scenario primitives; this increment adds
behavior over them. Pedestrian bodies and mixed interaction are Increment 3.

# Done when

- Cars obey acceleration, braking, and speed bounds and never overlap in the
  controlled car-following benchmark.
- Saturated demand produces stable queues rather than unbounded spawn overlap.
- Profile distributions and red-light decisions are reproducible and draw only
  from their named random streams (`demand`, `profile`, `compliance`,
  `perception`).
- Unit and scenario tests cover green/yellow/red boundaries and deterministic
  tie-breaking.
- Every state-affecting decision can be traced to a recorded decision reason.

# Context

Area [[IDX-001-tangle]].

Builds on Increment 1, [[TAS-026-phase-1-increment-1-general-scenario-foundation]],
for the compiled portals, guide paths, movements, rule, and signal primitives
this increment drives. Record it as a pinned `Depends on` edge at Increment 1's
current `context_rev` when this increment takes the frontier and that node is
resolved; until then it stays a proposed node behind that work.

Follows `PHASE_1_PLAN.md` numeric choices: `f64` in the kernel, `glam::DVec2`,
portable ChaCha streams derived from the root seed and stable agent IDs, and a
single-threaded state-affecting tick. `PHASE_2_PLAN.md` owns schema version 2, so
no Phase 2 field is added here.
