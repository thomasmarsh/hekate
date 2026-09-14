---
context_rev: 1
priority: P1
updated: 2026-09-14T05:26:01Z
summary: Integrate narrow passing, motor overtaking, and safe lane or facility transitions.
next: [[TAS-095-complete-lane-transitions-and-safe-aborts]]
---

Parent [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]].

# Outcome

The shared tactical pipeline uses the lateral machinery for bicycle/scooter
passing, motor-vehicle overtaking of narrow users, and configured transitions
around slower leaders, including safe loss-of-gap behavior.

# Done when

- [[TAS-093-enable-same-facility-narrow-user-passing]] completes same-facility
  bicycle and scooter passes from slower-leader detection through return.
- [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]] displaces a motor
  vehicle only when permission, geometry, visibility, and gap evidence allow it.
- [[TAS-095-complete-lane-transitions-and-safe-aborts]] connects adjacent
  facilities and owns forbidden-boundary, brake, abort, and return behavior.
- No tactic bypasses ordinary body queries, motion limits, collision recording,
  or the stable controller-stage ordering.

# Context

Gated on [[TAS-087-continuous-lateral-motion-and-gap-machinery]]. This node owns
roll-up; each child is a separately testable behavioral outcome.
