# Test policy: the default suite budget and the slow harness

`cargo nextest run --workspace` is the loop every change pays for, so the
default suite is quick, deterministic pass/fail only. Expensive work — wall-clock
benchmarks, measurement sweeps, and long-horizon gates — is not a test; it is an
`#[ignore = "slow: ..."]` case that the separate slow harness runs. This document
records the budget, what belongs where, the harness command, and how to add a
test. `.config/nextest.toml` holds the budget and `scripts/run-test-harness.sh`
runs the harness.

## The default-suite budget

Every `[profile.default]` and `[profile.ci]` test carries a nextest
`slow-timeout`:

```toml
slow-timeout = { period = "20s", terminate-after = 3 }
```

A default-profile test that runs past **20 s** is reported `SLOW`; one that runs
past three periods (**60 s**) is terminated and fails the run. The budget is a
guard, not a target: a test that only stays under it because CI is fast is a test
that should be made fast or moved, not one that should be tolerated at 59 s.

`SLOW` is a warning to act on, not a pass. When a default-profile test goes
`SLOW`, make it fast, move it to the harness, or delete it as redundant — with
the decision recorded. A `SLOW` line in the workspace gate is a bug report.

The default profile defines no `default-filter`: no test is dropped by a filter,
so "not selected" always means "ignored", never "filtered out silently". The
`harness` profile (below) has the same property.

## What belongs in the default suite

The default suite holds quick, deterministic behavior tests:

- unit tests, and integration tests that drive a small fixture in well under a
  second;
- pass/fail behavior checks whose cost is the fixture, not a measurement;
- golden checks that compare a cheap artifact (a hash, a byte body) rather than a
  whole long run.

The slow harness holds everything that is expensive or non-deterministic in cost:

- wall-clock benchmarks and frame-time measurements;
- multi-seed sweeps and long-horizon (thousands-of-ticks) gate runs;
- whole-trace golden reproductions and batch runs over a declared seed bank.

Two kinds of case are out of scope for both suites: a test that is already covered
by another suite (delete it, and name the suite that keeps the reassurance), and
a behavior test whose *fixture* is too long (shorten the fixture so the same
assertion runs quickly; then it is a default-suite test again).

## The harness command

Run the slow harness from the repository root:

```sh
scripts/run-test-harness.sh
```

It checks the installed `cargo-nextest` (pinned to 0.9.144, the version CI
installs) and warns on any other version, then runs every `#[ignore]` test in the
default build under the budget-free `harness` profile:

```sh
cargo nextest run --profile harness --workspace --all-features \
  --run-ignored ignored-only \
  -E 'not (binary(release_benchmark) | binary(presenter_frame_time) | binary(performance_counters))'
```

`scripts/check-harness-inventory.sh` fails when an `#[ignore]` test is not a
declared harness target, so a test cannot be ignored and never run.

The three release-mode wall-clock harnesses (`release_benchmark`,
`presenter_frame_time`, `performance_counters`) are excluded from that run
because `release_benchmark` panics under `debug_assertions` and all three measure
release-mode wall clock. Run them with:

```sh
scripts/run-test-harness.sh --release-benchmarks
```

That adds a `--release` invocation selecting exactly those three binaries.
`release_benchmark` rewrites `perf/release-bench.json`, so CI runs only the
default (debug) form; refresh the artifact locally or on a schedule.
`scripts/bench-release.sh` remains the single-purpose entry point for that one
benchmark and passes `--profile harness` for the same reason.

The harness profile sets no budget: its `slow-timeout` is nextest's documented
"infinite" sentinel (`period = "30y"`, the value `bench.slow-timeout` defaults to),
which also keeps the default profile's 20 s budget from inheriting into a
legitimate multi-minute run.

## Adding a new expensive test

1. Write the test as usual, but mark it with the reason it is expensive:

   ```rust
   #[test]
   #[ignore = "slow: <what makes it expensive>; run scripts/run-test-harness.sh"]
   fn the_expensive_gate_holds() { /* … */ }
   ```

   Use `slow: ...` for a default-build case. A release-mode wall-clock
   measurement uses the existing `release-mode wall-clock measurement; ...` form
   instead, and must panic or be meaningless under `debug_assertions`.

2. Declare it in `scripts/check-harness-inventory.sh`. Add
   `"<path> <function-name>"` to `slow_targets` (default build) or
   `release_targets` (release mode). For an `#[ignore]` inside a
   `macro_rules!` body that generates the test names, declare
   `"<path> macro:<macro-name>"` instead.

3. Check the inventory and run the harness:

   ```sh
   scripts/check-harness-inventory.sh
   scripts/run-test-harness.sh
   ```

4. Confirm the default suite still selects it:

   ```sh
   cargo nextest list --workspace --all-features | grep <test-name>   # no match
   ```

An `#[ignore]` with no declaration fails `scripts/check-harness-inventory.sh`,
which CI runs in its `harness` job; a stale declaration for a test that no longer
carries `#[ignore]` fails it too.

## Where the harness runs in CI

`.github/workflows/ci.yml` has a `harness` job on every push and pull request. It
installs the pinned nextest, runs `scripts/check-harness-inventory.sh`, then runs
`scripts/run-test-harness.sh` — the debug slow tests only. The release-mode
benchmarks are deliberately left out so a pull request never rewrites
`perf/release-bench.json`.

## Related documents

- `docs/dev-loop.md` — the per-edit loop, the fast paths, and the pinned nextest
  install.
- `.config/nextest.toml` — the profiles and the budget.
- `scripts/run-test-harness.sh`, `scripts/check-harness-inventory.sh`,
  `scripts/bench-release.sh` — the harness entry points.
