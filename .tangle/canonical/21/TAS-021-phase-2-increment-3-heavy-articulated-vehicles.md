---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Increment 3 adds buses, rigid trucks, and a tractor-semitrailer with segment-level swept collision evidence.
next: Add bus and rigid-truck templates with heavy-vehicle dynamics and swept turning envelopes.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 3:

- Bus and rigid-truck templates with mode-specific dimensions, wheelbase,
  turning limits, acceleration/braking envelopes, and desired-speed profiles.
- Tractor-semitrailer geometry, deterministic hitch and trailer integration,
  segment broad/narrow phase, swept queries, and articulation limits.
- Route-feasibility diagnostics and off-tracking, curb-encroachment, and
  articulation-limit events.
- Independent straight, constant-radius, S-turn, stop, following, and conflict
  fixtures.

# Done when

- Segment poses match analytic or high-resolution reference trajectories within
  declared tolerances.
- Swept tests detect contacts by any segment even when no endpoint overlaps.
- Infeasible authored turns fail validation or produce an explicit runtime
  inability to proceed; bodies never clip through boundaries.
- Phase 1 box/circle collision fixtures remain unchanged except for declared
  versioned corrections.

Parent [[TAS-017-phase-2-mixed-traffic]].
