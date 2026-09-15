---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T11:30:39Z
summary: Present usable corridor, target offset, predicted gap, maneuver, and wrong-way overlays.
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

# Result

Both slice children resolved: [[TAS-135-map-increment-2-state-into-presenter-overlays-wi]]
maps the versioned route state and typed edge records into backend-neutral corridor,
target-offset, predicted-gap, maneuver, and wrong-way primitives with agent, partner,
facility/movement, clearance, state, and reason identifiers and the shared inspector
summaries, and [[TAS-136-check-in-presenter-fixtures-and-the-negative-sou]] opens one
passing and one occupied-opposing fixture through the version-2 loader with golden and
both-backend parity plus the source-text negative guard and its falsification probe.
Five gates green at `083d6f8`: fmt, clippy `-D warnings`, workspace tests (1049 passed,
0 failed, 3 ignored), dependency direction, and `tangle check` (195 nodes). The
documented nuance that an Increment 1 version-2 body carries route state on a compiled
band — so its corridor is projected and drawn — is the corridor primitive's own
contract, asserted explicitly rather than papered over.
