---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Check in the six narrow Increment 1 isolated fixtures, their integration tests, and their benchmark-matrix paths.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

The Increment 1 checked-in fixtures exist and run end to end for both `bicycle`
and `scooter`: `scenarios/phase2/inc1/narrow_isolated_straight_v2.json5`,
`narrow_isolated_curve_v2.json5`, `narrow_isolated_braking_v2.json5`,
`narrow_following_v2.json5`, `narrow_signal_v2.json5`, and
`narrow_crossing_v2.json5`. `docs/benchmark-matrix.md` is revised so each
`CC-NARROW` fixture path it planned is replaced by the checked-in path.

# Done when

- Each fixture is a valid version-2 document that compiles and runs through the
  CLI; the `CC-NARROW` class fixtures (`narrow_isolated_straight_v2`,
  `narrow_isolated_curve_v2`) satisfy `T-RT`, `T-DIM`, `T-ENV`, and `T-SPD` at
  the `F/S/f` presets.
- Integration tests in `crates/hekate-sim/tests/` run straight, curve, braking,
  following, signal, and crossing behavior for both modes and assert body
  dimension, command-envelope, and steady-state free-speed bounds.
- `docs/benchmark-matrix.md` no longer lists an Increment 1 fixture as
  "planned"; each named path exists, and its cell, quantity, tolerance, baseline,
  and presets are unchanged (no widened disposition or relaxed tolerance).
- `cargo test --workspace` passes and the new fixtures are listed in the matrix.

# Context

Gated on [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]]. Per
`PHASE_2_PLAN.md` *Increment 1* (isolated straight, curve, braking, following,
signal, and crossing fixtures for both modes) and *Validation strategy*
(isolated operational rung). Read [[TAS-059-benchmark-matrix-and-tolerances]] and
`docs/benchmark-matrix.md` (classes `CC-NARROW`, `CC-FOLLOW`, `CC-CROSS`; tolerances
`T-RT`, `T-DIM`, `T-ENV`, `T-SPD`). Owns `scenarios/phase2/inc1/*`,
`crates/hekate-sim/tests/`, and the fixture paths in `docs/benchmark-matrix.md`.

# Result

All four `Done when` bullets hold. All six fixtures exist under
`scenarios/phase2/inc1/`, load and run through the CLI, are exercised by
`crates/hekate-sim/tests/narrow_isolated.rs`, and are listed in the matrix, which
no longer lists an Increment 1 fixture as planned.

## Fixtures (all new, all version 2, all `cc-narrow`/increment 1)

| Path | Modes | Behavior |
| --- | --- | --- |
| `scenarios/phase2/inc1/narrow_isolated_straight_v2.json5` | bicycle, scooter | free-flow straight, one facility per mode |
| `scenarios/phase2/inc1/narrow_isolated_curve_v2.json5` | bicycle, scooter | free-flow constant-curvature curve, one facility per mode |
| `scenarios/phase2/inc1/narrow_isolated_braking_v2.json5` | bicycle, scooter | red-only signal holds an authored stop line, both modes brake to rest |
| `scenarios/phase2/inc1/narrow_following_v2.json5` | bicycle, scooter | saturated arrivals queue narrow agents behind narrow leaders |
| `scenarios/phase2/inc1/narrow_signal_v2.json5` | bicycle, scooter | 30 s red then 120 s green; both modes yield, hold, and complete the route |
| `scenarios/phase2/inc1/narrow_crossing_v2.json5` | bicycle, scooter | two facilities cross at an authored conflict region; both modes traverse it |

Every template authors constant (`min == max`) capsule bodies and desired
speeds, so the sampled profile is the authored value. `narrow_isolated_curve_v2`
authors its guide paths as the chord polylines of quarter circles (radius 80 m
bicycle, 86 m scooter, centre the origin, start angle `-pi/2`, sweep `+pi/2`)
and declares that analytic reference in the fixture header.

## Tolerance tests and observed bounds

`crates/hekate-sim/tests/narrow_isolated.rs` (new), six tests, seed
`20_260_913`:

