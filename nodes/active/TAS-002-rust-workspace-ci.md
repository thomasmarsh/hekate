---
context_rev: 1
priority: P1
updated: 2026-09-12T02:12:00Z
summary: Create the pinned Rust workspace, lint and test gates, and release-build CI.
next: Verify `cargo test --workspace` and `cargo build --workspace --release` in CI.
---

# Outcome

A formatted, linted, tested Rust workspace that builds in release mode with a
pinned toolchain and pinned dependencies, with the one-way dependency direction
enforced.

# Done when

- Workspace contains `crates/tangle-model`, `crates/tangle-sim`,
  `apps/tangle-cli`, and `apps/tangle-viewer`.
- `rust-toolchain.toml` pins Rust 1.98.1 and Bevy is pinned to 0.19.1.
- CI runs rustfmt, clippy, unit tests, and a release build.
- CI fails if `tangle-model` or `tangle-sim` depend on Bevy or on an app crate.

# Progress

Workspace, toolchain pin, the four crates, the CI workflow, and the dependency
direction guard are in place. `cargo fmt --all --check` and
`cargo clippy --workspace --all-targets --all-features` pass locally against
Rust 1.98.1 and Bevy 0.19.1. Tests and the release build are not yet verified
locally: the development disk filled while compiling Bevy, so confirm both in CI.

Parent [[TAS-001-phase-1-increment-0]].
