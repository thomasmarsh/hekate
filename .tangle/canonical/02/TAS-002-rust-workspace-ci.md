---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Create the pinned Rust workspace, lint and test gates, and release-build CI.
---

# Outcome

A formatted, linted, tested Rust workspace that builds in release mode with a
pinned toolchain and pinned dependencies, with the one-way dependency direction
enforced.

# Done when

- Workspace contains `crates/hekate-model`, `crates/hekate-sim`,
  `apps/hekate-cli`, and `apps/hekate-viewer`.
- `rust-toolchain.toml` pins Rust 1.98.1 and Bevy is pinned to 0.19.1.
- CI runs rustfmt, clippy, unit tests, and a release build.
- CI fails if `hekate-model` or `hekate-sim` depend on Bevy or on an app crate.

# Result

Complete and verified green in CI. Run
[34667009542](https://github.com/thomasmarsh/hekate/actions/runs/34667009542)
on `0f1d885` passed every gate:

- Check formatting (`cargo fmt --all --check`) — success.
- Lint (`cargo clippy --workspace --all-targets --all-features`) — success.
- Test (`cargo test --workspace --all-features`) — success.
- Release build (`cargo build --workspace --release`) — success.
- Enforce dependency direction — success.

The workspace, Rust 1.98.1 toolchain pin, Bevy 0.19.1 pin, the four crates, and
the dependency-direction guard are all in place and confirmed by CI.

Parent [[TAS-001-phase-1-increment-0]].
