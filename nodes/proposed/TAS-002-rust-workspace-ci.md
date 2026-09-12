---
context_rev: 1
priority: P1
updated: 2026-09-12T01:30:53Z
summary: Create the pinned Rust workspace, lint and test gates, and release-build CI.
next: Add a Cargo workspace with rust-toolchain.toml pinning Rust 1.98.1 and the four initial crates/apps.
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

Parent [[TAS-001-phase-1-increment-0]].
