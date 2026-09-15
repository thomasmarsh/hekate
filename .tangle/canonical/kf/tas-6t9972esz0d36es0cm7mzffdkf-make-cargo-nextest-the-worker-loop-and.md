---
context_rev: 1
status: resolved
updated: 2026-09-15T14:54:24Z
summary: Make cargo-nextest the worker-loop and workspace test runner.
---

Parent [[TAS-137-cut-the-rust-dev-loop-and-workspace-test-wall]].

# Outcome

`cargo nextest` is the workspace test runner: a worker runs one touched crate
or test binary in seconds, and the full workspace gate runs test binaries in
parallel without changing what any test asserts.

# Done when

- A checked-in nextest configuration (`.config/nextest.toml`) defines the
default worker profile and any CI profile, with no filter that drops tests.
- The documented fast path (`cargo nextest run -p <crate>` / an `-E` binary
selector or equivalent) is recorded in the repo's dev-loop docs with exact
commands.
- Before/after wall times are recorded for the touched-binary loop and the
workspace gate against the TAS-137 baseline (~2 s touched binary, 148 s
`cargo test --workspace` over 83 binaries).
- No test is removed, weakened, or newly ignored; doctests, which nextest does
not run, keep a documented `cargo test --doc` path.
- CI uses nextest, or the deferral is recorded with its reason.

# Context

TAS-137 baseline: incremental compile+link of the touched test binary ~2 s;
`cargo test --workspace` 148 s over 83 test binaries; `cargo nextest` is not
installed (`error: no such command: nextest`). CI runs
`cargo test --workspace --all-features` under `RUSTFLAGS: -D warnings`.

# Result

Adopted cargo-nextest 0.9.144 as the worker-loop and workspace test runner:

- `.config/nextest.toml` has an empty `[profile.default]` (no filter, no
  `test-threads` override, no test group) and `[profile.ci]` with
  `fail-fast = false`; CI selects it with `--profile ci`.
- CI installs `nextest@0.9.144` via `taiki-e/install-action@v2` and runs
  `cargo nextest run --profile ci --workspace --all-features`, plus a separate
  `cargo test --doc --workspace --all-features` step for the one compiled
  doctest.
- `docs/dev-loop.md` (linked from README) records the install step, the
  touched-crate fast path, the `-E 'binary(...)'` selector, the workspace gate,
  the doctest path, and the three `--release --run-ignored ignored-only
  --no-capture` ignored harnesses.
- Measured on this host (4 CPUs), same test source: workspace gate 336.48 s ->
  191.62 s (1.76x) over the identical 1055-test / 91-binary selection with the
  3 ignored harnesses skipped; warm `-p hekate-model` 3.89 s; single binary
  `-E 'binary(claims)'` 1.49 s; doctests 3.32 s. A later re-run under agent CPU
  contention measured 248.6 s, so 191.62 s is the uncontended figure and the
  ~99 s serial `inc2_determinism` test is the remaining gate floor.

Evidence: commits `b53f9bb` and `ddfbd80`; measurement reports
`/tmp/tas137/baseline.md`, `/tmp/tas137/after.md`, `/tmp/tas137/after-fix.md`.
