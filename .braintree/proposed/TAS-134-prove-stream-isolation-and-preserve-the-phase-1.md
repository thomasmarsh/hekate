---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Prove stream isolation and preserve the Phase 1 baseline.
next: Add the stream-isolation cases and a falsification probe while keeping Phase 1 baseline artifacts unchanged.
---

Parent [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]].

# Outcome

Adding an unrelated agent or demand stream cannot perturb another agent maneuver
or wrong-way draws, and the Phase 1 baseline artifacts are unchanged.

# Done when

- Stream-isolation tests add unrelated car and narrow demand and reversed
  declaration order without changing owned maneuver draws or unaffected agent
  traces.
- A falsification probe demonstrates the suite catches draw-order coupling or an
  unkeyed maneuver choice.
- Existing demand, profile, compliance, and perception streams and Phase 1
  baseline artifacts remain unchanged.

# Context

Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns stream isolation
and baseline preservation; reproducibility is the sibling slice.
