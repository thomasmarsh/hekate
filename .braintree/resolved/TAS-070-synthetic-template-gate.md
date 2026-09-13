---
context_rev: 1
priority: P1
updated: 2026-09-13T17:20:16Z
summary: Prove a synthetic template changes dimensions, limits, and access without a mode branch.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

The Increment 0 extension gate: a synthetic mode template changes body
dimensions, kinematic limits, and facility access and runs through the shared
interaction code with no new named-mode branch.

# Done when

- A checked-in synthetic template compiles to components and runs at least one scenario.
- A test or review check shows the shared interaction, event, and metric code gained no branch on the synthetic mode name.
- The fixture fails if mode-specific dispatch is reintroduced.

# Context

Gated on [[TAS-067-mode-template-compilation]].

# Result

The Increment 0 extension gate holds: a checked-in synthetic template compiles
to components and drives the shared stages with no named-mode branch. All three
`Done when` criteria hold.

- **A checked-in synthetic template compiles to components and runs.**
  `crates/tangle-model/tests/fixtures/synthetic_mode_template_v2.json5` (new)
  authors `synthetic_hauler`, a box + single-body-wheeled template whose body
  (6.4 m x 2.4 m), desired speed (3.4 m/s), acceleration (0.9 m/s²), braking
  (1.6 m/s²), following time gap (2.8 s), compliance (0.4), and facility access
  (`path` + `crossing`) all differ from the Increment 0 passenger-car and
  pedestrian templates in `docs/schema-v2-contract.md`.
  `crates/tangle-model/tests/synthetic_mode_template.rs` (new) proves the fixture
  is a valid version-2 document, that `compile_mode_template` carries every
  authored body, limit, and access value into the `CompiledModeTemplate`, and
  that the derived `WheeledBox` family comes from the body/motion pair, not the
  id (renaming the id leaves every component and the family equal).
  `crates/tangle-sim/src/sim.rs` adds
  `the_synthetic_template_runs_through_the_shared_stages`: it compiles the
  fixture, derives the `VehicleProfile` the kernel's controller reads from the
  compiled body and behavior components, installs them on one spawned agent —
  Increment 0 does not yet wire `CompiledModeTemplate` into spawning, so this
  stands in for that later step — and then runs the kernel's own stages 1–4 over
  400 ticks. The altered desired speed caps the speed at 3.4 m/s and the altered
  acceleration bound caps each step at 0.9 m/s² as the agent accelerates from
  rest; a required stop-line constraint through the same motion stage caps the
  deceleration at the altered 1.6 m/s²; and the altered body length travels
  through the shared agent state.
- **The shared code gained no branch on the synthetic mode.**
  `crates/tangle-sim/tests/synthetic_template_no_branch.rs` (new) includes the
  source text of `sim.rs`, `stage.rs`, `controller.rs`, `control.rs`,
  `pedestrian.rs`, `event.rs`, `metrics.rs`, and `safety.rs` with `include_str!`
  and fails if any names `synthetic_hauler`. It also asserts the fixture declares
  the id it searches for, so the guard cannot silently drift from the fixture.
- **The fixture fails if mode-specific dispatch is reintroduced.** Adding the
  synthetic id to any shared interaction, event, metric, stage, or controller
  module makes the source-text guard fail, so a mode branch for this template
  breaks the gate.

Additive: no kernel behavior changed, no trace hash or golden file changed, and
`CompiledScenario`, `compile_v2`, and the checked-in JSON Schema are untouched.
The only production-file edit is a `#[cfg(test)]` test added to `sim.rs`'s
existing test module.

Evidence: `cargo test -p tangle-model` passes (69 unit + new integration tests);
`cargo test -p tangle-sim` passes (139 unit + every integration suite, including
the new run test and the no-branch guard); `cargo test --workspace` passes with
golden traces and baselines unchanged; `cargo clippy --workspace --all-targets
--all-features` is clean; `cargo fmt --all --check` is clean;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`.

Handoff: with this node resolved, `braintree check` reports the expected
`next-resolved-node` diagnostic for [[TAS-058-agent-components-and-controller-stages]]
because its `next` still names this now-resolved child. TAS-058 stays open for its
remaining criteria and must not be edited from this session.
