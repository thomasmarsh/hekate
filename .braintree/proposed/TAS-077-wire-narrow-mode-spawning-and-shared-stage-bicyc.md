---
context_rev: 1
updated: 2026-09-13T18:29:20Z
summary: Wire narrow-mode spawning and shared-stage bicycles and scooters with model cards.
next: Wire narrow-mode spawning and the shared-stage bicycle and scooter controller into tangle-sim with model cards.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

Bicycle and scooter run as first-class modes through the shared four controller
stages: component-driven spawning installs each narrow mode's body, profile, and
facility reference route from its compiled mode template, and the narrow wheeled
controller performs longitudinal following, stops, signals, priorities, facility
selection, and route completion before any free lateral maneuver is enabled. Its
motion respects authored dimensions, speed, acceleration, braking, steering, and
facility boundaries, and both modes carry model cards in the module that
implements them.

# Done when

- `tangle-sim` spawns narrow agents from their compiled mode template and routes
  their update through the four stages with no named-mode branch in shared code
  (guarded by a source-text/test probe as in [[TAS-070-synthetic-template-gate]]).
- A test proves command envelopes and body bounds are respected for one narrow
  agent accelerating, following a leader, holding a stop line, yielding at a
  signal, and completing its route along a compiled facility.
- A `bicycle` card and a `scooter` card conform to `docs/model-card-template.md`
  and the drift test `crates/tangle-sim/tests/model_cards.rs` passes.
- `cargo test -p tangle-sim` and `cargo test --workspace` pass with every Phase 1
  golden trace unchanged.

# Context

Gated on [[TAS-074-compile-version-2-facility-and-connector-shapes]],
[[TAS-075-add-increment-1-facility-and-narrow-mode-validat]], and
[[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]]. Per
`PHASE_2_PLAN.md` *Bicycles and scooters*, *Continuous lateral motion* (longitudinal
precursor), and *Increment 1* ("...before free lateral maneuvers are enabled").
Read [[TAS-058-agent-components-and-controller-stages]],
[[TAS-068-controller-stage-interfaces]], and [[TAS-069-model-card-template]].
Owns `crates/tangle-sim/src/sim.rs`, `stage.rs`, `controller.rs`, `control.rs`,
`profile.rs`, and `agent.rs`.
