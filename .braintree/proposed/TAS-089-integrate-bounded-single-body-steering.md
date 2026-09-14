---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Integrate bounded steering and lateral motion for single-body wheeled agents.
next: Implement one bounded steering step from route targets through world pose and projection.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

A lateral target produces continuous single-body wheeled motion whose speed,
acceleration, braking, steering angle/rate, curvature, and lateral acceleration
stay inside compiled limits, with world pose reconstructed then projected to
route coordinates.

# Done when

- MotionCommand can carry the bounded steering information TAS-083 fixes while
  preserving the existing longitudinal command path.
- Each step integrates without setting d or world position directly to a target,
  reconstructs the physical pose, and projects it back for drift evidence.
- The corridor boundary and existing collision/safety caps constrain the same
  proposed world step; a failed request brakes or holds rather than clipping.
- Straight, curved, forward, and reverse unit fixtures assert per-step limits,
  continuous displacement, projection drift, and no one-tick lane-centre snap.
- Existing car, narrow-longitudinal, pedestrian, and Phase 1 golden tests pass.

# Context

Gated on [[TAS-088-add-route-relative-lateral-agent-state]]. Owns a focused
single-body steering module or the narrow/controller seams, MotionCommand and
sim integration required by it, and focused tests. Do not implement gap
prediction, tactical state transitions, passing selection, or wrong-way choice.
