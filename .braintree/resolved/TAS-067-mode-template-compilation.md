---
context_rev: 1
priority: P1
updated: 2026-09-13T16:08:28Z
summary: Compile and validate mode templates into agent components, rejecting impossible combinations.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

Mode templates compile into validated agent components, and the compiler rejects
impossible combinations such as transit dwell without capacity or articulation
parameters on a holonomic body, with stable diagnostics naming the authored
object.

# Done when

- Compilation of a valid template produces the expected component bundle.
- Invalid combinations fail validation with a stable diagnostic code and the authored template id.
- `cargo test -p tangle-model` passes with golden compiled output for a valid template.

# Context

Gated on [[TAS-066-compiled-agent-component-model]].

# Result

`tangle-model` now compiles validated mode templates into agent component
bundles and rejects impossible combinations with stable diagnostics. Three
`Done when` criteria hold.

- **A valid template compiles to its component bundle.**
  `crates/tangle-model/src/mode_template.rs` (new) adds the pure compiler
  `compile_mode_template(&ModeTemplateSource) -> Result<CompiledModeTemplate,
  Vec<Diagnostic>>` and the bundle `CompiledModeTemplate`. The bundle carries
  the template id plus the six TAS-066 components the authored template maps
  onto — `AgentBody` (Box/Circle), `AgentMotion`
  (HolonomicWalking/SingleBodyWheeled), `TacticalCapabilities`, `AgentAccess`
  (facility kinds; direction, speed policy, and rule kinds are deferred and
  default to either/unlimited/none), `AgentOccupancy`, and `AgentBehaviorProfile`
  — and the derived `AgentFamily`. The compiler reads only the authored fields
  and never branches on the template id: a test compiles two templates that
  differ only in id and asserts every component is equal.
- **Invalid combinations fail validation with a stable code naming the
  template.** `CompiledModeTemplate::validate` returns
  `E_MODE_TEMPLATE_BODY_MOTION` for a body/motion pair no family serves
  (including articulation parameters on a holonomic body) and the new
  `E_MODE_TEMPLATE_OCCUPANCY` for transit occupancy or stop service (dwell)
  without a passenger capacity, each naming the authored template id.
  `compile_mode_template` rejects the same combinations. Source validation
  (`validate_v2`) keeps its Increment 0 coverage: `validate_mode_templates` now
  derives the pair from the same `derive_family` the compiler uses, so the valid
  body/motion set has one definition. Transit, articulated, and `serve_stop`
  source shapes stay deferred to Increments 3/4 per `docs/schema-v2-contract.md`;
  the component-level validator already covers them, so landing those shapes
  needs only wiring, not new validation logic.
- **`cargo test -p tangle-model` passes with golden compiled output.**
  `crates/tangle-model/tests/mode_template_compilation.rs` (new) pins the exact
  compiled bundle for the Increment 0 `passenger_car` and `pedestrian` templates
  (an expected value compared in the test), plus the impossible-combination and
  compose-bridge cases.

Additive: `CompiledScenario`, every version-1/version-2 compiled field, and the
`compile_v2` v1-view materialization are unchanged, and the checked-in JSON
Schema is unchanged (no source shape was added). The only changes outside the new
module and its test are the additive
`DiagnosticCode::ModeTemplateOccupancy` code, `derive_family` becoming
crate-visible, and reusing it in `validate_mode_templates`.

Evidence: `cargo test -p tangle-model` passes (69 unit + 10 new integration
tests in `tests/mode_template_compilation.rs`); `cargo test --workspace` passes;
`cargo clippy --workspace --all-targets --all-features` is clean; `cargo fmt
--all --check` is clean; `scripts/check-dependency-direction.sh` reports
`dependency direction OK`.

Handoff: after this node moves to `resolved`, `braintree check` reports the
expected `next-resolved-node` diagnostic for TAS-058 because its last child is
resolved; TAS-058 itself stays open for its remaining criteria and must not be
edited from this session.
