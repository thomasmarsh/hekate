---
context_rev: 1
status: proposed
updated: 2026-09-15T17:19:50Z
summary: Classify default-suite test cost and design the slow harness and budget.
---

Area [[IDX-001-hekate]].

# Fast-suite / harness split — decisions and targets

Owner policy: the default `cargo nextest run --workspace` suite is quick,
deterministic pass/fail only. Expensive tests are not tests; they are
`#[ignore = "slow: ..."]` and run from a separate harness that CI runs.

## Mechanism (decided)

- Selection: reuse the repo's `#[ignore = "slow: ..."]` +
  `cargo nextest run --run-ignored ignored-only` pattern.
- `.config/nextest.toml`:
  - `[profile.default]` and `[profile.ci]` get
    `slow-timeout = { period = "5s", terminate-after = 4 }`
    (warn at 5 s, hard-fail at 20 s). No `default-filter` that drops tests.
  - new `[profile.harness]` with no budget, for the harness runs.
- New `docs/test-policy.md`: the budget, what belongs in the default suite, the
  harness command, and how to add a new expensive test.
- New `scripts/run-test-harness.sh`: runs the ignored tests
  (`cargo nextest run --profile harness --run-ignored ignored-only
  --workspace --all-features`), version-checks nextest, prints the version.
- New `scripts/check-harness-inventory.sh`: fails if a `#[ignore]` is not a
  declared harness target (prevents an expensive test being ignored and never
  run).
- CI: a `harness` job runs `scripts/run-test-harness.sh` on push/PR so moved
  coverage is not lost.
- `scripts/bench-release.sh`: pass `--profile harness` so the new budget does not
  kill the 506 s release benchmark.
- Pin the documented nextest install to 0.9.144 (matches CI).

## Targets to move to the harness (GATE/MEASUREMENT, ~230 s CPU)

- `crates/hekate-sim/tests/mixed_interaction.rs` the 24x4000-seed sweep.
- `apps/hekate-cli/tests/inc2_trace.rs` the 12 golden-run cases (100.9 s).
- `apps/hekate-cli/tests/inc2_determinism.rs` the seed-bank batch sweep.
- `apps/hekate-cli/tests/narrow_determinism.rs` the bank batch and
  every-preset cases.
- `apps/hekate-cli/tests/migration_regression.rs` every-scenario and frozen
  goldens.
- `crates/hekate-sim/tests/vehicle_yielding.rs` the 4000-tick sweep trio.
- `apps/hekate-cli/tests/scenarios.rs` `car_following_benchmark`.
- `crates/hekate-sim/tests/metrics.rs` the PET convergence measurement.
- `apps/hekate-cli/tests/experiment_spec.rs` the paired experiment.
- `apps/hekate-cli/tests/increment6_trace.rs`, `safety_events.rs` mixed stream,
  `pedestrian_*` benchmark where over budget.

The exact per-test set and the burn-down of still-over-budget behavior tests
(as SLOW warnings, then fastening) are derived from the classification report.

## Deferred to the fasten child (BEHAVIOR, ~200 s recoverable)

- `lane_transitions` constraint family -> a crate-internal two-band scripted
  fixture in `crates/hekate-sim/src/sim.rs` next to `push_rider`
  (`src/sim.rs:6400`), driving the same rules in <1 s.
- `motor_overtaking`, `narrow_passing` -> scripted 10-12 m approach driven only
  until the pass closes.
- `close_pass` merged run, `run_metrics` close-pass trio -> stop at the first
  closed observation instead of fixed 900 ticks.
- `inc2_determinism` other-seed canaries -> compare one run against its
  checked-in golden hash instead of re-running.
- `scenarios.rs:143` -> 300-600 ticks.

## Quick removals (dominated determinism, ~30 s, named owners)

- `close_pass.rs:314`, `narrow_passing.rs:410`, `inc2_determinism.rs:100`,
  `signal_compliance.rs:214`, `longitudinal_control.rs:221` — each a run-to-run
  determinism check dominated by the golden/hash suites or `sim.rs` unit
  determinism tests.

## Comment corrections (from the redundancy review)

- Incorrect "stream pin" claim in the `inc2` batch tests; overstated golden
  coverage in `inc2_fixture_overlays`; the dropped spawn-count equality note.
