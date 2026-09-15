---
context_rev: 1
status: proposed
updated: 2026-09-15T14:20:37Z
summary: Make cargo-nextest the worker-loop and workspace test runner.
next: Add a checked-in .config/nextest.toml and document the touched-binary fast path, then measure it.
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
