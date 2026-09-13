---
context_rev: 1
priority: P1
updated: 2026-09-13T16:10:21Z
summary: Compile mode templates into agent components behind four explicit controller stages.
next: [[TAS-068-controller-stage-interfaces]]
---

Parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]].

# Outcome

Per `PHASE_2_PLAN.md` Increment 0 (extension contract): shared kernel behavior
operates on composable capabilities and physical state, not named modes. This
node delivers the compiled component model (body, motion, tactical capability,
access, occupancy, social state), mode-template validation, the four
controller-stage interfaces, and a reusable model-card template.

# Done when

- Compiled agent data carries body, motion, tactical capability, access, occupancy, and social state as separate composable components.
- The compiler validates mode templates and rejects impossible combinations such as transit dwell without capacity or articulation on a holonomic body.
- An agent update is split into relevant-world query, tactical choice, motion control, and physical advance as explicit interfaces.
- A model-card template exists and both Phase 1 model cards conform to it.
- A synthetic template can alter dimensions, limits, and access without adding a named-mode branch to shared interaction code.

# Context

Decomposed just in time into direct children; this node stays open until every child is resolved or disposed and the criteria above hold.
