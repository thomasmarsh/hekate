---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Version maneuver lifecycle, rule, boundary, and transition events across trace and replay.
next: Add the Increment 2 event union and carry it losslessly through run artifacts and replay.
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

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]] and
[[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]. Owns
crates/tangle-sim/src/event.rs and emission seams, CLI event serialization and
replay/summary consumers, presenter decoding as needed, and directly affected
goldens. Do not compute close-pass or wrong-way aggregate metrics.
