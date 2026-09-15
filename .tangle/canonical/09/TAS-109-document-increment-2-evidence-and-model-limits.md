---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T07:40:39Z
summary: Reconcile the benchmark matrix and narrow-mode cards with checked Increment 2 evidence.
next: Review the two narrow-mode model cards and the new matrix-consistency gate, then resolve TAS-109 or route any remaining acceptance work.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

The benchmark matrix and bicycle/scooter model cards point to checked Increment
2 evidence and state the implemented assumptions, ranges, limitations, and
incompatible fidelity settings without expanding the release claim.

# Done when

- Markdown and JSON matrix representations agree on checked passing, close-pass,
  prediction, and wrong-way fixture paths, presets, quantities, tolerances,
  baselines, supported/impossible/deferred cells, and zero remaining planned
  Increment 2 evidence.
- Bicycle and scooter cards describe lateral dynamics, decision cadence,
  predictor horizon, permissions, clearance-band interpretation, unsafe-commit
  behavior, wrong-way context, validated ranges, evidence, and failure modes.
- Cards explicitly exclude balance, lean, falls, biomechanics, implicit
  sidewalk riding, calibrated crash probability, and field calibration.
- Every evidence link exists and every numeric claim is copied from a resolved
  test result, not inferred; matrix consistency and model-card tests pass.
- No source, simulation, scenario, tolerance, or interaction disposition changes.

# Context

Gated on [[TAS-104-bound-predicted-versus-executed-clearance]],
[[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]],
[[TAS-106-check-in-increment-2-passing-fixtures]],
[[TAS-107-check-in-contextual-wrong-way-fixtures]], and
[[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]. Owns
docs/benchmark-matrix.md, docs/benchmark-matrix.json, and the existing bicycle
and scooter model cards only.

# Slices

- Reconcile the Markdown and JSON benchmark-matrix representations on the checked
  passing, close-pass, prediction, and wrong-way fixture paths, presets,
  quantities, tolerances, baselines, and supported/impossible/deferred cells.
- Update the bicycle and scooter model cards with the implemented assumptions,
  ranges, limitations, evidence, and the explicit exclusions.
- Verify every evidence link exists and every numeric claim is copied from a
  resolved test result; run matrix-consistency and model-card tests.

# Result

Delivered, node left active for review. No production, simulation, tolerance,
disposition, or matrix-cell change. The only edits outside documentation are
three stale section pointers in scenario comments and one new
documentation-consistency test; no fixture was added and no scenario value,
demand, or golden byte moved.

- **The two matrix representations already agreed on every axis the Done-when
  names, and both files are left byte-identical.** They state the same four
  checked-in Increment 2 ids and paths (`narrow_passing_v2`,
  `motor_passing_narrow_v2`, `motor_lane_change_v2`, `narrow_wrong_way_v2` under
  `scenarios/phase2/inc2/`), the same `CC-OVERTAKE`/`CC-OPPOSE` presets (`S`,
  `f`), quantities, tolerances (`T-O1`/`T-O2`/`T-O3` and `T-H1`/`T-H2`) and
  baselines (Increment 2 declared reference), the same 224-cell grid
  (157 supported / 67 impossible / 0 deferred with matching per-family counts),
  the same 16 cell classes and 22-entry tolerance catalogue, and zero planned
  Increment 2 evidence. TAS-130 and TAS-132 had already moved the passing and
  wrong-way entries, so reconciliation required no matrix edit; what was missing
  was the check that holds the two representations together.
- **`apps/hekate-cli/tests/benchmark_matrix.rs` is that check (new, 5 tests).**
  It parses `docs/benchmark-matrix.json` and holds it to
  `docs/benchmark-matrix.md`: the mode, family, and impossible-reason axes (§4.1,
  §4.2, §4.3); every `(pair, family)` cell's disposition with its class or
  reason, plus the disposition and per-family counts against the actual grid;
  every cell class's tolerance ids and fidelity presets against §5 and the §6
  catalogue; the fixture tables' ids and paths, with every checked-in path
  required to exist on disk, every planned path required not to, and every path
  §7.4 names required to resolve; and the Done-when's own clause — no
  `scenarios/phase2/inc2/` path may remain in either planned representation,
  while both must name exactly the four wired checked-in Increment 2 ids. It
  runs no simulation. Falsified by hand: flipping `bicycle__bicycle`/`overtaking`
  from `S:CC-OVERTAKE` to `I:OP` in the JSON, and moving
  `narrow_wrong_way_v2` back into `planned_patterns`, each fail the suite for the
  intended reason with the drifting cell or path named.
- **Both narrow-mode model cards now document the landed Increment 2 behavior**
  inside the template's section order, with the new sections between `Decision
  inputs` and `Bounds`: `## Lateral dynamics` (the compiled projection, the 2.0 s
  `LATERAL_APPROACH_S` approach, the
  `min(steering_rate_max_rad_s, lateral_accel_max_mps2 / max(v, MIN_SPEED_FOR_LATERAL_BOUND_MPS))`
  clamp with the 0.5 m/s floor, `d_dot = v * sin(theta_error)` with no target
  written to `d`, and the 1e-9 m corridor test); `## Decision cadence and
  predictor horizon` (tactical clauses at the run's lateral-decision cadence,
  which the kernel resolves as the fixed step — 0.05 s Standard, 0.02 s Fine,
  0.1 s Fast — with the safety clauses every step; horizon from the mode's
  authored `lateral.horizon_s`, 2.0 s in every checked-in lateral fixture,
  predicted at `DEFAULT_SUBDIVISIONS` = 8 per cadence); `## Permissions and
  tactic eligibility` (the rejection codes including `no_permission` and
  `boundary_forbidden`, and `T-O3`); `## Clearance bands and close-pass
  evidence` (the 0.5 m violating and 2.0 m study bands, the 0.78 m safe pass, and
  the 0.4001 m at 44.75 s unsafe pass recording band 0); `## Unsafe-commit
  response` (the 0.25 m floor and 2.0 s hold every checked-in lateral fixture
  authors, the ordered brake/hold/abort, and the one-cap-per-executed-pass
  relationship); `## Wrong-way context` (all eight reason codes, the legality
  separation, and the recorded fact that no production demand path calls
  `request_wrong_way_entry` yet); `## Evidence`; `## Exclusions`; and an
  `## Incompatible fidelity settings` section stating the lateral motion's preset
  dependence. `## State`, `## Parameters`, `## Constants`, `## Decision inputs`,
  `## Bounds`, `## Tie-breaks`, `## Emergency backstop`, `## Assumptions`,
  `## Parameter sources`, `## Validated ranges`, and `## Known failure modes`
  were updated to match: lateral state and inputs exist, the lateral parameters
  are read rather than merely carried, the claim-arbitration key is stated, and
  `## Validated ranges` now separates the Increment 1 envelope from the
  Increment 2 lateral envelope and names the number outside the Increment 1
  ranges (the constant 9.0 m/s bicycle desired speed, above the 3.5–6.5 m/s
  template range). Every number in either card is either a named source constant
  (`LATERAL_APPROACH_S` 2.0 s, `MIN_SPEED_FOR_LATERAL_BOUND_MPS` 0.5 m/s,
  `CORRIDOR_TOLERANCE_M` 1e-9 m, `SETTLE_TOLERANCE_M` 1e-3 m,
  `DEFAULT_SUBDIVISIONS` 8, `DEFAULT_STEP` 0.05 s) or a value a checked-in
  fixture or a resolved test result records (the 0.25 m/2.0 s commit policy, the
  0.4 m/0.75 m targets at a 2.0 s horizon, the 0.78 m and 0.4001 m passes, `T-O1`
  0.10 m with measured worst 0.0139/0.0071/0.0028 m, the -1.3e-15 m least bumper
  gap, the seed bank {11, 48, 102}), including the exclusions the Done-when
  requires (balance, lean, falls, biomechanics, implicit sidewalk riding,
  calibrated crash probability, and field calibration).
