---
context_rev: 1
updated: 2026-09-14T13:25:50Z
summary: Emit maneuver events edge-triggered with stable ordering.
next: Emit the new maneuver events edge-triggered from the state changes with explicit stable order keys.
---

Parent [[TAS-100-version-the-maneuver-event-and-trace-surface]].

# Outcome

Maneuver and rule events are emitted edge-triggered from existing state changes
with explicit stable ordering, so no transition is duplicated or reordered.

# Done when

- Emission is edge-triggered from state changes already owned by the maneuver and
  wrong-way stages; retries cannot duplicate a transition.
- Event ordering has explicit stable keys and is invariant to declaration or
  candidate insertion order.
- Lifecycle, ordering, replay, and old-fixture regression tests pass.

# Context

Extends [[TAS-100-version-the-maneuver-event-and-trace-surface]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns emission
timing and ordering; the payload shape and version are the sibling slice.