- `the_narrow_isolated_straight_fixture_holds_trt_tdim_tenv_and_tspd_at_every_preset`
  and `the_narrow_isolated_curve_fixture_holds_trt_tdim_tenv_and_tspd_at_every_preset`
  run at `F` (0.1 s), `S` (0.05 s), and `f` (0.02 s), which the matrix fixes for
  a `CC-NARROW` cell, and hold each fixture to `T-RT`, `T-DIM`, `T-ENV`, and
  `T-SPD`:
  - `T-RT` (`<= 1e-9 m`) — observed `0` to `1.33e-15 m` on the straight
    fixture's compiled reference; `2.01e-14` to `2.84e-14 m` on the curve
    fixture's declared analytic reference, at every preset.
  - `T-DIM` (`<= 1e-12 m`) — realized capsule length and `2 * radius` equal the
    authored constant exactly (error `0`).
  - `T-ENV` (`= 0` violations) — every sample's speed is in `[0, v0]` and the
    implied per-step acceleration is in `[-comfortable_brake, +max_accel]`;
    `Simulation::emergency_cap_steps() == 0` on both free-flow fixtures.
  - `T-SPD` (`<= 1e-6 m/s` after the settling distance) — the undisturbed
    leading agent on each facility reports its authored speed exactly
    (error `0`) at every preset.
- `the_narrow_isolated_braking_fixture_brakes_both_modes_to_the_authored_stop_line`
  — both modes approach in free flow, come to rest with the front bumper within
  0.5 m of the authored 60 m line, and never bind the emergency cap.
- `the_narrow_following_fixture_queues_both_modes_behind_a_narrow_leader` —
  both modes queue a follower inside a 4 m gap, slower than its own desired
  speed, with no overlapping bodies; the leading agent on each facility stays at
  its authored free speed.
- `the_narrow_signal_fixture_yields_then_completes_both_modes_routes` — both
  modes record a `Stop` decision under the red head, despawn `ExitedPath`, and
  ride their authored free speed downstream of the line.
- `the_narrow_crossing_fixture_sends_both_modes_through_the_conflict_region` —
  both modes emit a conflict-region `Entry`, complete their route, and ride
  their authored free speed.

## Recorded limitation: `T-RT` for the curve fixture

Authored facility references compile to polylines (`CompiledReferencePath::from_polyline`);
the only exact constant-curvature reference is the compiled
`CompiledReferencePath::arc` geometry, and the authored version-2 schema cannot
express an arc. A chord polyline alone does not round-trip to `1e-9 m` at
lateral offsets near vertices — it lands on the neighbouring chord, measured at
~0.05 m for a 24-segment quarter circle at `d = 0.8 m`. The matrix's `T-RT`
baseline names `narrow_isolated_curve_v2` as the "Increment 1 declared analytic
path reference", so the test proves `T-RT` on the analytic reference the fixture
declares, reconstructed deterministically from the fixture's own vertices, and
also asserts that every authored vertex lies on that arc within `1e-9 m` and
that the chord polyline is strictly shorter than the arc. Exact authored curved
references (an arc/constant-radius option on the authored facility reference)
remain scope a later increment would own; the coordinator tracks it, and no
child node was created for it.

## Fidelity limitation: curve T-RT needs a declared analytic reference

Authored `paths[].points` compile only to polylines, and a chord polyline alone
does not round-trip to `T-RT <= 1e-9 m` at lateral offsets near a vertex
(measured about 0.05 m for a 24-segment quarter circle at `d = 0.8 m`). The
matrix's `T-RT` baseline is the "Increment 1 declared analytic path reference",
so `narrow_isolated_curve_v2` declares its quarter-circle centre/radius/start
angle/sweep and the test reconstructs `CompiledReferencePath::arc` from the
fixture, asserts `T-RT <= 1e-9 m` on that reference across the usable lateral
band, and asserts every authored vertex lies on the arc within 1e-9 m. An
authored-schema construct for exact curved references (so the compiled geometry
is itself the analytic arc) is remaining scope outside this node, tracked as
feedback `[[FBK-028-tas-078-phase-1-scenario-gate-inert-demand-window-and-trt-schema-gap]]`.

