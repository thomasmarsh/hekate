---
context_rev: 1
updated: 2026-09-14T13:25:52Z
summary: Prove each unsafe-commit hazard response with a falsification probe.
next: Prove each committed-hazard response and add a falsification probe for the tie-break key and a hazard response.
---

Parent [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]].

# Outcome

Adversarial tests prove a committed maneuver responds to each newly unsafe
corridor with the documented bounded brake, abort, hold, or return action, and a
probe shows the suite fails when a response is removed.

# Done when

- Front intrusion, rear intrusion, disappearing connector, narrowing corridor, and
  blocked return each reach the required state and bounded motion response.
- A falsification probe changes the tie-break key or removes one hazard response
  and demonstrates that the suite fails for the intended reason.
- No case teleports, overlaps silently, exceeds a motion limit, or crosses a
  forbidden boundary without the TAS-100 event fact.

# Context

Extends [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]]; reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the hazard
responses and the falsification probe.
