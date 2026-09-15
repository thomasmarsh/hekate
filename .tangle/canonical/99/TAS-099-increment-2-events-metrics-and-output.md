---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T03:08:00Z
summary: Emit versioned maneuver, close-pass, and wrong-way evidence through standard outputs.
---

Parent [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]].

# Outcome

Sparse events, trajectories, metrics, summaries, replay, and inspector state
explain every Increment 2 maneuver and rule interval with stable identifiers,
applicability, exact geometric evidence, and definition versions.

# Done when

- [[TAS-100-version-the-maneuver-event-and-trace-surface]] versions and emits
  attempted, committed, aborted, completed, transition, and violation records.
- [[TAS-101-measure-close-passes-with-exact-clearance-evidence]] records
  configurable-band close passes with exact clearance and relative-speed facts.
- [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]] records
  wrong-way state, interval, distance, exposure, encounters, and conflicts.
- Run/replay/aggregate surfaces preserve identifiers, event order,
  applicability, and definition versions; Phase 1 changes are only explicit
  versioned union additions.

# Context

Gated on [[TAS-092-passing-and-lane-transition-behavior]] and
[[TAS-096-contextual-wrong-way-travel]]. This node owns roll-up only.

# Result

All three children resolved: [[TAS-100-version-the-maneuver-event-and-trace-surface]]
versions and emits the maneuver lifecycle, transition, and violation records;
[[TAS-101-measure-close-passes-with-exact-clearance-evidence]] records
configurable-band close passes with exact clearance and relative-speed evidence;
and [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]] records the
wrong-way state, interval, distance, exposure, encounters, and conflicts. The
run/replay/aggregate surfaces preserve identifiers, event order, applicability,
and definition versions, and the Phase 1 changes are explicit versioned union
additions only.
