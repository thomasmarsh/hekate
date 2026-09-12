---
context_rev: 1
priority: P1
updated: 2026-09-12T15:12:00Z
summary: Render one simulation through interchangeable GUI and terminal renderer backends.
---

# Outcome

Tangle can present the same live `Simulation` through more than one renderer
backend: the existing Bevy GPU viewer and at least one terminal (TUI) viewer,
sharing a Bevy-free and terminal-free presentation layer behind a backend
contract that each renderer implements.

# Done when

- A Bevy-free, terminal-free presentation layer owns scenario loading, the
  playback clock, the scene and viewport model, input commands, and selection,
  and is the only path by which any backend reads simulation state.
- The Bevy viewer is reimplemented as one backend on that contract with no
  change to playback semantics or to the headless trace hash.
- A terminal viewer renders the same scene as colored character cells and runs
  the walking-skeleton scenario with pause, single-step, speed, restart, and
  selection controls.
- Kitty graphics is either delivered as an opt-in terminal backend or
  explicitly rejected by a recorded decision backed by spike evidence.
- The dependency guard forbids Bevy and terminal crates from the shared layer
  and from the kernel.
- Backend rendering is pinned by deterministic golden-output tests.

# Context

Depends on [[TAS-006-bevy-viewer-skeleton]] at context_rev 1.
That node left the only presentation path inside the Bevy app; this epic extracts
presentation into a shared layer plus interchangeable backends and adds the
terminal backends.

The kernel remains authoritative: no backend may read wall-clock time or mutate
simulation state. Backend choice and frame pacing must not change which ticks
the kernel visits, matching the Increment 0 trace-hash gate.

# Sequencing

The terminal rendering survey scopes the Kitty proof of concept and the terminal
backend decision. The shared layer and its contract unlock the character-cell
backend, the conditional Kitty backend, and capability detection. The
conformance suite closes the epic once the backends are deterministic.

Parent [[IDX-001-tangle]].

# Result

All Done-when criteria are met and every child is resolved; the epic closes on
integrated child evidence rather than child counts.

- Shared presentation layer: [[TAS-011-shared-render-layer]] realized
  [[DEF-002-renderer-backend-contract]] in `crates/tangle-present`, which owns
  scenario loading, the playback clock, `Viewport`, `SceneGeometry`,
  `SceneFrame`, `ViewCommand`/selection, and the `RendererBackend` trait. It
  depends only on `tangle-model`, `tangle-sim`, `glam`, and `thiserror`.
- Bevy backend: [[TAS-011-shared-render-layer]] moved the clock and projection
  out of `tangle-viewer` and made `CurrentFrame` the Bevy `RendererBackend`;
  playback semantics and the headless golden trace hash are unchanged.
- Terminal backend: [[TAS-012-terminal-viewer-ascii]] renders the same frame as
  colored character cells through a `Write` sink with the shared pause,
  single-step, speed, restart, pan, zoom, select, and overlay commands.
- Kitty graphics: [[DEC-002-terminal-backend-strategy]] adopts it as opt-in
  behind a bounded pair lifecycle; [[TAS-010-kitty-graphics-poc]] supplies the
  spike evidence, and [[TAS-013-terminal-viewer-kitty]] implements the backend.
  [[TAS-014-terminal-capability-detection]] gates selection on a positive probe
  and falls back to cells without losing playback state.
- Dependency guard: `scripts/check-dependency-direction.sh` forbids Bevy,
  terminal crates, and application crates from `tangle-present` and forbids
  Bevy, `tangle-present`, and application crates from the kernel; CI runs it.
- Deterministic pinning: [[TAS-015-renderer-conformance-suite]] adds checked-in
  scene, character-grid, and Kitty byte goldens plus a clock/direct parity test
  that hashes to the same canonical trace.

The one frontier action this node named, [[TAS-015-renderer-conformance-suite]],
is resolved; no child remains open.

# Evidence

`cargo test --workspace --all-features`, `cargo fmt --all --check`, `cargo
clippy --workspace --all-targets --all-features -- -D warnings`, and
`./scripts/check-dependency-direction.sh` are clean on the integrated tree, and
the checked-in `tests/golden/walking_guide_v1.trace.sha256` is unchanged. The
end state has one simulation presented through the Bevy viewer, the
character-cell terminal viewer, and the opt-in Kitty terminal viewer, all behind
the same `RendererBackend` contract.
