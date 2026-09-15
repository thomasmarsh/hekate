---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Phase 1 Increment 1 makes the scenario a general composition of boundaries, regions, paths, movements, portals, crossings, conflict regions, rules, and fixed-time signal phases that validate, compile to dense ids with derived geometry, and render without scenario-specific branches.
---

# Outcome

A scenario author can describe any small intersection as data rather than by
naming an intersection type: boundaries, traversable regions, guide paths,
portals, movement connectors, pedestrian crossings, conflict regions, movement
rules, and fixed-time signal phases. Invalid references, degenerate geometry,
unreachable routes, and conflicting signal states fail before time zero with
stable, source-linked diagnostics. The primitives compile to dense IDs and
derived geometry and render in the viewers. Three benchmark layouts — a straight
approach, perpendicular conflicting paths, and a four-leg signalized
intersection — need no `IntersectionType` enum or scenario-specific branch in
the simulator.

This node owns the Increment 1 slice of `PHASE_1_PLAN.md`; a fresh worker may
continue it. Behavior (signals changing state, movements being traversed) is
Increment 2 work and is out of scope here — Increment 1 compiles and renders the
authored structure only.

# Done when

- `hekate-model` parses and semantically validates boundaries, regions, guide
  paths, movements, portals, crossings, conflict regions, rules, and fixed-time
  signal phases, with a stable diagnostic code and source object id for every
  failure.
- The checked-in `schemas/scenario-source.schema.json` matches the generated
  schema.
- The compiled scenario exposes the new primitives with dense IDs and derived
  geometry (polygon area/centroid, movement endpoints, signal cycle length).
- The three benchmark scenarios load, validate, and compile, and the simulator
  contains no `IntersectionType` enum or scenario-id branch.
- Every geometry and control primitive renders in the viewers through the
  shared `hekate-present` scene seam.
- Property/fuzz inputs within declared limits never panic the parser or
  validator and either compile or return diagnostics.

# Context

Area [[IDX-001-hekate]].
Depends on [[TAS-001-phase-1-increment-0]] at context_rev 1.

Increment 0 delivered one guide path and two portals as the whole schema.
`PHASE_1_PLAN.md` Increment 1 generalizes that to the primitives above and gates
the increment on the benchmark layouts needing no scenario-specific simulator
branch. `VISION.md` requires declarative, versioned, schema-validated,
diffable scenarios independent of the visualization layer, and describes a
movement as connecting regions or paths with direction, right-of-way, and
applicable controls.

Schema version 1 is extended here because these are Phase 1 car/pedestrian
primitives. `PHASE_2_PLAN.md` reserves schema version 2 and an explicit
version-1 migration for mixed-traffic facilities, so no Phase 2 field is added.

Tracks the unblock action of [[TAS-018-phase-2-increment-0-baseline-extension-contract]];
the Phase 1 definition-of-done checklist in `baselines/phase1/entry-gate.md`
stays open until this increment lands.

# Result

Increment 1 is complete. Three slices landed across `hekate-model`,
`hekate-present`, and the two viewers.

- Source contract: `crates/hekate-model/src/source.rs` defines `PolygonSource`,
  `MovementSource`, `CrossingSource`, `ConflictRegionSource`, `RuleSource`,
  `RuleKind`, and the fixed-time signal types. `validate.rs` adds 22 stable
  diagnostic codes and rejects bad references, degenerate geometry, duplicate
  ids across all object kinds, overlapping portals, and conflicting greens
  against authored conflict regions only, so no check names an intersection
  type.
- Compilation: `crates/hekate-model/src/compiled.rs` compiles every primitive
  to a dense id with derived geometry — `BoundaryId`, `RegionId`, `MovementId`,
  `CrossingId`, `ConflictRegionId`, `RuleId`, and `SignalId`; polygon area and
  centroid; movement entry/exit endpoints and headings; and signal cycle length
  with per-phase, per-head colors. `IdMap` names every compiled collection.
- Rendering: `crates/hekate-present/src/scene.rs` projects boundaries, regions,
  movements, crossings, conflict regions, rules, and signal heads into
  `SceneGeometry`, and the character-cell, Kitty/pixel, and Bevy backends each
  draw every primitive through that shared seam. Rule markers and signal gates
  are offset so both stay visible at a shared movement entry.

Evidence:

- `cargo test --workspace --all-features` passes: `hekate-model` 33 unit + 3
  fuzz/robustness + 1 walking integration; `hekate-present` 23 unit + 2 scene
  golden; `hekate-tui` 66 unit + golden/parity integration suites; `hekate-cli`
  scenario tests assert every benchmark exposes all compiled primitives with
  entries in the id map.
- `crates/hekate-model/tests/fuzz_scenarios.rs` drives arbitrary text and
  arbitrary source structures within declared limits through parse, validate,
  and compile and asserts no panic and a compile-or-diagnostics outcome.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, and `./scripts/check-dependency-direction.sh`
  pass; no new dependency edge was added.
- `schemas/scenario-source.schema.json` still matches the generated schema; the
  shared scene golden was regenerated for the extended `SceneGeometry`.
