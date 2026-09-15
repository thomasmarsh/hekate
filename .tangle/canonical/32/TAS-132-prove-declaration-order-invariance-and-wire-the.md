---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Prove declaration-order invariance and wire the matrix.
next: Show reversed facility and reference declarations preserve the physical outcome and replace only the planned matrix entries.
---

Parent [[TAS-107-check-in-contextual-wrong-way-fixtures]].

# Outcome

Reversing facility and reference declarations preserves the physical wrong-way
outcome, and the fixtures replace only their planned benchmark-matrix entries.

# Done when

- Reversing facility and reference declarations does not change the physical
  outcome after stable IDs are accounted for.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no interaction disposition or tolerance is widened.
- CLI validate and run plus cargo test --workspace pass at required presets.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Extends [[TAS-107-check-in-contextual-wrong-way-fixtures]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns declaration
invariance and the matrix wiring; the core fixtures are the sibling slice.
