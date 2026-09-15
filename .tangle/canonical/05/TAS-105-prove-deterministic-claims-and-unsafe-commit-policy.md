---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Prove deterministic simultaneous claims and every unsafe committed-maneuver response.
next: [[TAS-127-prove-deterministic-simultaneous-claim-resolutio]]
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Adversarial tests prove that simultaneous claims resolve by the documented
stable key and that a committed maneuver responds to each newly unsafe corridor
with the documented bounded brake, abort, hold, or return action.

# Done when

- Two and three-agent claims on the same corridor select the same winner after
  reversing source declaration, candidate discovery, and insertion order.
- Fixed-seed repetition is identical and the result does not depend on an
  unordered collection or incidental random draw.
- Front intrusion, rear intrusion, disappearing connector, narrowing corridor,
  and blocked return each reach the required state and bounded motion response.
- A falsification probe changes the tie-break key or removes one hazard response
  and demonstrates that the suite fails for the intended reason.
- No case teleports, overlaps silently, exceeds a motion limit, or crosses a
  forbidden boundary without the TAS-100 event fact.

# Context

Gated on [[TAS-091-resolve-gap-claims-and-maneuver-transitions]],
[[TAS-095-complete-lane-transitions-and-safe-aborts]], and
[[TAS-100-version-the-maneuver-event-and-trace-surface]]. Owns one focused
adversarial integration suite and minimal defects it exposes. Do not add broad
scenario fixtures, metrics, or presentation.

Gates my test artifact enters: Cargo's hekate-sim integration-test discovery and
the workspace warnings-as-errors test gate.

# Slices

- [[TAS-127-prove-deterministic-simultaneous-claim-resolutio]] Deterministic simultaneous-claim proof.
- [[TAS-128-prove-each-unsafe-commit-hazard-response-with-a]] Unsafe-commit hazard responses and probe.
