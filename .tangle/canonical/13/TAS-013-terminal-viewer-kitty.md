---
status: resolved
context_rev: 1
priority: P2
updated: 2026-09-15T01:22:43Z
summary: Opt-in Kitty graphics backend places pixel frames of the shared scene behind the mandated bounded pair lifecycle.
---

# Outcome

When [[DEC-002-terminal-backend-strategy]] adopts Kitty graphics, `hekate-tui`
can render the shared scene as placed pixel images through the Kitty graphics
protocol at a measured frame rate, with no change to simulation results.

# Done when

- An RGBA rasterizer renders the same `SceneFrame` as the character backend,
  sharing geometry and style definitions.
- Frames are transmitted with chunked base64 payloads, placed, replaced, and
  deleted without leaking images or flicker.
- Transmission uses the bounded lifecycle mandated by
  [[DEC-002-terminal-backend-strategy]]: transmit-only `a=t` plus a stable
  placement `a=p,i=<id>,p=<placement>`, `C=1`, explicit deletion, and bounded
  in-flight frames and raw bytes. There is no same-id `a=T` re-transmission
  path, which crashed WezTerm 0.1.0 in [[TAS-010-kitty-graphics-poc]].
- tmux and screen passthrough is handled, or the backend declines when it
  detects it cannot transmit.
- Frame rate and transmit cost are documented for the walking scenario and for
  a denser synthetic scene.
- Selecting the backend never changes which ticks the kernel visits.

# Context

Depends on [[DEC-002-terminal-backend-strategy]] at context_rev 2.
Depends on [[TAS-010-kitty-graphics-poc]] at context_rev 1.
Depends on [[TAS-011-shared-render-layer]] at context_rev 1.
If the decision rejects Kitty graphics, resolve this node with a `disposition`
of `abandoned` and record the character-cell backend as the replacement.

Parent [[TAS-009-renderer-backends]].

# Result

All Done-when criteria are met. `hekate-tui` has an opt-in Kitty graphics
backend on the shared contract, selected only through the
[[TAS-014-terminal-capability-detection]] gate, with the bounded lifecycle
mandated by [[DEC-002-terminal-backend-strategy]].

- `pixel.rs` adds `RgbaImage` and `PixelRasterizer`. It reuses
  `raster::Rasterizer`'s world-to-cell projection and the same color constants
  (`PATH_COLOR`, `PORTAL_COLOR`, `BODY_COLOR`, `SELECTED_COLOR`,
  `VECTOR_COLOR`, `BACKGROUND`), then scales each cell to a fixed pixel block
  (default 6x12, so a cell stays 1:2), so the pixel image is the same picture
  as the character grid. The scale shrinks to keep a frame within 1920x1080.
- `kitty.rs` adds the protocol: a local RFC 4648 base64 encoder, chunked
  transmission at 4096 base64 bytes with control data only on the first chunk,
  `apc`, tmux/screen passthrough wrapping, and `Multiplexer` detection from
  `TMUX`/`STY`. `KittyBackend<W>` implements `RendererBackend` for any `Write`
  sink so tests capture bytes.
- The lifecycle is the mandated `pair` form: transmit-only `a=t,f=32`, then a
  stable `a=p,i=1,p=1,C=1,c=<cols>,r=<rows>`, all with `q=2`. No `a=T`
  re-transmission path exists. The image is deleted with `a=d,d=i,i=1` on
  resize, when the 32 MiB raw budget would be exceeded, and on `shutdown`;
  the placement id is stable, so re-placing replaces rather than stacks.
- GNU screen's passthrough cannot carry a graphics string of this size; the
  backend detects that and declines with an error instead of emitting a
  truncated image. tmux is wrapped in `ESC Ptmux; ... ESC \` with doubled
  escapes.
- `hud.rs` now owns the two status lines and the inspector/help footer for
  both backends, so the pixel and character views show identical text.
- `session.rs` generalizes `TuiSession` over a `SessionBackend` trait;
  `fallback.rs` adds `BackendPair`, which runs the opt-in Kitty backend behind
  `BackendFallback` and replays the same `SceneFrame` in character cells when a
  draw or present fails. `main.rs` builds the pair for `--backend
  ascii|kitty|auto`, restores the terminal on fallback, and calls `shutdown`
  before leaving the alternate screen. The previous "Kitty not built" note is
  gone.

## Measured cost

Raw transport (`a=t,f=32`) means transmit cost is fixed by pixel count and does
not depend on compressibility. Measured with
`cargo run -p hekate-tui --release --example kitty-bench` (240 frames; includes
raster, base64, and a discarding sink, so terminal write/parse is excluded):

- walking, `80x24` terminal -> `480x252` px: 1.2-2.8 ms/frame CPU,
  646,962 bytes/frame on the wire.
- dense synthetic, `160x48` terminal with 200 bodies -> `960x540` px:
  9.8-18.1 ms/frame CPU, 2,771,537 bytes/frame on the wire.

Add the raw-pty write cost already measured in [[TAS-010-kitty-graphics-poc]]
(14.7 ms for 480x270, 41.3 ms for 960x540) for a real-terminal bound of roughly
16-18 ms/frame walking and 50-59 ms/frame dense. The backend therefore prefers
small viewports and flat geometry, matching the spike's residual guidance; a
compressed (zlib) transport would cut the walking scene's wire cost by ~370x but
is not adopted here.

# Evidence

`cargo test -p hekate-tui` is 63 library tests plus 3 argument tests plus
`walking_parity` plus a new `backend_parity` integration test, all green. New
tests cover RFC 4648 base64 vectors, chunk framing and control-only-on-first,
that no output ever contains `a=T`, transmit plus stable `a=p,...,C=1`
placement plus HUD, budget-triggered delete, explicit `shutdown` delete,
tmux wrapping, screen declining and staying declined, resize deleting the old
placement, and pixel raster determinism, shared colors, overlay toggling, and
selection color. `backend_parity` drives the same seed through the cell and
Kitty sessions with identical irregular partitions and matches direct kernel
stepping at tick 20.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace --all-features`, and
`./scripts/check-dependency-direction.sh` are clean; the checked-in golden trace
hash is unchanged; `cargo build --workspace --release` succeeded with thin LTO
(measured 8m54s).

# Residual

Real tmux and screen passthrough was unit-tested but not exercised against a
live multiplexer here; the spike's real tmux gap remains consciously accepted.
Compression is not adopted, so a large dense viewport can exceed a 60 Hz budget
on the wire. [[TAS-009-renderer-backends]] advances to
[[TAS-015-renderer-conformance-suite]].
