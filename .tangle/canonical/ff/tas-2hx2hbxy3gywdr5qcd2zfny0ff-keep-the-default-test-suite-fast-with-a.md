---
context_rev: 1
priority: P1
status: proposed
updated: 2026-09-15T17:51:56Z
summary: Keep the default test suite fast with a separate harness for expensive tests.
next: "[[tas-5mdwpgdngga2whqe4h4fckydxg-remove-the-dominated-determinism-tests-and]]"
---

Area [[IDX-001-hekate]].

# Outcome

The default `cargo nextest run --workspace` suite stays fast: every
default-profile test is a quick, deterministic pass/fail check, and expensive
benchmarks, measurements, and long-horizon gates live in a separate harness that
never runs in the per-edit loop. A budget guard fails if a default-profile test
grows too expensive, so the suite cannot drift back.

# Done when

- A checked-in test policy records the cost budget, what belongs in the default
  suite, and how to run the separate harness.
- Every default-profile test over the budget has a recorded decision: made
  fast/narrow, moved to the harness, or removed as redundant.
- The separate harness runs the expensive tests via a documented command, and CI
  runs it so coverage is not lost.
- A guard fails when a default-profile test exceeds the budget.
- Every load-bearing assertion still runs somewhere; nothing is deleted without a
  named owner.
- The overstated comments from the redundant-test-compute review are corrected.

# Context

Owner policy (2026-09-15): unit tests are supposed to be quick; if a test is
expensive it is not a test and belongs in a separate harness, so the default
suite never hurts iteration speed. The workspace gate was cut from 619.8 s to
440.5 s `user` CPU by removing redundant compute, but it still contains tests
that are benchmarks, measurements, or long-horizon gates by cost.

Known expensive default-profile tests (per-test seconds after the redundancy
cut): the `mixed_interaction` 24x4000-seed safety sweep (~36 s); the
`lane_transitions` constraint family (~140 s, horizon 6.0 and 600-6000-tick
drives); the merged `close_pass` run (~11 s); the `run_metrics` close-pass trio
(~26 s CPU); `narrow_passing`, `motor_overtaking`, `wrong_way`, and
`car_following_benchmark`; and the `inc2_determinism`/`inc2_trace` golden suites.
Already out of the default run: `performance_counters`, `presenter_frame_time`,
and `release_benchmark` (`#[ignore]`, release-mode wall-clock).

`lane_transitions` is a pass/fail behavior test, not a benchmark; its cost is the
6.0 s maneuver-prediction horizon and long drives. `run_metrics` is pass/fail
metric-artifact behavior with redundant full-simulation runs. A two-band scripted
unit fixture is a small in-process fixture that scripts two adjacent bands and
pushed riders to exercise the constraint rule deterministically in about a second
instead of the 24-32 s integration drives.

Prior audits: `tas-6qnx3bwgespw23mryk5wvqnfks` and its evidence in
`/tmp/tas137/audit2-sim.md`, `/tmp/tas137/audit2-cli.md`.
