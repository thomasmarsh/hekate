---
context_rev: 2
updated: 2026-09-13T19:58:00Z
summary: Narrow modes spawn from their compiled template through the shared stages with a model and cards; the tactic suite and facility route completion are split to TAS-080.
next: '[[TAS-080-complete-narrow-longitudinal-tactics-and-compile]]'
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

# Result

Delivered the spawning, model, profile, and card slice of TAS-077; the
longitudinal-tactic suite and compiled-facility route completion are split to
[[TAS-080-complete-narrow-longitudinal-tactics-and-compile]]. This node stays
active with that child as its `next`. Done-when bullets 1, 3, and 4 hold;
bullet 2 does not yet.

## Spawn path

`Simulation` now selects a vehicle demand source's mode template and spawns
through the compiled [`AgentFamily`]. `CompiledScenario::demand_mode(DemandId)`
(the one additive, read-only `tangle-model` accessor: a new private
`demand_modes: Vec<Option<ModeTemplateId>>` parallel to `demand`, `None` on the
version-1 path, populated by `compile_v2`) is the demand-to-mode link.
`advance_vehicle_demand` looks it up per source and passes it to `try_admit`;
`try_admit` branches on `template.family() == Some(AgentFamily::WheeledCapsule)`,
never on a template id. A capsule source samples `crate::narrow::sample_narrow_profile`
from the same per-agent `profile`/`compliance` streams (keyed by agent id) and
projects it onto a shared `VehicleProfile`; every other source keeps the
unchanged passenger-car path. The narrow agent runs the same four stages as a
car — `query_vehicle_world` carries the narrow profile in the new
`VehicleObservation::narrow`, and `command_motion` dispatches to the narrow
wheeled model when it is present — so no shared interaction, event, metric, or
stage code names a mode.

## Narrow model, profile, and cards

- `crates/tangle-sim/src/narrow.rs` (new): the narrow wheeled family's isolated
  longitudinal model. `NarrowProfile` (public) carries the capsule body and the
  sampled parameters; `sample_narrow_profile` draws it from a compiled mode
  template in a fixed order; `NarrowProfile::vehicle_profile` projects onto the
  shared longitudinal profile; `NarrowWheeledController` /
  `IdmNarrowWheeledController` are the replaceable narrow model, the shared IDM
  law (`crate::control::idm_acceleration`, the IDM law refactored to take its
  four parameters so both families reach one implementation).
- The two model cards are the two `# Model card` blocks of `narrow.rs`'s module
  documentation (`# Model card — bicycle`, `# Model card — scooter`). Each states
  all twelve template sections in order (state; parameters; constants; decision
  inputs; longitudinal law; bounds; tie-breaks; emergency backstop; assumptions;
  parameter sources; validated ranges; known failure modes; incompatible
  fidelity settings) and names the model family, `NarrowWheeledController`, and
  `Simulation::emergency_cap_steps`.
- `crates/tangle-sim/src/controller.rs` holds the narrow model in
  `ControllerModels` and reports it in `ControllerModelNames { vehicle, narrow,
  pedestrian }`; `crates/tangle-sim/src/agent.rs` derives each agent's
  `BodyKind` once at spawn (capsule for a narrow profile) so snapshots report it
  with no mode branch, and stores the narrow profile;
  `Simulation::agent_narrow_profile` exposes it.

## Longitudinal tactics and states

The shared stages implement the tactic states for any path-following profile, so
a narrow agent selects the same `TacticReason`/`TacticTarget` set as a car:
`StopLine` (committed, `ConstraintClears`), `YieldCrossing` (committed),
`Follow` (preparing, leader target), and `FreeFlow`/`RouteComplete`. Narrow mode
now reaches them through `Observation::Vehicle`; a dedicated narrow test over a
leader, a stop line, a signal, and a compiled facility is TAS-080.

## No-id-branch guard

`crates/tangle-sim/tests/narrow_mode_no_branch.rs` (new) is the TAS-070-pattern
source-text guard over eight modules — `sim.rs`, `stage.rs`, `controller.rs`,
`control.rs`, `narrow.rs`, `event.rs`, `metrics.rs`, `safety.rs` — proving none
names `"bicycle"`/`"scooter"` as a Rust string literal in production code, plus a
falsification probe that the guard flags a branch and ignores prose and test
code, and a check that the fixture declares the ids the guard searches for.
`narrow.rs`'s unit test `the_command_depends_only_on_the_profile_not_the_template_id`
is the behavioral falsification: renaming a template id changes neither the
sampled profile nor the commanded acceleration.

## Test names and claims

- `crates/tangle-sim/tests/narrow_spawn.rs`
  `narrow_modes_spawn_from_their_compiled_template_with_their_body_and_profile`:
a 300 s run of the checked-in narrow fixture proves both capsule templates spawn
(bicycle- and scooter-length bodies), each narrow body matches its sampled
profile and reports `BodyKind::Capsule` while cars stay `Box`, every narrow speed
stays within its desired speed, and a narrow agent completes its route and
despawns.
- `crates/tangle-sim/tests/narrow_mode_no_branch.rs`: the guard above (3 tests).
- `crates/tangle-sim/tests/model_cards.rs`: `every_initial_model_card_states_the_full_model_inventory`
  now scans four cards (vehicle, pedestrian, bicycle, scooter) against the
template; `each_card_names_its_model_family_and_its_replaceable_interface`
asserts each narrow card names its mode, family, interface, and cap counter;
`the_seam_indexes_the_cards_and_interfaces` and
`the_checked_in_mixed_fixture_runs_the_documented_models` cover `idm-narrow`.
- `narrow.rs` unit tests: envelope sampling, distinct bicycle/scooter profiles,
the no-id-branch probe, acceleration/braking bounds, and that the narrow command
equals the shared IDM law on the projected vehicle profile.

## Files changed

`crates/tangle-sim/src/{narrow.rs (new), control.rs (IDM law split out),
controller.rs (narrow model slot), stage.rs (VehicleObservation::narrow),
agent.rs (body-kind and narrow-profile columns), sim.rs (spawn dispatch,
snapshot body kind, accessor), lib.rs (module and `NarrowProfile` export)}`;
`crates/tangle-sim/tests/{narrow_spawn.rs (new), narrow_mode_no_branch.rs (new),
model_cards.rs, mixed_interaction.rs}`; `crates/tangle-model/src/compiled.rs`
(the additive `demand_modes` field and `demand_mode` accessor). No
`crates/tangle-present/**`, `apps/**`, `baselines/**`, `schemas/**`, or `docs/**`
file changed; no Phase 1 golden trace, event, metric, or baseline changed.

## Acceptance

- `cargo test -p tangle-sim` — passes (147 lib + all suites, including the new
  `narrow_spawn`, `narrow_mode_no_branch`, and `model_cards`).
- `cargo test --workspace` — passes (67 result lines), Phase 1 golden traces
  unchanged (`cargo test -p tangle-cli --test golden_trace --test baseline
  --test migration_regression` passes: 4 + 2 + 4).
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features`
  and `cargo fmt --all --check` — clean.
- `braintree check` — `graph check: passed (118 nodes)` while this node was still
  in `proposed/` (before the split child and the move to `active/`).
