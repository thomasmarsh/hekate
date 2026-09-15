---
context_rev: 1
status: proposed
updated: 2026-09-15T22:15:46Z
summary: Deterministic hitch/trailer pose integration and admit wiring for ArticulatedWheeled.
next: Extend the per-agent runtime state with a segment-pose chain, wire Simulation::try_admit to spawn a tractor_semitrailer from its own template, and drive each trailing segment's pose from the tractor's motion through its authored hitch offset.
---

Parent [[tas-5vc5c3cbttnvcztafvns26bcs4-add-hitch-integration-runtime-dispatch-and]].

# Context

`Simulation::try_admit` (`crates/hekate-sim/src/sim.rs:3423`) dispatches only on `AgentFamily::WheeledCapsule`/`WheeledBox` today and spawns one `position`/`heading_rad` per agent (`AgentInit`); there is no per-segment pose representation anywhere in `hekate-sim`. `ModeBodySource::ArticulatedChain`, `BodySegment::hitch_offset_m`, and `AgentBody::ArticulatedChain::articulation_limit_rad` are authorable and compilable (tas-4ep58y0syjtnwny4bgcg5q41j7); nothing yet reads them at runtime.

# Outcome

An admitted `AgentFamily::ArticulatedWheeled` agent carries one pose per chain segment; the tractor (lead) segment integrates exactly as a `WheeledBox` does today, and each trailing segment's pose is deterministically driven off the segment ahead of it through its authored `hitch_offset_m`. A jackknife (relative hitch angle beyond the compiled `articulation_limit_rad`) is a typed, observable event, never a silent clip.

# Done when

- `Simulation::try_admit` spawns an `ArticulatedWheeled` agent from its own template, mirroring the `WheeledBox`/`WheeledCapsule` spawn wiring (`sample_mode_template_profile` precedent).
- Every trailing segment's pose is a deterministic function of the segment ahead of it and its authored hitch offset; re-running the same seed reproduces byte-identical segment poses.
- A jackknife beyond `articulation_limit_rad` raises a typed event/diagnostic rather than clipping the body through a boundary.
- At least one straight-path and one constant-radius fixture (analogous to `heavy_isolated_v2`/`heavy_turning_v2`) admits a `tractor_semitrailer` and holds every segment pose inside a declared tolerance of its analytic or high-resolution reference trajectory.
- Existing `WheeledBox`/`WheeledCapsule`/`HolonomicCircle` spawn and update paths stay byte-identical (inc2/inc3 goldens unchanged).
- The five gates pass.
