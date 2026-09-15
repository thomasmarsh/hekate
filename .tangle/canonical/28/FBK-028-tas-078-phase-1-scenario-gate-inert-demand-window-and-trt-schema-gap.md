---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: TAS-078 surfaced an undocumented Phase 1 gate on scenarios/**, inert demand interval_s, and a T-RT bound the authored schema cannot express.
tangle_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Resolved TAS-078 (six narrow Increment 1 fixtures under
scenarios/phase2/inc1, their hekate-sim integration tests, and the
benchmark-matrix de-planning) as the sole writer for the node.

Friction: Three findings, each discovered only after coding.

1. Adding the first version-2 scenario under `scenarios/` broke
   `apps/hekate-cli/tests/migration_regression.rs`, whose gate required every
   checked-in `*.json5` under `scenarios/` to be a version-1 source that
   migrates and reproduces the original reader's event body. Neither
   `SKILL.md` nor the task brief named any test gate that enumerates the
   directory a new artifact lands in, and the increment's fixture path is fixed
   by `docs/benchmark-matrix.md` under `scenarios/`. The task did allow
   `apps/hekate-cli/**` test wiring, so the gate was widened to validate and
   compile a version-2 document directly (version-1 documents keep the full
   differential), but the collision was avoidable only by knowing that gate
   existed before choosing to check a fixture in at the matrix path.
2. The authored `demand[].spawn.rate.interval_s` window (`start_s`/`end_s`) is
   parsed and validated but never simulated: `advance_vehicle_demand` draws
   arrivals at `rate_per_hour` from `t = 0` and never reads the window. The
   schema treats the field as meaningful, nothing marks it inert, and no test
   covers it. A crossing fixture designed to stagger the two modes by demand
   window silently overlapped them and had to be redesigned.
3. `docs/benchmark-matrix.md` `T-RT` names `narrow_isolated_curve_v2` as the
   "Increment 1 declared analytic path reference", but authored
   `paths[].points` compile only to polylines
   (`CompiledReferencePath::from_polyline`). A chord polyline cannot meet the
   bound: its own round trip at a lateral offset near a vertex lands on the
   neighbouring chord, measured at ~0.05 m for a 24-segment quarter circle at
   `d = 0.8 m`. Meeting `T-RT` required reconstructing the compiled
   `CompiledReferencePath::arc` the fixture declares and proving the authored
   vertices lie on it — an evidence-form judgment call the brief did not cover,
   which was escalated to the supervisor for a ruling.

Improvement: Three concrete changes.

1. Give each node or increment brief a "gates my artifact enters" line naming
   the existing test suites that enumerate a directory, so a writer knows
   before coding that a new path under `scenarios/` changes a Phase 1 gate.
2. `docs/schema-v2-contract.md` (or the schema) should mark every authored
   field the kernel does not yet read — `demand[].spawn.rate.interval_s` today —
   and the matrix or schema test should fail a fixture that relies on an inert
   field, so a fixture cannot be designed around scenario data the kernel
   ignores.
3. The benchmark-matrix template should require every analytic-reference
   tolerance to name the schema construct that expresses the reference. A
   `T-RT`-style bound then cannot be assigned to a fixture form the authored
   schema cannot author without the gap being visible when the matrix is
   written, not when the fixture is.
