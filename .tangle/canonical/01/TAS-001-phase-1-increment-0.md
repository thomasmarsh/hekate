---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Phase 1 Increment 0 is delivered: workspace, kernel, CLI, and a Bevy viewer on one scenario.
---

# Outcome

A running Bevy viewer and a headless command both drive the same `hekate-sim`
kernel on one hand-authored JSON5 scenario with a single guide path and a portal
at each end, showing deterministic constant-speed cars.

# Done when

- Running the same seed twice produces the same canonical trace hash.
- Viewer frame rate, pause, and speed controls do not change that hash.
- No Bevy type appears in the public API or dependency tree of `hekate-model`
  or `hekate-sim`.

# Result

Increment 0 is complete. [[TAS-002-rust-workspace-ci]] pinned the workspace and
CI, [[TAS-003-scenario-source-parse]] delivered the scenario contract,
[[TAS-004-sim-fixed-step-kernel]] the deterministic kernel,
[[TAS-005-cli-canonical-trace]] the headless trace, and
[[TAS-006-bevy-viewer-skeleton]] the viewer and controls.

Evidence:

- `cargo test --workspace --all-features` passes, including the checked-in
  golden trace hash and `same_seed_produces_the_same_hash_twice`.
- Viewer clock tests show that frame partitioning, pause, and speed never change
  the kernel states reached for the same seed and tick count; the headless hash
  is a pure function of that same step sequence.
- `./scripts/check-dependency-direction.sh` reports OK, so no Bevy, `hekate-cli`,
  or `hekate-viewer` edge reaches `hekate-model` or `hekate-sim`.
- `cargo build --workspace --release`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, and `cargo fmt --all --check` pass.

Area [[IDX-001-hekate]].
