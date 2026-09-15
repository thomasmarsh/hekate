---
context_rev: 1
status: proposed
updated: 2026-09-15T17:20:06Z
summary: Remove the dominated determinism tests and correct the overstated comments.
next: Remove the dominated run-to-run determinism tests with a named owner and correct the batch-test and overlay comments.
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
