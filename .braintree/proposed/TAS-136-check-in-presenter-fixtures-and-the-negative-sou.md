---
context_rev: 1
updated: 2026-09-14T13:25:55Z
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

Extends [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the fixtures and
the negative guard; the overlay mapping is the sibling slice.
