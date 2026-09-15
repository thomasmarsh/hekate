---
status: proposed
context_rev: 1
priority: P3
updated: 2026-09-15T14:20:37Z
summary: Cut the Rust dev-loop and workspace test wall time
next: "[[tas-6t9972esz0d36es0cm7mzffdkf-make-cargo-nextest-the-worker-loop-and]]"
---

Area [[IDX-001-hekate]].

# Outcome

The per-edit and per-gate wall time for the Rust workspace is reduced so a fresh
worker can run a focused acceptance gate in seconds rather than tens of seconds,
and the coordinator workspace gate fits in a small fraction of a session.

# Done when

- The worker-loop fast path (touched crate and test binary) runs in a few
  seconds, measured before and after.
- `cargo test --workspace` wall time is materially reduced or parallelized
  (for example with cargo-nextest), with no test weakened or removed.
- The 25 GB `target/` directory is pruned and a documented housekeeping recipe
  exists.

# Context

Measured 2026-09-14 during TAS-020: incremental compile+link of the touched test
binary ~2s; one touched integration binary (`lane_transitions`, tick-loop tests)
34-37s; `cargo test --workspace` 148s over 83 test binaries; `target/` 25 GB.
Three of three Increment 2 workers hit the 30-minute harness timeout, largely on
repeated gates. Repo tooling, independent of the Phase 2 increment work.
