---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Record the wrong-way interval and rule-state trajectory.
next: Open and close the wrong-way interval at the rule boundaries and expose rule state on sampled trajectories.
---

Parent [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].

# Outcome

Wrong-way travel is visible as one reasoned violation interval bounded by the
rule geometry and as optional rule state on sampled trajectories.

# Done when

- The interval opens and closes at TAS-083 geometric and rule boundaries and
  records perceived rule, decision reason, facility, and affected movement IDs;
  rejected decisions create no false travel interval.
- Sampled trajectories expose optional rule state and opposing direction; sparse
  transitions remain event records and survive immutable run and replay.
- Focused tests cover clear interval, abort-before-entry, facility handoff, route
  exit, replay, and inapplicable modes.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Extends [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the interval
and trajectory rule state; do not add the aggregate metrics.
