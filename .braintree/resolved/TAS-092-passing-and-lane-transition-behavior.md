---
context_rev: 1
priority: P1
updated: 2026-09-14T15:01:12Z
summary: Integrate narrow passing, motor overtaking, and safe lane or facility transitions.
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
- [[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]] compiles the
  adjacency's shared-boundary lateral coordinate so a lateral handoff and its
  committed hazard response read one compiled geometric datum.
- [[TAS-095-complete-lane-transitions-and-safe-aborts]] connects adjacent
  facilities and owns forbidden-boundary, brake, abort, and return behavior.
- No tactic bypasses ordinary body queries, motion limits, collision recording,
  or the stable controller-stage ordering.

# Context

Depends on [[TAS-087-continuous-lateral-motion-and-gap-machinery]] at context_rev 1.
This node owns roll-up; each child is a separately testable behavioral outcome.

# Result

Complete. All four children resolved:
[[TAS-093-enable-same-facility-narrow-user-passing]],
[[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]],
[[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]], and
[[TAS-095-complete-lane-transitions-and-safe-aborts]]. The shared tactical
pipeline carries bicycle/scooter passing, motor-vehicle overtaking of narrow
users, and configured lane/facility transitions with bounded abort and return.
No tactic bypasses ordinary body queries, motion limits, collision recording, or
the stable controller-stage ordering. Final gate on the integrated tree:
`cargo test --workspace` (875 passed, 0 failed), `cargo clippy --workspace
--all-targets --all-features -- -D warnings` (clean), `cargo fmt --all --check`
(clean), and `scripts/check-dependency-direction.sh` (`dependency direction OK`).
