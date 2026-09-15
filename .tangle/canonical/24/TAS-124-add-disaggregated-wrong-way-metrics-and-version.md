---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T02:53:52Z
summary: Add disaggregated wrong-way metrics and version bumps.
next: Run the five-gate validation and resolve.
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

# Result

The five families land in metric definition v3. `RunMetricsRecorder` observes
each completed tick beside the event stream and the tracker, so
`RunMetricsArtifact` carries `wrong_way_intervals`, `wrong_way_distance_m`
(whole-step arc length against the rule direction), `wrong_way_duration_s`,
`wrong_way_exposure_agent_s` (agent-seconds of co-presence, the predicate
documented on `WrongWayValues`), `wrong_way_encounters` (distinct co-present
partners, deduped across intervals), and `wrong_way_conflicts` (contacting
collisions and entering near misses with an opposing participant), each over the
run and sliced by mode pair, movement, facility, participant pair, and perceived
rule with explicit applicability (nominal travel reaches no family).
`canonical_run_captured` closes the run's open opposing traversals beside the
close-pass intervals before the capture, so an interval still open at the final
tick is counted. `aggregate` and `compare` carry the families and every
dimension.

No version bump: `DEF-006` (`# Added under this same version`) and the bump table
in `docs/schema-v2-contract.md` add the wrong-way families under the existing
metric definition v3 and give `TRAJECTORY_FORMAT_VERSION` one bump for the whole
additive column union (TAS-088, extended by TAS-144). This slice adds no
trajectory column and no consumer needs a new signal, so
`METRIC_DEFINITION_VERSION == 3` and `TRAJECTORY_FORMAT_VERSION == 3` are
unchanged and asserted.

Focused evidence: the recorder derives each family from the trailer intervals
and the event stream, `tests/run_metrics.rs` covers accumulation over closed and
run-end-open intervals, the permitted rule dimension, the inapplicable no-policy
and `either` cases, and collision plus near-miss linkage, and
`tests/aggregate.rs` / `tests/compare.rs` / `tests/converge.rs` carry the new run
keys and dimensions. Phase 1 goldens and `migration_regression` are unchanged.
