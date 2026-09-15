---
context_rev: 1
status: resolved
updated: 2026-09-15T18:03:41Z
summary: Remove the dominated determinism tests and correct the overstated comments.
---

Parent [[tas-2hx2hbxy3gywdr5qcd2zfny0ff-keep-the-default-test-suite-fast-with-a]].

# Outcome

The run-to-run determinism tests already dominated by the golden and unit suites
are gone, and the comments no longer claim coverage that was removed.

# Done when

- `crates/hekate-sim/tests/close_pass.rs:314`,
`crates/hekate-sim/tests/narrow_passing.rs:410`,
`crates/hekate-sim/tests/signal_compliance.rs:214`, and
`crates/hekate-sim/tests/longitudinal_control.rs:221` are removed, each with the
owner that keeps its reassurance named (the golden/hash suites or the `sim.rs`
same-seed unit tests). `apps/hekate-cli/tests/inc2_determinism.rs:100` is removed
only if it is not the only live-runs canary for the inc2 driver; if it is the
only one, keep it and say so.
- The batch-test names and comments no longer claim an event-stream comparison
they do not make, and the `inc2_fixture_overlays` comment states the golden's
actual coverage (landmark frames plus run-end, not every frame or the
`target_offset`/`predicted_gap` streams).
- `cargo nextest run --workspace --all-features` passes.

# Context

Informed by [[tho-55ch2x2wgytsh1xew9pbjf5py3-classify-default-suite-test-cost-and-design-the]].

# Result

Committed as `7b3f853`. Default suite 1055 -> 1052 tests, `user` CPU
295.2 -> 270.1 s; harness 31 tests.

Removed, each with the owner named in a comment:

- `close_pass.rs::detection_is_deterministic_across_runs` and
  `narrow_passing.rs::the_pass_decision_is_stable_for_a_seed` were pure run-to-run
  canaries; `sim.rs::same_seed_produces_identical_observations` and the CLI
  golden/hash suites own the property.
- `inc2_determinism.rs::narrow_passing_v2_reproduces_its_trace_hash_at_standard`
  (the live-runs canary) was not the only guard: `inc2_trace.rs::assert_matches_golden`
  runs the same driver at every fixture+preset and pins it to the checked-in
golden.

Narrowed instead of removed (supervisor decision, option b), because the
different-seed half is the sole seed-sensitivity guard for those scenarios:

- `signal_compliance.rs::decision_records_change_with_the_seed` (kept the
  `assert_ne!` divergence, dropped the same-seed equality).
- `longitudinal_control.rs::control_and_signal_state_change_with_the_seed` (kept
  the `assert_ne!` divergence, dropped the same-seed equality).

Comments corrected: the `inc2`/`narrow` batch names no longer claim an
event-stream comparison (the stream equality is owned by `batch.rs` and
`run_directory.rs`); the `inc2_fixture_overlays` comment states the golden's
actual coverage (landmark frames plus run-end, not every frame or the
`target_offset`/`predicted_gap` streams); and the removed
`the_inc2_seed_bank_batch_runs_every_fixture` spawn-count equality is named as
not otherwise pinned. `scripts/check-harness-inventory.sh` was updated for the
renamed ignored targets. Evidence: commit `7b3f853`; `/tmp/tas137/after-h2.md`.
