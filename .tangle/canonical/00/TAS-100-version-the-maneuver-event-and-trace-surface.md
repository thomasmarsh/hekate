---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Version maneuver lifecycle, rule, boundary, and transition events across trace and replay.
---

Parent [[TAS-099-increment-2-events-metrics-and-output]].

# Outcome

The typed event union and canonical serialization record overtake attempt,
commit, abort, completion, facility transition, forbidden-boundary action, and
rule violation with one documented version and stable within-tick order.

# Done when

- Payloads match TAS-083 and name agent, partner when applicable, source/target
  facility or movement, side, reason, state edge, perceived rule, and decision
  reason without copying high-volume trajectory samples.
- EVENT_VERSION is bumped exactly once for the additive union and manifests,
  JSONL serialization, replay, summaries, inspectors, and tests all recognize
  the same version.
- Emission is edge-triggered from state changes already owned by TAS-091,
  TAS-095, TAS-097, and TAS-098; retries cannot duplicate a transition.
- Event ordering has explicit stable keys and is invariant to declaration or
  candidate insertion order.
- Focused round-trip, ordering, lifecycle, replay, and old-fixture regression
  tests pass; required Phase 1 goldens change only with the versioned rationale.

# Context

Depends on [[TAS-095-complete-lane-transitions-and-safe-aborts]] at context_rev 1.
Depends on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] at context_rev 1.
Owns crates/hekate-sim/src/event.rs and emission seams, CLI event serialization
and replay/summary consumers, presenter decoding as needed, and directly
affected goldens. Do not compute close-pass or wrong-way aggregate metrics.

# Result

Complete. Both children resolved:
[[TAS-119-add-the-maneuver-event-payloads-and-bump-event-v]] added the
`Maneuver`, `FacilityTransition`, and `OpposingTraversal` payloads under the
single `EVENT_VERSION` 2 -> 3 bump with the event-level `ManeuverReasonCode` and
regenerated versioned goldens; and
[[TAS-120-emit-maneuver-events-edge-triggered-with-stable]] emits `Maneuver` and
`FacilityTransition` edge-triggered from the state changes the maneuver and
handoff stages already compute, with explicit stable order keys and no golden
change. Focused round-trip, ordering, lifecycle, replay, and old-fixture
regressions pass. `OpposingTraversal` interval emission and the close-pass event
belong to later children under the same version.

# Slices

- [[TAS-119-add-the-maneuver-event-payloads-and-bump-event-v]] Event payloads and EVENT_VERSION.
- [[TAS-120-emit-maneuver-events-edge-triggered-with-stable]] Edge-triggered emission and ordering.
