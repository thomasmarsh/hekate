---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Add route-relative lateral, target, corridor, and maneuver state to wheeled agents.
next: Add component-driven lateral state and expose it through snapshots without changing world truth.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

Eligible single-body wheeled agents carry deterministic current s/d, target
offset or transition, predicted-gap summary, claim identity, and maneuver state
alongside their authoritative world pose.

# Done when

- The stable agent store initializes route coordinates from compiled facility
  projection and records following, preparing, committed, returning, and aborted
  plus the target clearance/horizon and target facility when applicable.
- Passenger cars and narrow agents share one physical-family representation;
  pedestrians and legacy version-1 paths retain their current state and output.
- State changes occur only through the existing controller-stage pipeline and
  stable AgentId order; no hash-map iteration determines behavior.
- Snapshot and canonical trace expose optional route s/d, target offset,
  predicted gap, and maneuver state under an explicit format/version change;
  sparse lifecycle events remain TAS-100 scope.
- Unit tests cover spawn initialization, absent-state compatibility, stable
  storage, serialization, and projection signs in both traversal directions.

# Context

Gated on [[TAS-085-compile-increment-2-policy-and-traversal-semantics]]. Owns
the smallest required seams in crates/tangle-sim/src/agent.rs, stage.rs,
snapshot.rs, sim.rs, and their focused tests. Do not integrate lateral motion,
choose passes, or emit maneuver events.
