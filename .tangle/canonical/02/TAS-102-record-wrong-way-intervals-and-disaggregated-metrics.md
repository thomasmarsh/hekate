---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T03:08:00Z
summary: Record wrong-way rule intervals, state, exposure, encounters, and conflicts.
---

Parent [[TAS-099-increment-2-events-metrics-and-output]].

# Outcome

Wrong-way travel is visible as one reasoned violation interval and as
definition-versioned distance, duration, exposure, encountered-agent, and
conflict metrics disaggregated through the normal output pipeline.

# Done when

- The interval opens and closes at TAS-083's geometric/rule boundaries and
  records perceived rule, decision reason, facility and affected movement IDs;
  rejected decisions create no false travel interval.
- Sampled trajectories expose optional rule state and opposing direction;
  sparse transitions remain event records and survive immutable run/replay.
- Metrics accumulate distance, duration, exposure, unique encounters, and
  conflicts by mode, movement, facility, and participant pair with explicit
  applicability; nominal travel is not counted.
- Metric and trajectory format versions are bumped where their consumers need a
  signal, and aggregate/compare preserve the new dimensions.
- Focused tests cover clear interval, abort-before-entry, facility handoff,
  collision/near-miss linkage, route exit, replay, and inapplicable modes.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] and
[[TAS-100-version-the-maneuver-event-and-trace-surface]]. Owns rule-state
snapshot/trajectory fields, wrong-way interval tracking, metric
definitions/aggregation, artifact consumers, and tests. Do not add viewer
overlays or acceptance scenarios.

# Slices

- [[TAS-123-record-the-wrong-way-interval-and-rule-state-tra]] Wrong-way interval and rule-state trajectory.
- [[TAS-124-add-disaggregated-wrong-way-metrics-and-version]] Disaggregated wrong-way metrics and version bumps.

# Result

Both slices resolved. [[TAS-123-record-the-wrong-way-interval-and-rule-state-tra]]
records the rule-state trajectory and the interval at the geometric/rule
boundaries with perceived rule, decision reason, facility, and movement IDs, and
[[TAS-124-add-disaggregated-wrong-way-metrics-and-version]] lands the five
families across aggregate and compare under metric definition v3 (`DEF-006`
rationale for the unchanged `TRAJECTORY_FORMAT_VERSION`). Focused tests cover
clear interval, abort-before-entry, facility handoff, collision/near-miss
linkage, route exit, replay, and inapplicable modes; the required Done-when
evidence is recorded on each slice.
