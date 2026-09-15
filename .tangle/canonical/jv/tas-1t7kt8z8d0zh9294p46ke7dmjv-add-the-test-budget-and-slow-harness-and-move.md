---
context_rev: 1
status: resolved
updated: 2026-09-15T17:51:32Z
summary: Add the test budget and slow harness and move the measurement gates.
---

Parent [[tas-2hx2hbxy3gywdr5qcd2zfny0ff-keep-the-default-test-suite-fast-with-a]].

# Outcome

The default profile carries a cost budget, and the expensive measurement and gate
tests are selected only by the slow harness that CI runs.

# Done when

- `docs/test-policy.md` records the budget, the default-versus-harness split, and
the exact harness command.
- `.config/nextest.toml` has a `[profile.harness]` with no budget, a
`slow-timeout` budget on `[profile.default]` and `[profile.ci]`, and no
`default-filter` that drops tests.
- The measurement and gate tests are `#[ignore = "slow: ..."]`;
`scripts/run-test-harness.sh` runs them and a CI job runs the harness.
- `scripts/check-harness-inventory.sh` fails when an `#[ignore]` is not a declared
harness target.
- `scripts/bench-release.sh` passes `--profile harness`, and the documented
nextest install is pinned to 0.9.144.
- `cargo nextest run --workspace --all-features` still passes and selects no
harness test.

# Context

Informed by [[tho-55ch2x2wgytsh1xew9pbjf5py3-classify-default-suite-test-cost-and-design-the]].

# Result

The default suite now carries a cost budget and the expensive measurement/gate
tests run only from a CI-invoked slow harness. Committed as `124387e`.

- `.config/nextest.toml`: `[profile.default]` and `[profile.ci]` carry
  `slow-timeout = { period = "20s", terminate-after = 3 }`; `[profile.harness]`
  has no budget. No `default-filter` anywhere.
- `docs/test-policy.md` records the budget, the split, and how to add an
  expensive test; `docs/dev-loop.md` pins nextest to 0.9.144 and points at it.
- `scripts/run-test-harness.sh` runs the moved tests, excludes the three
  release-mode wall-clock binaries from its debug run, and runs them under
  `--release` only via `--release-benchmarks`; `scripts/check-harness-inventory.sh`
  fails on an undeclared ignore or a stale declaration. CI has a `harness` job.
- 31 tests moved to `#[ignore = "slow: ..."]`: the `mixed_interaction` gate, the
  `vehicle_yielding` sweep, the PET convergence measurement, the safety-events
  mixed stream, the 12 `inc2_trace` goldens plus its CLI run/replay test, the
  `inc2_determinism` seed-bank batch, the `narrow_determinism` batch and
  every-preset cases, the `migration_regression` scenarios and frozen goldens,
  `car_following_benchmark`, the paired experiment, and `increment6_trace`.
- Measured: default suite 1086 -> 1055 tests, `user` CPU 440.5 -> 295.2 s
  (-33 %), wall 153 -> 112 s; harness 31 tests / 97.4 s; 4 SLOW >20 s, all under
  the 60 s cap. No assertion deleted.

Deviation from the recorded plan: the plan said 5 s SLOW / 20 s hard fail, but
the shipped budget is 20 s SLOW / 60 s hard fail, because the `lane_transitions`
behavior tests (about 23-31 s) must keep passing until the fasten child makes
them sub-second; they appear as SLOW now. Evidence: commit `124387e`;
`/tmp/tas137/after-h1.md`.
