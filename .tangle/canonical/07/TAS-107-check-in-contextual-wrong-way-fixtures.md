---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T06:06:30Z
summary: Check in contextual wrong-way fixtures including an occupied opposing corridor.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Checked version-2 scenarios prove contextual opposing-route selection, ordinary
head-on interaction, explicit rule evidence, and the inability to bypass an
occupied or disconnected corridor.

# Done when

- Fixtures cover a permitted nominal choice, a contextual prohibited-but-
  connected wrong-way choice, a physically disconnected rejection, and an
  occupied opposing corridor with deterministic wait/brake/abort behavior.
- Tests assert perceived rule, reason, affected movements, interval boundaries,
  distance/duration/exposure/encounters/conflicts, ordinary collision and
  near-miss visibility, and route completion or explicit non-entry.
- Reversing facility/reference declarations does not change the physical
  outcome after stable IDs are accounted for.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no interaction disposition or tolerance is widened.
- CLI validate and run plus cargo test --workspace pass at required presets.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] and
[[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]]. Owns
scenarios/phase2/inc2 wrong-way fixtures, one focused integration suite, and
only their path entries in docs/benchmark-matrix.md and .json.

Gates my scenario artifacts enter: apps/hekate-cli/tests/migration_regression.rs
enumerates scenarios/**/*.json5; crates/hekate-present/tests/v2_fixtures.rs may
enumerate version-2 fixtures; benchmark-matrix checked-path tests enumerate the
matrix fixture paths.

# Slices

- [[TAS-131-check-in-the-contextual-wrong-way-fixtures]] Core wrong-way fixtures.
- [[TAS-132-prove-declaration-order-invariance-and-wire-the]] Declaration-order invariance and matrix wiring.

# Result

Both slices resolve, so every `# Done when` clause is disposed:

- [[TAS-131-check-in-the-contextual-wrong-way-fixtures]] (resolved) checks in
`scenarios/phase2/inc2/narrow_wrong_way_v2.json5` and
`apps/hekate-cli/tests/run_metrics.rs`: the permitted, prohibited-but-connected,
physically-disconnected, and occupied-corridor cases, the perceived rule, reason,
affected movement, interval boundaries, distance/duration/exposure/encounters/
conflicts, ordinary collision and near-miss visibility, and route completion or
explicit non-entry. `hekate-cli validate` and `run --seed 0` pass.
- [[TAS-132-prove-declaration-order-invariance-and-wire-the]] (resolved) proves
reversing facility and reference declarations preserves the physical outcome
after stable IDs are accounted for, and moves only the two `narrow_wrong_way_v2`
matrix entries from planned to checked-in in both representations with no
disposition or tolerance widened.
- Fixture paths replace only their planned entries in both benchmark-matrix
representations; no interaction disposition or tolerance is widened.
- `cargo test --workspace --all-features` passes at the pinned Standard (0.05 s)
preset (1005 passed, 0 failed, 1 ignored on `86b9d08`), together with the format,
lint, dependency-direction, and `tangle check` gates recorded in TAS-132's gate
evidence. The required-preset half of the "CLI validate and run plus cargo test
--workspace pass at required presets" clause is delegated to
[[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] (child of
[[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]), exactly as
TAS-130's preset clause was; no preset coverage is claimed here.