## Matrix de-planning (no disposition, tolerance, baseline, or preset changed)

- `docs/benchmark-matrix.md` §4.1 — the `bicycle` and `scooter` rows replace
  `narrow_isolated_* (planned)` with the checked-in fixture names.
- `docs/benchmark-matrix.md` — a new §7.2 "Checked-in fixtures" lists the six
  checked-in paths; the four Increment 1 rows are removed from the planned table
  (now §7.3); the fixture-resolution section is renumbered to §7.4 with its two
  cross-references. No cell, quantity, tolerance, baseline, or preset changed.
- `docs/benchmark-matrix.json` — the four `.planned_patterns` keys
  (`narrow_isolated`, `narrow_signal`, `narrow_following`, `narrow_crossing`) move
  to a new `fixtures.checked_in_increment_1` map of the six paths; every cell,
  class, tolerance, and count is unchanged.

## Files changed

New: the six fixtures above, the six tests in
`crates/hekate-sim/tests/narrow_isolated.rs`, and this node's move to
`resolved/`. Modified: `docs/benchmark-matrix.md`,
`docs/benchmark-matrix.json`, and
`apps/hekate-cli/tests/migration_regression.rs` (needed, see below). No
production module in `hekate-sim`, `hekate-model`, `hekate-present`, or
`apps/hekate-cli/src` changed; no `baselines/**`, `schemas/**`, golden trace,
Phase 1 fixture, or other node file changed.

### Required CLI test-gate change

The Phase 2 Increment 0 gate
`migration_regression::every_scenario_migrates_and_preserves_the_original_trace_body`
walked every `*.json5` under `scenarios/` and required each to be a version-1
source that migrates, so the first version-2 fixture under `scenarios/` failed
it. The gate now parses each scenario and, for a version-2 document, validates
and compiles it directly instead of migrating; version-1 documents keep the
full byte-for-byte differential against the original reader. The renamed test
`every_scenario_migrates_or_is_a_valid_version_2_document` still enumerates
every checked-in scenario, so nothing escapes the gate.

## Acceptance

- `cargo test -p hekate-sim` — passes (147 lib + all suites, including the six
  new `narrow_isolated` tests and the existing `narrow_longitudinal`,
  `narrow_spawn`, and `narrow_mode_no_branch`).
- `cargo test --workspace` — passes; Phase 1 golden traces unchanged
  (`baselines/**`, `tests/golden/**`, and the pinned migrated fixtures are
  untouched).
- `cargo fmt --all -- --check` and `RUSTFLAGS="-D warnings" cargo clippy -p
  hekate-sim -p hekate-cli --all-targets --all-features` — clean.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- CLI, each fixture: `cargo run -q -p hekate-cli -- validate
  scenarios/phase2/inc1/<fixture>.json5` reports the document valid, and
  `cargo run -q -p hekate-cli -- run scenarios/phase2/inc1/<fixture>.json5
  --seed 20260913 --ticks 2400` exits 0 with a trace hash for all six.
- `tangle check` — `graph check: passed (120 nodes)` while this node was
  still in `proposed/`.

## Note for the coordinator

`narrow_crossing_v2` models no narrow-narrow conflict resolution — Increment 1
has none — so a simultaneous arrival of the two modes at the shared conflict
region is recorded as a contact rather than resolved. Its test therefore asserts
the crossing geometry, route completion, and the narrow bounds, not pairwise
separation; the `CC-CROSS` separation evidence is the Increment 5 pairwise rung.
Also note the authored `demand[].spawn.rate.interval_s` window is parsed and
validated but not simulated (the kernel draws arrivals at `rate_per_hour` from
t=0), so no fixture can rely on a demand time window; the fixtures keep each mode
alone on its own facility through low rates instead.

Post-move note: after this file moves to `resolved/`, `tangle check`
transiently reports `next-resolved-node` because TAS-019's `next` still names
this node; the coordinator advances TAS-019's `next`. No parent or sibling node
file was edited and no child node was created.
