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

That is the full gate: 1118 non-ignored tests across the workspace's test
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

## Target housekeeping

A long-lived checkout accumulates regenerable build output under `target/`.
Reclaim it with

```sh
scripts/clean-target.sh --dry-run   # list exactly what would be removed
scripts/clean-target.sh             # remove it and report the reclaimed space
```

The script prints `du -sh target` before and after plus the reclaimed delta, and
removes three kinds of pure build output that cargo rebuilds on demand:

- `target/**/tangle*` and `target/**/libtangle*` — fingerprints, rlibs, and
  test binaries from before the workspace crates were renamed from `tangle-*`
  to `hekate-*`. No current manifest names a `tangle-*` crate, so nothing reads
  them again. Both the bare and the `lib`-prefixed artifact names are matched.
- `target/debug/incremental/` — the per-hash incremental cache.
- `target/doc/` — the rustdoc output.

On the 2026-09-15 tree (35 GB) the measured reclaim is about 17 GiB, reaching
18 GB, spread over the three classes above: the pre-rename `tangle*`/`libtangle*`
artifacts, the per-hash incremental cache, and the rustdoc output. The rules
overlap by design, so an artifact both classes match is removed by whichever
runs first, never twice. The script only ever descends from the target directory
(`$CARGO_TARGET_DIR` when set, `target/` otherwise), so source, checked-in
goldens, scenarios, schemas, and `Cargo.lock` are never candidates, and
re-running it is a no-op once the tree is clean. It relies on `find`'s default
`-P`, so a symlinked target directory is not followed.

The old crate names cannot creep back either: `scripts/check-no-legacy-tangle.sh`
fails on any tracked file that names a pre-rename `tangle-*` crate, a
`tangle_*` identifier, or a `libtangle*` artifact (CI runs it next to the
dependency-direction check), and `scripts/check-no-legacy-tangle.sh --self-test`
proves it rejects a planted token.

What it costs: deleting the `tangle*`/`libtangle*` artifacts is free, because no
`hekate-*` fingerprint references them, so the next build reuses every
`hekate-*` artifact it keeps and neither invalidates nor relinks them. Deleting
`target/debug/incremental` is free until sources change: cargo consumes the
per-crate cache only when it recompiles a crate, and the measured post-prune
runs recompiled nothing (`Finished` in 0.25 s and 0.68 s, no `Compiling`
lines). The cost appears only once a source change forces a recompile, which
then runs without the per-crate cache; the cache refills as it goes.

Cargo never collects superseded hashed test binaries, so `target/debug/deps`
also holds several stale `hekate_viewer-*` binaries of about 390 MB each. They
are safe to delete by hand, keeping the one for the revision you are debugging,
but picking them out is a judgment call, so the script leaves them alone.
