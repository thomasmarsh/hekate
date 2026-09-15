# Rust dev loop

`cargo nextest` runs the workspace test binaries, in the worker loop and in CI.
nextest runs one process per test, so it parallelizes without cargo's
shared-process `--test-threads` and no test can leak state into another. The
configuration is `.config/nextest.toml`.

Install it once with `cargo install cargo-nextest --locked` (CI does the same
with `taiki-e/install-action`); without it every command below fails with
`error: no such command: nextest`.

nextest does not run doctests, so they keep the `cargo test --doc` path below.

## Fast path: one crate

```sh
cargo nextest run -p hekate-sim
```

Point it at the crate you touched; nextest rebuilds and links only that crate's
test binaries.

## One test binary

```sh
cargo nextest run -E 'binary(inc2_determinism)'
```

`-E` takes a nextest filterset (`binary(...)`, `package(...)`, `test(...)`; see
<https://nexte.st/docs/filtersets>). Add `-p <crate>` when a binary name is not
unique across packages. `cargo nextest list` runs the same selection without
running anything.

## Workspace gate

```sh
cargo nextest run --workspace --all-features
```

That is the full gate: 1055 non-ignored tests across the workspace's test
binaries, the same selection `cargo test --workspace --all-features` makes over
test binaries.

## Doctests

```sh
cargo test --doc --workspace --all-features
```

`crates/hekate-sim/src/lib.rs` carries the workspace's one compiled doctest, so
run this after the workspace gate. CI runs it as its own step.

## Ignored harnesses

The workspace keeps three `#[ignore]`d harnesses out of the normal run. All three
are release-mode wall-clock harnesses, and each test function's name differs from
the name of the test binary that contains it:
`release_benchmark_writes_the_checked_in_artifact` in `hekate-cli`'s
`release_benchmark`, `increment_2_profile_frame_time` in `hekate-present`'s
`presenter_frame_time`, and `increment_2_profile_counters` in `hekate-sim`'s
`performance_counters`. Run one explicitly with nextest's ignored-only switch:

```sh
cargo nextest run --release --run-ignored ignored-only -p hekate-present --test presenter_frame_time --no-capture
cargo nextest run --release --run-ignored ignored-only -p hekate-sim --test performance_counters --no-capture
cargo nextest run --release --run-ignored ignored-only -p hekate-cli --test release_benchmark --no-capture
```

`--no-capture` passes the harness's output through, and nextest runs that one
test serially. `scripts/bench-release.sh` wraps the release-mode benchmark.

## CI profile

`cargo nextest run --profile ci …` (or `NEXTEST_PROFILE=ci`) is nextest's
recommended CI profile: it runs every test regardless of failures instead of
stopping at the first one. It changes nothing about which tests run.

## Golden fixtures

`scripts/regen-goldens.sh` exports `UPDATE_GOLDENS=1`; nextest passes the
environment through to every test process, so the suites rewrite their fixtures
unchanged. Only that script writes goldens, and each suite owns distinct files,
so no nextest test group serializes them.
