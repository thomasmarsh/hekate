---
context_rev: 1
priority: P1
updated: 2026-09-12T01:30:53Z
summary: Implement the fixed-step kernel with deterministic constant-speed agents, snapshots, and typed events.
next: Implement Simulation::new, step, time, snapshot, and finish over a compiled scenario.
---

# Context

Depends on [[TAS-003-scenario-source-parse]] at context_rev 1.

# Outcome

`tangle-sim` exposes the small kernel API and advances agents at constant speed
along the guide path using the authoritative fixed-step clock, emitting typed
spawn/despawn events and intentionally lossy snapshots.

# Done when

- `step()` advances exactly one configured tick and never reads wall-clock time.
- Hot agent state is stored in dense, stable-order arrays with no hash-map
  iteration in state-affecting logic.
- `tangle-sim` has no Bevy, window, wall-clock, or filesystem dependency.
- Unit tests cover time and tick arithmetic and constant-speed path progress.

Parent [[TAS-001-phase-1-increment-0]].
