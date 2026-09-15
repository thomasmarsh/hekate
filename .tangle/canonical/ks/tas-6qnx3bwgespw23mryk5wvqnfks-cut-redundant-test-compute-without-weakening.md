---
context_rev: 1
priority: P2
status: resolved
updated: 2026-09-15T17:04:48Z
summary: Cut redundant test compute without weakening assertions.
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

# Result

Deletion and merging (no new parallelism) cut the workspace gate's CPU spend by
more than the earlier parallelization work: `user` CPU 619.79 -> 440.52 s
(-179.27 s, -28.9 %) and test count 1118 -> 1086 (-32), warm wall about 157 s.
Every load-bearing assertion still runs; each removal names the cheaper owner.

Slice 1, hekate-cli (commit `c009763`): `user` 281.54 -> 143.14 s (-49.2 %),
workspace `user` 619.79 -> 485.34 s (-21.7 %), -29 tests. Removed the 12
`*_admits_its_maneuver_at_*` cases (owned by `inc2_trace`'s `declared_run`), 11 of
12 `*_reproduces_its_trace_hash_at_*` cases (owned by the stronger golden; kept
one live-runs canary), the 3 `*_standard_run_is_the_canonical_cli_trace` cases
(transitive), the `right` batch run in both seed-bank tests (owned by
`assert_batch_hash_is_canonical` and the run-directory stream pin), and
`the_inc2_seed_bank_batch_runs_every_fixture`; merged the two
`increment6_trace` tests; dropped the present two-projection test. Kept the six
`*_changes_its_trace_for_a_seed_outside_the_bank` cases and the bank-declaration
test.

Slice 2, hekate-sim (commit `727f580`): `user` 341.81 -> 286.55 s (-16.2 %),
workspace `user` 485.34 -> 440.52 s (-9.2 %), -3 tests. Merged the four
`close_pass` 900-tick runs into one
`a_passing_run_reports_its_bands_events_and_run_end_closure` (all assertion
families kept as helpers plus the still-open prefix run); removed the
byte-identical `lane_transitions` control run; removed the duplicated
`narrow_passing` forward case and strengthened the remaining reverse test with a
`PassSide::Left` assertion; removed `motor_overtaking`'s third run (owned by the
cited `sim.rs` unit tests); reduced `performance_counters` `FOCUSED_TICKS`
2000 -> 1000.

Not cut (declared evidence or sole guard): the 24x4000 `mixed_interaction`
sweep, the PET convergence test, the TAS-114 constraint behaviours, the
seed-varies cases, the inc2 goldens, the CLI end-to-end `run`/`replay` test, and
the `wrong_way` permutation test.

Recorded residuals (P2, not silently accepted): the batch-test name and comment
still advertise an event-stream comparison they no longer make, whose real owner
is `apps/hekate-cli/tests/batch.rs` and `run_directory.rs`; the
`inc2_fixture_overlays` comment overstates the golden (it pins landmark frames
plus run-end, not every frame or the `target_offset`/`predicted_gap` streams);
and the removed trace-vs-observation spawn-count equality is not otherwise pinned
though its substance is guarded by the byte-exact golden and maneuver
membership. Recorded judgment calls (need a measurement or ledger decision): the
`lane_transitions` horizon 6.0 -> 4.0 and 600 -> 300-tick drives (about -30 to
-45 s, measurement-gated, TAS-114-named evidence); the `run_metrics` close-pass
trio (share a run, about -12 to -20 s); the `migration_regression` differential
horizon; the `scenarios.rs` seed sweeps; the `narrow_determinism` second run; and
a two-band scripted unit fixture that could replace the TAS-114 integration
family (up to about -56 s, a ledger decision).

Evidence: commits `c009763`, `727f580`; `/tmp/tas137/after-r1.md`,
`/tmp/tas137/after-r2.md`.
