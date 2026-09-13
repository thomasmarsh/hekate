---
context_rev: 1
updated: 2026-09-13T18:29:25Z
summary: Check in narrow-mode isolated fixtures and tests and finalize their benchmark-matrix paths.
next: Check in the narrow_isolated_* and narrow following, signal, and crossing fixtures and wire their tests.
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
- Integration tests in `crates/tangle-sim/tests/` run straight, curve, braking,
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
`crates/tangle-sim/tests/`, and the fixture paths in `docs/benchmark-matrix.md`.
