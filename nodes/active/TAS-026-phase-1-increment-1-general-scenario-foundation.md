---
context_rev: 1
priority: P1
updated: 2026-09-12T15:36:25Z
summary: Phase 1 Increment 1 makes the scenario a general composition of boundaries, regions, paths, movements, portals, crossings, conflict regions, rules, and fixed-time signal phases that validate, compile, and render without scenario-specific branches.
next: Compile the new primitives to dense IDs and derived geometry in `CompiledScenario`, then render them through the shared `tangle-present` scene seam.
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

- `tangle-model` parses and semantically validates boundaries, regions, guide
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
  shared `tangle-present` scene seam.
- Property/fuzz inputs within declared limits never panic the parser or
  validator and either compile or return diagnostics.

# Context

Area [[IDX-001-tangle]].
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

First slice landed: the versioned source contract. Remaining slices are dense-id
compilation/derived geometry and viewer rendering; the node stays active.

- `crates/tangle-model/src/source.rs` adds `PolygonSource`,
  `MovementSource`, `CrossingSource`, `ConflictRegionSource`, `RuleSource`,
  `RuleKind`, `SignalSource`, `SignalHeadSource`, `SignalPhaseSource`,
  `SignalStateSource`, and `SignalColor`, with the new collections defaulted so
  existing scenarios keep parsing.
- `crates/tangle-model/src/validate.rs` adds 22 stable diagnostic codes and
  checks polygon vertices/area, movement portal/path references and
  self-loops, crossing and conflict-region references and arity, rule/signal
  agreement, signal head/phase completeness, conflicting greens, duplicate ids
  across all object kinds, and overlapping portals. A conflicting-green phase
  is rejected against authored conflict regions only, so the check uses no
  intersection type.
- `schemas/scenario-source.schema.json` is regenerated and its drift test
  passes.
- Three benchmark scenarios live in `scenarios/benchmarks/`:
  `straight_approach_v1`, `perpendicular_conflict_v1`, and
  `four_leg_signal_v1`. The four-leg layout is two roads, two movements, two
  crossings, one conflict region, and a four-phase fixed-time controller.

Evidence:

- `cargo test --workspace --all-features` passes; the model suite is 28 tests
  including `accepts_a_general_signalized_layout` and
  `flags_conflicting_greens`.
- `apps/tangle-cli/tests/scenarios.rs` loads every checked-in `.json5` scenario
  and asserts all three benchmarks compile with no scenario-kind branch.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, and `./scripts/check-dependency-direction.sh`
  pass; no new dependency edge was added.
