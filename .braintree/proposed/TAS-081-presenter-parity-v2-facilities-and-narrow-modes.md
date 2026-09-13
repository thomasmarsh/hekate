---
context_rev: 1
updated: 2026-09-13T21:29:57Z
summary: Load version-2 sources and render facility regions and narrow capsule bodies in the terminal and Bevy presenters.
next: Route the presenter load path through parse_scenario_document/compile_v2 and render facility regions and narrow capsule bodies.
---

Parent [[TAS-017-phase-2-mixed-traffic]].

# Outcome

`tangle-present` loads schema-version-2 documents (version 1 through migration,
version 2 directly through `compile_v2`), and the terminal and Bevy presenters
draw the Phase 2 Increment 1 features: continuous-width facility regions with
their reference paths, and bicycle/scooter capsule bodies with their sampled
dimensions. `tangle-tui` and `tangle-viewer` open the checked-in
`scenarios/phase2/inc1/*_v2.json5` fixtures and show the narrow agents and the
facilities they occupy. The scene format is extended additively and versioned so
Phase 1 rendering is unchanged.

This closes the Increment 1 presentation gap recorded in
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]]: the compiled facility
and narrow-mode data landed, but `tangle-present::load_scenario` still used the
version-1 reader, so the v2 narrow fixtures could not be opened by either viewer.

# Done when

- `tangle_present::load_scenario` negotiates the schema version through
  `parse_scenario_document`, compiling version 2 through `CompiledScenario::compile_v2`
  while version 1 keeps the migration path, and the existing Phase 1 scene output
  is unchanged.
- The scene representation carries facility regions (with their reference path)
  and narrow capsule bodies; both presenters render them, with any format change
  versioned and covered by a golden or drift test.
- `tangle-tui` and `tangle-viewer` open all six `scenarios/phase2/inc1/*_v2.json5`
  fixtures without a load error, and a test proves the v2 load path.
- `cargo test --workspace` passes and `scripts/check-dependency-direction.sh`
  reports OK.

# Context

Per `PHASE_2_PLAN.md` Increment 1 and the Increment 0 body-kind/segment
presentation. Read [[TAS-072-body-kind-and-segment-presenters]] (the landed
presenter scene format this extends), [[TAS-074-compile-version-2-facility-and-connector-shapes]]
(compiled facility/reference-path geometry), and
[[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]] (narrow capsule
bodies and profiles). The v2 source shapes and version negotiation are
[[TAS-062-version-2-source-shapes]].

Depends on [[TAS-074-compile-version-2-facility-and-connector-shapes]] at context_rev 1.
Depends on [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]] at context_rev 2.
Depends on [[TAS-072-body-kind-and-segment-presenters]] at context_rev 1.
