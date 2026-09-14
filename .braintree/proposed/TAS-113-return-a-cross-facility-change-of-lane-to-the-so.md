---
context_rev: 1
updated: 2026-09-14T13:24:58Z
summary: Return a cross-facility change of lane to the source band.
next: Add the return crossing at the compiled shared boundary and the return-leg obstruction policy, with focused tests.
---

Parent [[TAS-095-complete-lane-transitions-and-safe-aborts]].

# Outcome

A committed cross-facility change of lane can return from the destination band to
the source band through the compiled shared boundary, steering back to the
source offset under the same compiled corridor and obstruction policy as an
ordinary return, with the world pose continuously authoritative.

# Done when

- The return crossing fires at the compiled shared boundary
  (`CompiledFacilityAdjacency::shared_boundary_offset`) and moves route and
  facility ownership in one step with no despawn, re-spawn, or snap.
- `returning` and `aborted` on a cross-facility change of lane detect an
  obstructing body through the ordinary predictor and hold or re-decide per the
  documented policy, exactly as `RouteState.return_blocked` already does for a
  same-facility maneuver.
- Focused tests cover a completed pass out into an adjacent band and back, and a
  return blocked by a body in the destination corridor.

# Context

Depends on [[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]] at context_rev 1.
Reads the sim seams in [[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]
and the compiled datum in
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the sim-side
return crossing and return-leg policy; do not change the authored schema or
redefine the maneuver lifecycle.
