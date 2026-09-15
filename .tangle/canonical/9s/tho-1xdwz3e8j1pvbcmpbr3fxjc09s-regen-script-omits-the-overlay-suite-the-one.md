---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:55Z
summary: regen script omits the overlay suite; the one-overtake property holds at seed 0 only.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 fixture and golden regeneration
surface (TAS-132, TAS-133, TAS-136). Verified at commit 98ba8d3.

# Residuals

1. **`scripts/regen-goldens.sh` does not name the overlay suite.** The script
   runs `scene_golden`, `golden_cells`, `golden_kitty`, and `inc2_trace`, but
   not `crates/hekate-present/tests/inc2_fixture_overlays.rs`, whose module docs
   say it must be run with `UPDATE_GOLDENS=1` to rewrite
   `tests/golden/present/inc2_tactical_fixtures.seed0.txt`. A full
   regeneration therefore leaves that golden stale whenever the tactical
   projection changes.
2. **The authored "exactly one overtaking interval" property holds at seed 0
   only.** `crates/hekate-sim/tests/inc2_passing_fixtures.rs` builds every
   fixture with `SEED = 0` (line 82, `build` at line 366) and asserts
   `close_pass_tracker().overtakes().len() == 1` (lines 386-390 and 582-586),
   while the CLI, determinism, and presenter suites pin the declared bank seeds
   11, 48, and 102
   (`apps/hekate-cli/tests/inc2_support/mod.rs:148-204`,
   `scenarios/phase2/inc2/inc2_seed_bank.json`). The one-interval property is
   never asserted at a pinned bank seed, and TAS-132/TAS-133 found it does not
   hold at those seeds, so it is a property of the seed-0 run rather than of
   the authored fixture.

# Consequence

A change to the tactical projection silently invalidates an un-named golden,
and a fixture property a reader would take as authored is really a seed-0
observation. Any future claim that a fixture carries exactly one overtaking
interval must name the seed it was measured at.

# Seam

`scripts/regen-goldens.sh`,
`crates/hekate-present/tests/inc2_fixture_overlays.rs`,
`crates/hekate-sim/tests/inc2_passing_fixtures.rs`,
`apps/hekate-cli/tests/inc2_support/mod.rs`,
`scenarios/phase2/inc2/inc2_seed_bank.json`.
