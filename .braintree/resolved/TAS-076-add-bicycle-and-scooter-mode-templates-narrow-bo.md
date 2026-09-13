---
context_rev: 1
updated: 2026-09-13T19:29:47Z
summary: Add bicycle and scooter mode templates, narrow bodies, and steering and clearance profiles.
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

# Result

The narrow wheeled family now compiles into agent component bundles that carry
its steering and lateral-clearance parameters, and no shared model module
branches on a template id. All three `Done when` criteria hold.

## Checked-in templates and compilation

`crates/tangle-model/tests/fixtures/narrow_mode_templates_v2.json5` (new) is a
valid version-2 scenario authoring all three templates: the Increment 0
`passenger_car` plus the contract's normalized `bicycle` and `scooter` templates
(capsule bodies, `facility` access with `nominal_direction: 'either'` and
`speed_policy: { limit_mps: null }`, and the narrow wheeled profile set). It also
carries a bikeway facility the two narrow modes share and one rate demand per
mode, so the whole document parses and validates.
`crates/tangle-model/tests/narrow_mode_templates.rs` (new) proves the fixture
parses (`parse_scenario_source_v2`), validates with no diagnostics
(`validate_v2`), compiles as a whole scenario (`CompiledScenario::compile_v2`),
and that each template compiles through `compile_mode_template` to a
`WheeledCapsule` bundle.

## Component fields added

The two narrow parameters TAS-075 left unrepresented are now on the compiled
behavior profile, `crates/tangle-model/src/components.rs`
(`AgentBehaviorProfile`): `steering_rate_max_rad_s: Option<ProfileRange>` and
`lateral_clearance_m: Option<ProfileRange>`, with the getters
`steering_rate_max_rad_s()` and `lateral_clearance_m()`. The new
`AgentBehaviorProfile::narrow_wheeled(...)` constructor sets both; the existing
`walking(...)` and `wheeled(...)` constructors set both to `None`, so every
Increment 0 bundle is byte-for-byte the value it was.
`crates/tangle-model/src/mode_template.rs` (`compiled_profile`) selects
`narrow_wheeled` when the authored body/motion pair is
`capsule` + `single_body_wheeled` — the same `(body, motion)` key
`required_profile_params` uses — and never reads the template id.

## Differ and no-id-branch tests

`crates/tangle-model/tests/narrow_mode_templates.rs`:

- `the_narrow_templates_differ_from_each_other_and_from_passenger_car` — asserts
  body differs (car is a `Box`, bicycle and scooter are `Capsule`, and their
  length/radius ranges differ), limits differ (bicycle and scooter desire-speed,
  acceleration, and braking ranges differ, and both stay below the car's speed
  envelope and follow more closely), steering/lateral differ (the car carries
  neither parameter, both narrow modes carry both, and their steering ranges
  differ), and access differs (the car permits `path`, both narrow modes permit
  `facility` and not `path`).
- `renaming_a_template_id_does_not_change_its_components` — the falsification
  probe: for each narrow id, a template renamed to `a_different_name` compiles to
  equal body, motion, tactics, access, occupancy, profile, and family, and a
  different id.
- `no_shared_model_module_branches_on_a_narrow_template_id` — the source-text
  guard, over the production code (each module truncated at its `#[cfg(test)]`)
  of exactly five shared model modules: `mode_template.rs` (the compiler),
  `components.rs` (the component model and family dispatch), `compiled.rs` (the
  compiled scenario), `source.rs` (the authored schema), and `validate.rs`
  (semantic validation). It fails if any production code names the id as a Rust
  string literal `"bicycle"`/`"scooter"` or `'bicycle'`/`'scooter'`.
- `the_fixture_declares_the_narrow_ids_this_guard_searches_for` — keeps the guard
  from drifting off the ids the fixture actually authors.
- `the_no_id_branch_guard_detects_a_branch_and_ignores_prose_and_tests` —
  falsifies the guard mechanism: it flags `id == "bicycle"` and a `"scooter"`
  match arm, ignores a doc-comment prose mention of a bicycle or scooter, and
  ignores ids that appear only inside a `#[cfg(test)]` module.

Supporting coverage:
`the_narrow_fixture_is_a_valid_version_2_document`,
`the_narrow_templates_compile_through_the_full_scenario`, and
`a_narrow_template_compiles_to_a_capsule_bundle_with_narrow_parameters` (each
narrow bundle is `WheeledCapsule`, `Either`/unlimited access, and carries both
narrow parameters). `components.rs` unit test
`the_narrow_wheeled_profile_carries_steering_and_clearance` pins the new
constructor and the two getters against the unchanged `walking`/`wheeled`
layouts.

## Files changed

`crates/tangle-model/src/components.rs` (two `AgentBehaviorProfile` fields, the
`narrow_wheeled` constructor, two getters, one unit test),
`crates/tangle-model/src/mode_template.rs` (`compiled_profile` selects the
narrow constructor by body/motion), the new
`crates/tangle-model/tests/fixtures/narrow_mode_templates_v2.json5`, and the new
`crates/tangle-model/tests/narrow_mode_templates.rs`. No `crates/tangle-sim/**`,
`crates/tangle-present/**`, `apps/**`, `baselines/**`, `schemas/**`, or `docs/**`
file changed; no existing golden compiled template changed.

## Acceptance

- `cargo test -p tangle-model` — passes (75 lib + 8 new `narrow_mode_templates`
  + every existing suite; `mode_template_compilation.rs` golden bundles, the
  migration goldens, and the facility and synthetic-template suites unchanged).
- `cargo build --workspace` and `cargo test --workspace` — pass.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `cargo fmt -p tangle-model -- --check` and `cargo clippy -p tangle-model
  --all-targets` — clean.
- `braintree check` — `graph check: passed (118 nodes)` while this node was still
  in `proposed/`.

Post-move note: after this file moves to `resolved/`, `braintree check`
transiently reports `next-resolved-node` because
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]] still names TAS-076 in
its `next`. That is expected; the coordinator advances the parent's `next`. No
parent or sibling node file was edited and no child node was created.
