---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Map Increment 2 state into presenter overlays with backend parity.
next: Map the versioned snapshot and event state into backend-neutral overlays and expose all five overlays in both backends.
---

Parent [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]].

# Outcome

The shared scene and both viewer backends can inspect an Increment 2 run usable
corridor, target offset, predicted gap, maneuver state, and wrong-way rule state
without simulation or scenario-specific branches.

# Done when

- `hekate-present` maps the versioned snapshot and event state into backend-neutral
  overlay primitives and inspector text with agent, partner, facility or movement,
  clearance, state, and reason identifiers.
- Bevy and terminal backends expose all five required overlays with deterministic
  draw and order behavior and graceful absence for Phase 1 and Increment 1 runs.

# Context

Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]; reads the
presenter and gate seams in
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the mapping and
backend parity; fixtures and the negative guard are the sibling slice.
