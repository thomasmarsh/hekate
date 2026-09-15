---
context_rev: 1
status: resolved
updated: 2026-09-15T15:53:47Z
summary: Cut the workspace gate by parallelizing long serial tests and trimming redundant work.
---

Parent [[TAS-137-cut-the-rust-dev-loop-and-workspace-test-wall]].

# Outcome

The post-nextest workspace gate spends its wall time in a few monolithic serial
loops and a little redundant in-test work. Each long loop becomes independent
per-case tests that nextest parallelizes, redundant work is removed, and every
existing assertion still runs.

# Done when

- Each audited candidate has a recorded decision (keep, trim, parallelize,
remove) with the evidence that justifies it.
- No monolithic serial loop dominates the gate and total suite CPU is reduced;
the 4-core gate is at its parallel floor, so a test-only refactor cannot lower
wall much further, with every formerly-monolithic test's assertions preserved,
possibly redistributed across per-case tests.
- No test is removed, weakened, or newly ignored.
- `cargo nextest run --workspace --all-features` passes, and before/after wall
time and test counts are recorded.

# Context

The Increment 2 audit found no non-load-bearing tests, no network or unbounded
randomness, and bounded fixed-seed generators. The cost is a few long serial
loops plus redundant in-test work:

- `every_inc2_fixture_reproduces_its_trace_hash_at_both_required_presets`
(`apps/hekate-cli/tests/inc2_determinism.rs:95`) is 99.3 s of the 188 s test
time: a `for fixture { for preset { run twice } }` loop nextest cannot split.
- `every_inc2_fixture_matches_its_golden_bytes_and_hash_at_both_required_presets`
(`apps/hekate-cli/tests/inc2_trace.rs:149`) is 42.4 s of the same shape.
- `crates/hekate-sim/tests/mixed_interaction.rs` runs a 24-seed x 4000-tick
sweep three times.
- `crates/hekate-sim/tests/lane_transitions.rs` probe helpers take two
`SnapshotDetail::Full` snapshots per tick where only position is needed.
- `crates/hekate-sim/src/sim.rs` `wall_clock_delay_does_not_change_results`
sleeps 30 x 1 ms for no coverage.

Post-nextest baseline (sibling `tas-6t9972esz0d36es0cm7mzffdkf`): 191.62 s over
1055 tests / 91 binaries.

# Result

No test was removed, weakened, or newly ignored; every candidate has a recorded
decision, and both slices are committed with the workspace gate green.

Parallelized (commit `7ca7a78`):

- `every_inc2_fixture_reproduces_its_trace_hash_at_both_required_presets` and the
  seed-bank admission loop now expand to per-fixture+preset `#[test]` cases via
  index-mapped macros (`inc2_determinism.rs` 7 -> 39 tests).
- `inc2_trace.rs` golden and maneuver-coverage monoliths likewise expand
  (4 -> 37 tests).
- Removed the 99.3 s serial pole: `inc2_determinism` binary wall 96.3 -> 54.2 s,
  `inc2_trace` 41.2 -> 18.4 s, longest refactored case 99.3 -> ~27-33 s.
- Compile-time guards (`inc2_support/mod.rs`) fail the build if `FIXTURES` or
  `PRESETS` grows without extending the hand-maintained case lists.

Redundant work removed (commit `42453bd`), measured on the same 4-CPU host with
`cargo nextest run -p hekate-sim --all-features`:

- The three `mixed_interaction` 24-seed sweeps now share one `sweep_reports()`
  pass (72 -> 24 `run_report` calls); the binary drops 108.9 -> 39.0 s.
- `lane_transitions` probes take one `Full` snapshot per tick instead of two
  (`placements` reuses the frame) and `drive` takes `Position`; the binary drops
  220.0 -> 139.6 s.
- `wall_clock_delay_does_not_change_results` no longer sleeps 30 x 1 ms; the
  equality assertion is kept. Reviewer note (P2): with no injected delay the name
  over-claims; it now asserts same-seed determinism, and the crate has no
  wall-clock read, so the delay-based check was redundant.
- hekate-sim package `user` CPU 396.87 -> 341.81 s (-13.9 %).

Gate shape: the post-nextest gate is CPU-bound (about 669 s `user` over 4 cores,
background load ~3-9), so its wall is at the parallel floor and is dominated by
wall clock noise; the 4-core wall did not move materially even though total CPU
and the longest tests did. Recorded decisions left alone: the anti-special-case
source guards (`narrow_mode_no_branch`, `synthetic_template_no_branch`,
`tactical_no_special_case`, `presenter_no_special_case`) stay as-is (low runtime
payoff, separate maintainability concern); `the_mixed_benchmark_shares_one_world`
still runs its own seed-3 report (~1.5 s) as a distinct single-run concern.

Evidence: commits `7ca7a78`, `42453bd`; `/tmp/tas137/b2-baseline.md`,
`/tmp/tas137/after-b2.md`, `/tmp/tas137/after-b1.md`.
