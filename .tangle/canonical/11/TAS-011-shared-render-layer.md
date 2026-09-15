---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Extract a Bevy-free and terminal-free presentation crate shared by every backend.
---

# Outcome

`crates/hekate-present` owns scenario loading, the playback clock, scene
projection from snapshots, the viewport model, view commands, and selection; the
Bevy viewer consumes it through the contract in
[[DEF-002-renderer-backend-contract]] with unchanged playback behavior.

# Done when

- `crates/hekate-present` has no Bevy, window, wall-clock, or terminal
  dependency, and `hekate-model` and `hekate-sim` do not depend on it.
- `PresentationClock` and the scene projection move out of `hekate-viewer`; the
  existing clock tests move with them and stay green.
- `hekate-viewer` becomes a Bevy backend on the shared contract; its controls,
  interpolation, overlays, and inspector are unchanged for the user.
- The headless trace hash and the viewer/headless equivalence tests still pass.
- `scripts/check-dependency-direction.sh` forbids Bevy and terminal crates from
  `hekate-present`, `hekate-model`, and `hekate-sim`, and CI runs it.

# Context

Depends on [[DEF-002-renderer-backend-contract]] at context_rev 1.
Depends on [[TAS-006-bevy-viewer-skeleton]] at context_rev 1.
This is a behavior-preserving refactor plus the seam every later backend needs;
it should keep the goldens from [[TAS-005-cli-canonical-trace]] unchanged.

Parent [[TAS-009-renderer-backends]].

# Result

All Done-when criteria are met. [[DEF-002-renderer-backend-contract]] is
realized and now resolved as current knowledge.

- New `crates/hekate-present` depends only on `hekate-model`, `hekate-sim`,
  `glam`, and `thiserror`. It owns `load_scenario`, `PresentationClock`/`Speed`,
  `Viewport`, `Overlays`, `SceneGeometry`, `SceneBody`/`SceneFrame`,
  `ViewCommand`, `PresentationController`, and the `RendererBackend` +
  `BackendCapabilities` contract.
- `PresentationClock` and its kernel-equivalence tests moved out of
  `hekate-viewer` into `hekate-present`; the scene projection, viewport,
  selection, overlays, and command handling moved with them. `cargo test -p
  hekate-present` runs 22 green tests, and the moved
  `clock_driven_run_matches_direct_stepping_at_every_observed_tick` and
  `pausing_does_not_change_states_reached_for_the_same_active_time` still pin
  viewer/headless tick equivalence.
- `hekate-viewer` now builds a `PresentationController` from the loaded
  scenario, translates keys, wheel, and clicks into `ViewCommand`s, and renders
  only from `SceneFrame`. Its `CurrentFrame` resource implements
  `RendererBackend`, so the Bevy viewer is one backend on the shared contract;
  controls, interpolation, overlays, and the inspector keep their existing
  bindings and behavior.
- The headless trace hash is unchanged:
  `cargo test --workspace --all-features` passes
  `checked_in_walking_trace_matches_golden_bytes_and_hash` and
  `same_seed_produces_the_same_hash_twice`, and the Bevy camera is driven from
  the controller viewport so backend choice cannot change kernel ticks.
- `scripts/check-dependency-direction.sh` now forbids Bevy, terminal crates
  (`crossterm`, `ratatui`, `termion`, `termwiz`, `console`), and application
  crates from `hekate-present`, and forbids `hekate-present` from
  `hekate-model` and `hekate-sim`. CI already runs the script, which reports
  `dependency direction OK`.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, and `cargo build --workspace --release` are
  clean.

The contract's `draw` stage captures the frame into `CurrentFrame`; the ECS
render systems act as the `present` stage, which is how Bevy submits to its
window.
