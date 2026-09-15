---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T11:13:30Z
summary: Present usable corridor, target offset, predicted gap, maneuver, and wrong-way overlays.
next: [[TAS-136-check-in-presenter-fixtures-and-the-negative-sou]]
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

The shared scene and both viewer backends can inspect an Increment 2 run's usable
corridor, target offset, predicted gap, maneuver state, and perceived/current
wrong-way rule state without simulation or scenario-specific branches.

# Done when

- hekate-present maps the versioned snapshot/event state into backend-neutral
  overlay primitives and inspector text with agent, partner, facility/movement,
  clearance, state, and reason identifiers.
- Bevy and terminal backends expose all five required overlays with deterministic
  draw/order behavior and graceful absence for Phase 1/Increment 1 runs.
- One passing fixture and one occupied-opposing fixture open through the
  version-2 loader and show parity in golden/shared-scene tests.
- A source-text negative guard plus falsification probe rejects scenario names
  and Increment 2 mode names in shared presentation modules.
- Presenter, TUI, viewer, workspace, dependency-direction, and relevant golden
  tests pass.

# Context

Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]],
[[TAS-101-measure-close-passes-with-exact-clearance-evidence]],
[[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]],
[[TAS-106-check-in-increment-2-passing-fixtures]], and
[[TAS-107-check-in-contextual-wrong-way-fixtures]]. Owns shared scene/overlay
mapping, both backend renderings, inspector text, presenter tests, and directly
affected scene goldens. Do not alter simulation decisions or metric semantics.

# Slices

- [[TAS-135-map-increment-2-state-into-presenter-overlays-wi]] Overlay mapping and backend parity.
- [[TAS-136-check-in-presenter-fixtures-and-the-negative-sou]] Presenter fixtures and negative guard.
