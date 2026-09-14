---
context_rev: 1
updated: 2026-09-14T13:25:50Z
summary: Add the maneuver event payloads and bump EVENT_VERSION once.
next: Add the maneuver, transition, and violation event payloads and bump EVENT_VERSION exactly once.
---

Parent [[TAS-100-version-the-maneuver-event-and-trace-surface]].

# Outcome

The typed event union and canonical serialization carry every increment-2 maneuver
and rule record under one documented version.

# Done when

- Payloads match TAS-083 and name agent, partner when applicable, source and
  target facility or movement, side, reason, state edge, perceived rule, and
  decision reason without copying high-volume trajectory samples.
- EVENT_VERSION is bumped exactly once for the additive union, and manifests,
  JSONL serialization, replay, summaries, and inspectors recognize the same
  version.
- Round-trip tests pass and required Phase 1 goldens change only with the
  versioned rationale.

# Context

Extends [[TAS-100-version-the-maneuver-event-and-trace-surface]]; reads the seams
and gates in [[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns
the payloads and the version bump; do not change emission timing.
