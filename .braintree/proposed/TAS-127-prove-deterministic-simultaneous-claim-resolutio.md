---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:43Z
summary: Prove deterministic simultaneous-claim resolution.
next: Prove two and three-agent claims select the same winner after reversing declaration, discovery, and insertion order.
---

Parent [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]].

# Outcome

Adversarial tests prove simultaneous corridor claims resolve by the documented
stable key regardless of declaration, discovery, or insertion order.

# Done when

- Two and three-agent claims on the same corridor select the same winner after
  reversing source declaration, candidate discovery, and insertion order.
- Fixed-seed repetition is identical and the result does not depend on an
  unordered collection or incidental random draw.

# Context

Extends [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]]; reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the claim
determinism proof; the hazard responses are the sibling slice.
