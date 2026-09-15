---
context_rev: 1
status: active
updated: 2026-09-15T14:55:14Z
summary: Cut the workspace gate by parallelizing long serial tests and trimming redundant work.
next: Split the monolithic Increment 2 determinism and golden loops into per-fixture/preset tests so nextest parallelizes them.
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
- The workspace gate wall time is materially below the post-nextest 191.62 s
baseline, with every formerly-monolithic test's assertions preserved, possibly
redistributed across per-case tests.
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
