---
status: resolved
context_rev: 1
priority: P3
updated: 2026-09-15T16:10:58Z
summary: Cut the Rust dev-loop and workspace test wall time
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

# Result

The Rust dev-loop and workspace gate are both parallelized and trimmed:

- cargo-nextest 0.9.144 is the runner; the workspace gate went from 336.5 s
  (`cargo test --workspace --all-features`, 1055 tests) to 191.6 s over the same
  test selection, plus a separate doctest step. Warm touched-crate loop 3.9 s,
  single test binary 1.5 s.
- The 99.3 s serial `inc2_determinism` pole was split into per-case tests
  (longest about 27-33 s), and redundant hekate-sim work was removed (package
  user CPU 396.9 -> 341.8 s; `mixed_interaction` 108.9 -> 39.0 s,
  `lane_transitions` 220.0 -> 139.6 s). The gate is CPU-bound at about 620 s user
  on 4 cores, so wall is at its parallel floor and is dominated by host load.
- `target/` went from 35 GB to 17 GB with a documented `scripts/clean-target.sh`
  recipe.

Children `tas-6t9972esz0d36es0cm7mzffdkf` (nextest),
`tas-10p2xa87xh5vfetphxmcgp67y7` (test-work pruning), and
`tas-0wfqm53dckb26t1exycssqa2s2` (target prune) are all resolved. Evidence:
commits `b53f9bb`, `ddfbd80`, `771ee2a`, `7ca7a78`, `42453bd`, `bbf4887`,
`b384748`.
