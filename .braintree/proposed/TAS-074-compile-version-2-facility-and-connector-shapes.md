---
context_rev: 1
updated: 2026-09-13T18:28:57Z
summary: Compile version-2 facility and connector shapes to reference-path geometry in tangle-model.
next: Add version-2 facility and connector source shapes and compiled reference-path geometry to tangle-model.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

`tangle-model` parses the Increment 1 version-2 facility and connector source
shapes and compiles them into geometry: a continuous-width facility region with
an optional reference path exposing arc length `s`, signed lateral offset `d`,
tangent, normal, and curvature; usable lateral intervals after the current body
envelope and configured clearance; facility adjacency and connectors across
traversal directions; and separately authored nominal/permitted versus physically
possible traversal directions. `CompiledScenario` carries compiled facilities and
compiled mode templates instead of only materializing them into the version-1
view, while version-1 and existing version-2 compiled behavior stay unchanged.

# Done when

- The version-2 source structs for facilities, facility connectors, access, and
  permission/obligation shapes exist with `JsonSchema`, and
  `schemas/scenario-source.schema.json` is regenerated with the checked-in-schema
  drift test passing.
- `CompiledScenario` exposes compiled facilities (region, reference-path
  coordinates, usable intervals, connectors, and both direction properties) and
  the compiled mode-template bundles; `compile_v2` populates them.
- A checked-in analytic test proves path-to-world-to-path round-trip error
  `<= 1e-9 m` (`T-RT`) on a straight facility and a constant-curvature facility,
  using the compiled reference path, and that the usable lateral interval
  subtracts the body envelope and clearance.
- `cargo test -p tangle-model` and `cargo build --workspace` pass; no Phase 1
  golden trace or baseline changes.

# Context

Gated on [[TAS-073-extend-the-version-2-schema-contract-with-increm]]. Per
`PHASE_2_PLAN.md` *Facilities, paths, and free space* and *Schema version 2*. Read
[[TAS-061-version-2-schema-contract]], [[TAS-062-version-2-source-shapes]], and
[[TAS-066-compiled-agent-component-model]] for the compiled seams. Owns
`crates/tangle-model/src/source.rs`, `crates/tangle-model/src/compiled.rs`, and
`schemas/scenario-source.schema.json`. This leaf is the facility/coordinate
contract every controller and fixture leaf consumes.
