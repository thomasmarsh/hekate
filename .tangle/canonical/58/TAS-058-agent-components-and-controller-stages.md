---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: The compiled component model, mode-template compilation, the four controller stages, and the model-card template are delivered.
---

Parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]].

# Outcome

Per `PHASE_2_PLAN.md` Increment 0 (extension contract): shared kernel behavior
operates on composable capabilities and physical state, not named modes. This
node delivers the compiled component model (body, motion, tactical capability,
access, occupancy, social state), mode-template validation, the four
controller-stage interfaces, and a reusable model-card template.

# Done when

- Compiled agent data carries body, motion, tactical capability, access, occupancy, and social state as separate composable components.
- The compiler validates mode templates and rejects impossible combinations such as transit dwell without capacity or articulation on a holonomic body.
- An agent update is split into relevant-world query, tactical choice, motion control, and physical advance as explicit interfaces.
- A model-card template exists and both Phase 1 model cards conform to it.
- A synthetic template can alter dimensions, limits, and access without adding a named-mode branch to shared interaction code.

# Context

Decomposed just in time into direct children; this node stays open until every child is resolved or disposed and the criteria above hold.

# Result

The Increment 0 extension contract is complete; all five criteria hold through
resolved children, with no child disposed.

- **Compiled components.** [[TAS-066-compiled-agent-component-model]] added
  `crates/hekate-model/src/components.rs`: an `AgentCore` plus composable body,
  motion, tactical-capability, access, occupancy, and social-state components,
  and an `AgentFamily` derived from body and motion alone.
- **Template compilation and validation.** [[TAS-067-mode-template-compilation]]
  added the pure `compile_mode_template` compiler and `CompiledModeTemplate`
  bundle (with a `compose(core, social)` bridge) and rejects impossible
  combinations with stable diagnostics naming the template id; `validate_v2`
  keeps rejecting every impossible combination the Increment 0 source can
  express.
- **Four controller stages.** [[TAS-068-controller-stage-interfaces]] added
  `crates/hekate-sim/src/stage.rs` and routed both Phase 1 modes through
  relevant-world query, tactical choice, motion control, and physical advance
  with no trace-hash change.
- **Model-card template.** [[TAS-069-model-card-template]] checked in
  `docs/model-card-template.md`, conformed both Phase 1 cards to it, and re-based
  the drift test on the template inventory.
- **Synthetic-template gate.** [[TAS-070-synthetic-template-gate]] checked in a
  synthetic version-2 template with altered dimensions, limits, and access;
  proved it compiles to components, runs through the shared stages with its
  limits binding, and added a source-text guard that fails if shared code names
  the synthetic mode.

Evidence: `tangle check` passes (110 nodes); `cargo test --workspace` passes
with all golden traces and Phase 1 baselines unchanged;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`.

Limitation carried to later increments: Increment 0 defines the compiled
component model but does not yet wire `CompiledModeTemplate` into
`Simulation` spawning; `compile_v2` still materializes only the
`passenger_car`/`pedestrian` templates into the version-1 view. End-to-end
component-driven spawning is later work, so the synthetic-template run test
installs the derived body and profile on a spawned agent in place of it.
