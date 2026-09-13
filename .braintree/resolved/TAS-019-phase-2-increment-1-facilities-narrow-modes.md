---
context_rev: 2
priority: P1
updated: 2026-09-13T22:10:07Z
summary: Increment 1 is complete: facilities, narrow modes, validation, isolated fixtures, reproducibility, and viewer parity all landed.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 1:

- Continuous-width facilities, reference-path coordinates (arc length `s`,
  signed lateral offset `d`, tangent, normal, curvature), directional
  traversal, access rules, and movement connectors.
- Version-2 validation for containment, usable width, curvature against turning
  limits, connector continuity, directional reachability, mode-to-facility
  access, legal versus physically possible routes, and spawn clearance for the
  largest eligible body.
- Bicycle and scooter templates, demand, profiles, bodies, isolated controllers,
  and model cards.
- Longitudinal following, stops, signals, priorities, facility selection, and
  route completion before free lateral maneuvers are enabled.
- Isolated straight, curve, braking, following, signal, and crossing fixtures
  for both modes.
- Facility regions and narrow capsule bodies are observable in the terminal and
  Bevy viewers, and the Increment 1 fixtures open through the version-2 load
  path.

# Done when

- Path/world coordinate round trips remain within declared tolerance on
  straight and curved fixtures.
- Bicycle and scooter motion respects dimensions, speed, acceleration, braking,
  steering, and facility boundaries.
- Repeated seeded runs reproduce demand, profiles, decisions, events, and trace
  hashes.
- Each mode passes independent fixtures without relying on car-specific
  dimensions or controller defaults.
- The terminal and Bevy viewers open the Increment 1 fixtures and render their
  facility regions and narrow bodies without a version-2 load error.

Parent [[TAS-017-phase-2-mixed-traffic]].

# Result

Increment 1 is complete; all five deliverables and all four gate criteria hold
through resolved children, with no child disposed.
[[TAS-081-presenter-parity-v2-facilities-and-narrow-modes]] closed the
observability gap: `tangle_present::load_scenario` negotiates the schema version
(version 2 through `compile_v2`, version 1 through migration), the scene carries
facility regions with their reference paths and renders narrow capsule bodies,
and `tangle-tui` and `tangle-viewer` open all six Increment 1 fixtures. Phase 1
scene output is unchanged apart from the declared `SCENE_FORMAT_VERSION` 1->2
bump, which adds one `facilities: []` line to the walking golden.

- **Facility contract and geometry.** [[TAS-073-extend-the-version-2-schema-contract-with-increm]]
extended `docs/schema-v2-contract.md` in place with the Increment 1 facility,
connector, permission, and narrow-template shapes and the compiled coordinate
contract; [[TAS-074-compile-version-2-facility-and-connector-shapes]] landed the
version-2 source structs and compiled reference-path geometry (`length`,
`s`/signed-`d`, tangent, normal, curvature, usable lateral interval, connectors,
and separate nominal/permitted/physically-possible directions) with an analytic
`T-RT` test; [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]] added
the validation rules with stable diagnostics, `ModeBodySource::Capsule`, and the
required-nullable speed limit, and proved no malformed facility panics.
- **Narrow templates and longitudinal behavior.** [[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]]
compiled the bicycle/scooter templates to component bundles with their own
bodies and steering/clearance profiles and no id branch;
[[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]] wired
component-driven spawning, the narrow wheeled model, and the bicycle/scooter
model cards; its resolved child
[[TAS-080-complete-narrow-longitudinal-tactics-and-compile]] proved command
envelopes and body bounds across accelerating, following, holding a stop line,
yielding at a signal, and completing a compiled-facility route.
- **Fixtures.** [[TAS-078-check-in-narrow-mode-isolated-fixtures-and-tests]]
checked in the six `scenarios/phase2/inc1/*_v2.json5` fixtures (straight, curve,
braking, following, signal, crossing) for both modes, the `narrow_isolated`
tolerance suite (`T-RT`/`T-DIM`/`T-ENV`/`T-SPD` at F/S/f), and the benchmark-matrix
de-planning, with no tolerance, cell, or preset widened.
- **Reproducibility and independence.** [[TAS-079-prove-narrow-mode-reproducibility-and-independen]]
proved each fixture reproduces its trace hash at every preset, a declared seed
bank reproduces per-seed hashes and event streams, and a falsification probe
flags a narrow agent whose body or limits come from Phase 1 passenger-car
defaults.

Gate criteria traced to committed evidence: round trips -> TAS-074/TAS-078
(`T-RT <= 1e-9 m`); dimensions/speed/acceleration/braking/steering/facility
boundaries -> TAS-077/TAS-080 and the `T-DIM`/`T-ENV`/`T-SPD` bounds in
TAS-078; seeded reproducibility -> TAS-079; per-mode independent fixtures with
no car defaults -> TAS-078 and TAS-079.

Commit trail: `5661642` (contract), `f768504` (compiled geometry), `274f4a0`
(validation), `6917ffb` (templates), `6a84b1d` (spawn/model/cards), `a2f5ee0`
(longitudinal evidence, TAS-077 roll-up), `616bde0` (fixtures), and the TAS-079
commit. `cargo test --workspace` and `scripts/check-dependency-direction.sh`
pass; `braintree check` passes (121 nodes); no Phase 1 golden trace, event or
metric version, or `baselines/phase1/**` changed.

Limitations carried forward: authored `paths[].points` compile to polylines, so
exact curved references need an authored analytic construct (the curve fixture
proves `T-RT` on its declared analytic reference; see `FBK-028`); the authored
`demand[].spawn.rate.interval_s` window is parsed but not simulated (also
`FBK-028`); free lateral maneuvers, lane/facility transitions, overtaking, and
wrong-way selection are Increment 2; facility-region and narrow-body
presentation in `tangle-present`/viewers was not required by the increment and
remains for the release demonstration.
