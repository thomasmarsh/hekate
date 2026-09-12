---
context_rev: 1
priority: P2
updated: 2026-09-12T14:10:00Z
summary: Release builds use thin LTO so cross-crate optimization covers the kernel and its consumers.
next: Confirm `cargo build --workspace --release` succeeds with thin LTO and record the build-time cost.
---

# Outcome

The workspace release profile enables `lto = "thin"` so release binaries used
for benchmarks, profiling, and packaging get cross-crate inlining, and the
choice is recorded so later baselines name the profile they measured.

# Done when

- `[profile.release]` sets `lto = "thin"` alongside `debug = false`.
- `cargo build --workspace --release` succeeds with the new profile.
- This node records why thin and not full LTO, the build-time cost, and the
  determinism scope so release benchmarks can name the profile.

# Context

Parent [[IDX-001-tangle]].
The release profile currently sets only `debug = false`, so release binaries get
no cross-crate optimization. `PHASE_1_PLAN.md` scopes canonical-trace
determinism to one target triple, compiler version, and build profile, so a
release profile change does not threaten the checked-in dev-profile golden
trace, but release baselines must state the profile they used.
