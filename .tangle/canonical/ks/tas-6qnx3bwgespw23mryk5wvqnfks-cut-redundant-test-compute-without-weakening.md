---
context_rev: 1
priority: P2
status: proposed
updated: 2026-09-15T16:27:25Z
summary: Cut redundant test compute without weakening assertions.
next: Delete the inc2_determinism cases already covered by the golden suite and drop the duplicate batch runs.
---

Area [[IDX-001-hekate]].

# Outcome

The workspace gate's CPU spend drops by removing tests and runs whose
reassurance is already provided more cheaply elsewhere, with no load-bearing
assertion lost. Parallelism is not counted as a saving.

# Done when

- The high-confidence redundant items are removed or merged, each with the
  cheaper test that already provides its reassurance named in the change.
- Every load-bearing assertion still runs; the only removals are exact
  duplicates or strictly weaker checks.
- `cargo nextest run --workspace --all-features` passes and the hekate-sim and
  hekate-cli package `user` CPU are measured before and after.
- Items that need a horizon/seed experiment or a ledger update are recorded as
  separate decisions, not silently cut.

# Context

Two adversarial necessity audits (2026-09-15) ran after TAS-137. Parallelism is
not a saving; the gate is CPU-bound at about 620 s `user` over 4 cores.

High-confidence redundant (the same assertion already runs more cheaply):

- `apps/hekate-cli/tests/inc2_determinism.rs`: the 12
  `*_admits_its_maneuver_at_*` cases duplicate `inc2_trace`'s
  `assert_maneuver_occurs` on the identical `run()`; 11 of 12
  `*_reproduces_its_trace_hash_at_*` cases are subsumed by the stronger golden
  (`inc2_trace` pins bytes against the checked-in artifact plus the hash); the 3
  `*_standard_run_is_the_canonical_cli_trace` cases are transitively implied; the
  `right` batch run is pinned by `assert_batch_hash_is_canonical` on `left`.
  Estimated about 110 s CPU.
- `crates/hekate-sim/tests/close_pass.rs`: four tests each run the same
  `passing_scenario(9.0, 4.0)` seed-7 900-tick run; one shared run serves every
  assertion (about -17 s). `lane_transitions.rs:2249` reruns a control already
  run at `:2192` (about -6 s). `narrow_passing.rs:303` forward iteration
  duplicates `:256` (about -8 s). `motor_overtaking.rs:371` third run is already
  asserted at unit level (about -10 s). `performance_counters.rs` `FOCUSED_TICKS`
  2000 -> about 200 (about -5 s). Estimated about 50 s CPU.

Do not cut (declared evidence or sole guard): the 24x4000 `mixed_interaction`
sweep and the PET convergence test (declared in `canonical/28` and
`canonical/30`); the TAS-114 constraint behaviours (only their duplication and
horizon may change); the `*_changes_its_trace_for_a_seed_outside_the_bank` cases;
the inc2 golden cases; the CLI end-to-end `run`/`replay` test; the `wrong_way`
permutation test.

Judgment calls to record, not silently cut: the `lane_transitions` horizon
6.0 -> 4.0 and 600 -> 300-tick drives (measurement-gated, TAS-114-named
evidence); the `run_metrics` close-pass trio (share a run); the
`migration_regression` differential horizon; the `scenarios.rs` seed sweeps; the
`narrow_determinism` second run; and a two-band scripted unit fixture that could
replace the TAS-114 integration family.