- **Three stale section pointers fixed, comment-only.**
  `scenarios/phase2/inc2/{narrow_passing_v2,motor_passing_narrow_v2,narrow_wrong_way_v2}.json5`
  still said §7.3 named them after TAS-130/TAS-132 moved them into §7.2; they now
  say §7.2. No scenario value, demand, or tolerance changed, and the Increment 2
  goldens and determinism suites still pass.
- **One scope boundary, recorded rather than decided silently.** The two variant
  fixtures TAS-130 checked in (`narrow_passing_unsafe_v2`,
  `motor_lane_change_boundary_v2`) are named by neither matrix representation and
  were left that way: the brief records the passing and wrong-way matrix entries
  as already wired, and this node may change no matrix cell. If review wants
  every checked-in Increment 2 fixture named by §7.2 and the JSON, that is a
  two-entry additive edit in both files — no cell, tolerance, or disposition
  change — and the new consistency test passes in either state as long as both
  representations agree.
- **Recorded limitation the cards now state rather than hide:** the `CC-OPPOSE`
  evidence is driven in-process through `Simulation::request_wrong_way_entry`, and
  no production demand path calls that seam, so a CLI run of the wrong-way
  fixture opens no opposing interval on its own. That is TAS-131's and TAS-133's
  recorded boundary, unchanged here.
- **Deliberately unchanged:** any simulation behavior, tolerance, disposition, or
  matrix cell; `docs/schema-v2-contract.md`; the presenter layer and landed
  seams; every golden, baseline, and Phase 1 artifact; and every scenario demand,
  geometry, and policy value.

Verified on this commit: `cargo fmt --all --check` clean; `cargo clippy -p
hekate-sim -p hekate-cli --all-targets --all-features -- -D warnings` clean;
`cargo test -p hekate-cli --test benchmark_matrix` 5 passed, `--test
migration_regression` 4 passed, `--test inc2_trace` 7 passed, `--test
inc2_determinism` 4 passed, `--test run_metrics` 22 passed; `cargo test -p
hekate-sim --test model_cards` 4 passed, `--test narrow_isolated` 6 passed,
`--test prediction` 15 passed, `--test wrong_way` 20 passed, `--test
inc2_passing_fixtures` 5 passed; and `docs/benchmark-matrix.json` parses.
`cargo test --workspace` and the full five-gate run were not executed here, per
the brief.
