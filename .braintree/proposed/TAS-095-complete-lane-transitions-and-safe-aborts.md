---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Complete configured lane or facility transitions with bounded abort, braking, and return.
next: Connect lateral claims to adjacent-facility routes and prove every loss-of-gap outcome.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

Eligible wheeled agents can transition through authored adjacent connectors
around a slower leader, while every infeasible or degraded maneuver produces a
bounded hold, brake, abort, return, or explicit forbidden-boundary fact.

# Done when

- A transition updates route/facility ownership only at the contract-defined
  geometric handoff, preserves stable progress, and never despawns/re-spawns or
  snaps to the destination reference.
- Current and destination leader/follower constraints remain active and the
  ordinary spatial index and collision scan see every intermediate world pose.
- Preparing loss, committed front hazard, committed rear hazard, target
  disappearance, boundary closure, and return obstruction each follow the
  documented deterministic policy within motion limits.
- Crossing a prohibited boundary is prevented when avoidable and exposes the
  violation fact TAS-100 will record when unavoidable under the policy.
- Focused tests cover adjacent-lane change, connector handoff, each abort case,
  complete-and-return, and no-change behavior without authored capability.

# Context

Gated on [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]]. Owns
facility-transition integration, safe-abort behavior, and focused tests. Do not
define event payloads, close-pass aggregation, or acceptance scenarios.
