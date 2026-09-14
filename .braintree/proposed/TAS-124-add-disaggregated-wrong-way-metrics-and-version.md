---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Add disaggregated wrong-way metrics and version bumps.
next: Accumulate wrong-way distance, duration, exposure, encounters, and conflicts by dimension and bump the needed format versions.
---

Parent [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].

# Outcome

Wrong-way metrics are disaggregated through the normal output pipeline with
definition-versioned distance, duration, exposure, encountered agents, and
conflicts.

# Done when

- Metrics accumulate distance, duration, exposure, unique encounters, and
  conflicts by mode, movement, facility, and participant pair with explicit
  applicability; nominal travel is not counted.
- Metric and trajectory format versions are bumped where their consumers need a
  signal, and aggregate and compare preserve the new dimensions.
- Focused tests cover collision and near-miss linkage and inapplicable modes.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Extends [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the aggregate
metrics and version bumps; the interval is the sibling slice.
