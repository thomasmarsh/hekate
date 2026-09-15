---
context_rev: 1
status: proposed
updated: 2026-09-15T17:20:06Z
summary: Add the test budget and slow harness and move the measurement gates.
next: Add docs/test-policy.md, the harness profile, the budget, scripts/run-test-harness.sh, the inventory guard, and the CI harness job, then move the measurement/gate tests to #[ignore].
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
