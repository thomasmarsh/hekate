---
context_rev: 1
status: proposed
updated: 2026-09-15T20:10:01Z
summary: Add bus and rigid-truck box templates with heavy profile envelopes and per-template spawn.
next: Author the bus and rigid_truck mode templates, compile and validate them, and make each spawn with its own dimensions and dynamics in a heavy_isolated_v2 straight fixture.
---

Parent [[TAS-021-phase-2-increment-3-heavy-articulated-vehicles]].

# Context

`PHASE_2_PLAN.md` "Buses and rigid trucks" (around line 177) fixes the intent: oriented rectangular bodies with mode-specific dimensions, wheelbase, turning limits, acceleration/braking envelopes, and desired-speed profiles, whose controllers reuse the ordinary following, signal, priority, yielding, and lateral logic rather than a new dynamics law.

Today every box mode is placed from the `passenger_car` template: `Simulation::try_admit` samples `self.scenario.profiles()` (`crates/hekate-sim/src/sim.rs:3451-3455`), and only the template whose id is `passenger_car` reaches the v1 profile view (`crates/hekate-model/src/compiled.rs:4011`, `:4103`). This child owns the heavy templates and that spawn wiring. Authored wheelbase and turn feasibility belong to the next child; articulation is out of scope.

# Outcome

`bus` and `rigid_truck` are authored mode templates with heavy box dimensions and heavy IDM envelopes; they compile and validate, and an admitted bus or rigid truck drives with its own body length/width and sampled dynamics rather than the passenger car's.

# Done when

- `bus` and `rigid_truck` authored templates compile and validate, with their required heavy profile params checked in `crates/hekate-model/src/validate.rs` (`required_profile_params`).
- An admitted bus/rigid-truck agent's `body_length_m`/`body_width_m` and sampled dynamics come from its own template, and the `passenger_car` spawn path stays byte-identical so the existing inc2 goldens do not shift.
- `scenarios/phase2/inc3/heavy_isolated_v2.json5` runs a straight bus and rigid-truck fixture, with a test pinning their dimensions and a bounded drive.
- `schemas/scenario-source.schema.json` is regenerated and the schema drift test passes.
- The five gates pass.
