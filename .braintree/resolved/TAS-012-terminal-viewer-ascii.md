---
context_rev: 1
priority: P1
updated: 2026-09-12T13:54:38Z
summary: Terminal viewer renders the shared scene as colored character cells with full playback controls.
---

# Outcome

A `tangle-tui` binary renders the shared `SceneFrame` as colored character
cells, runs the walking-skeleton scenario, and offers the same playback and
inspection controls as the Bevy viewer.

# Done when

- A cell rasterizer maps world geometry, portals, and agent bodies into a
  character grid with aspect-ratio correction and ANSI color, degrading to
  256-color or 16-color output when truecolor is unavailable.
- Pause, single step, speed, restart same and next seed, pan, zoom, select, and
  overlay toggles are available and emitted as shared `ViewCommand`s.
- A status line reports scenario, simulated time, tick, seed, speed, and agent
  counts; selecting an agent shows the same state the Bevy inspector shows.
- Frame output goes through a `Write` sink so tests capture it instead of a real
  terminal.
- The same seed drives the same ticks as the Bevy viewer and the headless CLI.

# Context

Depends on [[DEF-002-renderer-backend-contract]] at context_rev 1.
Depends on [[TAS-011-shared-render-layer]] at context_rev 1.
Depends on [[DEC-002-terminal-backend-strategy]] at context_rev 2.

Parent [[TAS-009-renderer-backends]].

# Result

All Done-when criteria are met. `tangle-tui` is a crossterm terminal backend on
the shared presentation contract.

- New `apps/tangle-tui` depends only on `tangle-model`, `tangle-sim`,
  `tangle-present`, `crossterm`, and `glam`. `crates/tangle-present` and the
  kernel still forbid terminal crates, so
  `scripts/check-dependency-direction.sh` reports `dependency direction OK`.
- `raster.rs` projects world geometry, portals, and oriented agent bodies into a
  `CellGrid` with a 1:2 cell aspect correction (one row spans two columns of
  world). `palette.rs` converts 24-bit colors to the nearest 256-color or
  16-color palette entry; `grid.rs` coalesces color runs and emits full-width
  ANSI rows. Detected depth is truecolor, 256, or 16.
- `backend.rs` implements `RendererBackend` for `CellBackend<W: Write>`: `draw`
  rasterizes and builds a two-line status plus the inspector/help footer, and
  `present` writes exactly `height` full-width rows through the sink so a frame
  cannot scroll the alternate screen. Tests capture `Vec<u8>`, not a terminal.
- Pause, single step, 1x/4x/max speed, restart same and next seed, pan, zoom,
  cycle-select, clear selection, and both overlay toggles translate to the
  shared `ViewCommand`s in `main.rs`. The inspector prints the same fields as the
  Bevy viewer: position, heading, speed, path, distance, body size, intent, and
  decision.
- Tests: `cargo test -p tangle-tui` runs 27 green tests, including a
  `Write`-sink frame capture and a checked-in walking-scenario parity test that
  drives one seed through irregular frame partitions and matches direct
  `Simulation::step` state at tick 20.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace --all-features`, and
  `cargo build --workspace --release` are clean; the checked-in golden trace
  hash is unchanged.

The capability-detection fallback ([[TAS-014-terminal-capability-detection]]) is
the next frontier child; [[TAS-013-terminal-viewer-kitty]] and
[[TAS-015-renderer-conformance-suite]] are unblocked siblings.
