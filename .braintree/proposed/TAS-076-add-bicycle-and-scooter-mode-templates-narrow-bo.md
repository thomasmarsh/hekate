---
context_rev: 1
updated: 2026-09-13T18:29:14Z
summary: Add bicycle and scooter mode templates, narrow bodies, and steering and clearance profiles.
next: Add bicycle and scooter version-2 mode templates, bodies, and narrow profile parameters to tangle-model.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

The compiled component model and mode-template compiler serve the narrow wheeled
family: `bicycle` and `scooter` version-2 templates compile to component bundles
with their own narrow box dimensions, speed, acceleration, braking, steering
response, and lateral-clearance parameters, plus their facility access, and no
shared code branches on the template id. The compiled components carry every
parameter the narrow controller and its model card need, aligned with the
profile-parameter set fixed by [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]].

# Done when

- Checked-in `bicycle` and `scooter` version-2 templates parse, validate, and
  compile to agent component bundles.
- A test proves the two templates differ from each other and from
  `passenger_car` in body, limits, steering/lateral parameters, and access, and
  that compiling two templates differing only in id yields equal components
  (no id branch).
- `cargo test -p tangle-model` passes with the existing golden compiled
  templates unchanged.

# Context

Gated on [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]]. Per
`PHASE_2_PLAN.md` *Bicycles and scooters* and *Increment 1*. Read
[[TAS-067-mode-template-compilation]], [[TAS-066-compiled-agent-component-model]],
and [[TAS-070-synthetic-template-gate]] (the no-named-mode-branch pattern). Owns
`crates/tangle-model/src/mode_template.rs`, `crates/tangle-model/src/components.rs`,
and the checked-in template fixtures.
