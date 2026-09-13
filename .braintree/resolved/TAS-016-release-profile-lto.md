---
context_rev: 2
priority: P2
updated: 2026-09-12T14:45:45Z
summary: Release builds keep LTO off so builds stay fast until a measurement justifies enabling it.
---

# Outcome

The workspace release profile does not enable LTO. Build time is the priority
for now: release builds back CI gates, benchmarks, and profiling, and thin LTO's
link-time cost is not yet justified by any measured cross-crate benefit. LTO is
reconsidered only when a profile shows a win that outweighs the added build
time, at which point the new choice is recorded.

# Done when

- `[profile.release]` no longer sets `lto`.
- `cargo build --workspace --release` succeeds.
- This node records why LTO is off, the build-time motivation, and the
  determinism scope so release baselines name the profile.

# Context

Parent [[IDX-001-tangle]].
The release profile previously set `lto = "thin"` (commit `c654c7b`) for
cross-crate inlining. In practice that made release builds too slow for the
incremental loop and no benchmark shows a benefit, so the setting is reverted.
Default Cargo LTO is off, so removing the key is the whole change.

`PHASE_1_PLAN.md` scopes canonical-trace determinism to one target triple,
compiler version, and build profile, so a release profile change does not
threaten the checked-in dev-profile golden trace, but release baselines must
state the profile they used.

# Result

- `[profile.release]` sets only `debug = false`, with a comment recording that
  LTO is deliberately off.
- `cargo build --workspace --release` succeeded in ~10s on the incremental tree
  (2026-09-12, macOS, toolchain from `rust-toolchain.toml`).
- No LTO, including thin, until a profile measures a cross-crate win that
  outweighs the build-time cost.
