---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Check in presenter fixtures and the negative source guard.
next: Add the passing and occupied-opposing presenter fixtures and the source-text negative guard with a falsification probe.
---

Parent [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]].

# Outcome

One passing and one occupied-opposing fixture render with backend parity, and a
negative guard proves shared presentation modules carry no scenario or mode branch.

# Done when

- One passing fixture and one occupied-opposing fixture open through the version-2
  loader and show parity in golden and shared-scene tests.
- A source-text negative guard plus falsification probe rejects scenario names and
  Increment 2 mode names in shared presentation modules.
- Presenter, TUI, viewer, workspace, dependency-direction, and relevant golden
  tests pass.

# Context

Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the fixtures and
the negative guard; the overlay mapping is the sibling slice.
