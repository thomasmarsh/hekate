---
context_rev: 1
status: proposed
updated: 2026-09-15T22:02:18Z
summary: Add hitch integration, runtime dispatch, and swept collision for articulated-wheeled.
next: Add deterministic hitch/trailer pose integration for AgentFamily::ArticulatedWheeled.
---

Parent [[TAS-021-phase-2-increment-3-heavy-articulated-vehicles]].

# Context

The static authoring/compile/validate slice (tas-4ep58y0syjtnwny4bgcg5q41j7) added `ModeBodySource::ArticulatedChain`, `MotionKind::ArticulatedWheeled`, the matching `BodySegment` hitch offsets and `articulation_limit_rad`, and their compile/validate coverage, plus a checked `tractor_semitrailer_v2` geometry fixture. Nothing yet makes an `AgentFamily::ArticulatedWheeled` agent move, hitch, or collide: `Simulation::try_admit` and every kernel controller in `hekate-sim` dispatch only `HolonomicCircle`, `WheeledBox`, and `WheeledCapsule` today, and there is no swept or narrow-phase collision path for a multi-segment body.

# Outcome

An admitted `tractor_semitrailer` (or any future `ArticulatedWheeled` mode) drives with deterministic tractor-driven hitch and trailer pose integration honoring each segment's hitch offset and the authored `articulation_limit_rad`; broad/narrow-phase collision indexes and reports per-segment contacts instead of one proxy shape; and an infeasible authored turn or a runtime articulation-limit violation surfaces as a typed diagnostic or event rather than a silent clip through a curb or another agent.

# Done when

- `Simulation::try_admit` (or its successor) spawns an `ArticulatedWheeled` agent from its own template, mirroring the wheeled-box/wheeled-capsule spawn wiring.
- Tractor pose deterministically drives each trailing segment's pose through its authored hitch offset; a jackknife beyond `articulation_limit_rad` is a typed, observable event, not a silently clipped body.
- Broad phase indexes one proxy per chain or one per segment; narrow phase reports the actual contacting segment pair, not the whole chain.
- A swept query along a curved reference detects a contact even when no endpoint overlaps.
- Route-feasibility diagnostics extend to authored articulation (off-tracking / corner curvature under articulation), which tas-4ep58y0syjtnwny4bgcg5q41j7 explicitly deferred.
- At least one runtime fixture (straight and/or constant-radius) drives a `tractor_semitrailer` end to end and is checked in, analogous to `heavy_isolated_v2`/`heavy_turning_v2`.
- The five gates pass.
