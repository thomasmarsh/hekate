---
context_rev: 1
status: proposed
updated: 2026-09-15T20:56:30Z
summary: node decompose duplicates a body parent route; planned-fixture closure unnamed.
tangle_revision: 1.0.0+ga6de4e4
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Decompose TAS-021 with `tangle node decompose --plan`, dispatch the first child as a slice whose approved change checks in a fixture named in the benchmark matrix as planned.
Friction: Two findings. (1) The plan body repeated the parent route as a `Parent [[TAS-021-...]].` line in `# Context`; `node decompose` had already prepended its canonical `Parent [[TAS-021-...]].` route, so every child carried two primary Parent links. `tangle check` reported `requires exactly one primary Parent or Area route` plus `orphan unfinished node`, and TAS-021 next then failed as `not a direct child`; the authoring reference plan example shows only `# Context` with `...` and never says the body must omit the parent route. (2) Checking in `scenarios/phase2/inc3/heavy_isolated_v2.json5` failed `benchmark_matrix` with `a planned fixture path must not be checked in yet`; the coordination reference compile-and-golden write-set closure does not name the planned-to-checked-in matrix move, so it surfaced only as a red test mid-slice.
Improvement: Two changes. (1) Document in `tangle help authoring` that `node decompose` writes the canonical `Parent [[PARENT]]` route and the plan body must not restate it, or make the plan validator reject a body whose first Parent link duplicates the route. (2) Add the benchmark-matrix planned-to-checked-in move (`docs/benchmark-matrix.md`, `docs/benchmark-matrix.json`, and the gate test) to the coordination reference examples of forced write-set closure.
