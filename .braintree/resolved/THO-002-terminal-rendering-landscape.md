---
context_rev: 2
updated: 2026-09-12T12:41:54Z
summary: Character cells are the portable default; Kitty graphics is opt-in only behind proven support and a measured-throughput spike.
---

# Question

Which terminal rendering techniques can present the Tangle scene, and what does
each require of target terminals, the shared presentation layer, and the
fallback path?

# Candidate techniques

1. Character cells with ANSI color: portable to every terminal, including macOS
   Terminal.app, but bounded by cell resolution and approximate geometry.
2. Unicode half-blocks or braille for finer resolution inside a character cell:
   still portable, more samples per cell, more complex rasterization.
3. Kitty graphics protocol: true pixel images placed in the terminal, richest
   output, but limited to Kitty, WezTerm, Ghostty, and Konsole, needs
   passthrough under tmux or screen, and has no universally reliable probe.

# Constraints to weigh

- Detection: environment hints (`TERM`, `TERM_PROGRAM`, `KITTY_WINDOW_ID`) and
  terminal queries (DA1, `XTGETTCAP`) versus the risk of emitting graphics
  sequences into a terminal that silently mishandles them.
- Fallback: the terminal backend must degrade to character cells without losing
  playback position or selection.
- Aspect ratio: terminal cells are roughly twice as tall as wide, so the
  character path needs its own projection correction that the pixel path does
  not share.
- Input: terminals deliver keys and pointer events as escape sequences; the
  shared layer should own normalized view commands so both backends stay
  behaviorally identical.
- Stack: `crossterm` plus a small cell buffer versus `ratatui`, and whether cell
  diffing belongs in the shared layer or in the character backend.

# Findings

## Viable techniques

Three techniques can present the Tangle scene; they are cumulative rather than
exclusive.

1. **ANSI character cells (required baseline).** One styled character per cell.
   Universal: macOS Terminal.app, iTerm2, Alacritty, xterm, VS Code, any SSH
   session, and tmux/screen. Resolution is bounded by the cell grid (80x24
   default, perhaps 200x50 maximized), so thin paths alias and geometry is
   approximate. Truecolor (`SGR 38;2`/`48;2`) is common but not universal;
   256-color and 16-color fallbacks are required.
2. **Subcell glyphs (recommended enhancement inside the character backend).**
   Half-blocks give 1x2 subcells per cell, braille (U+2800 block, 2x4 dots)
   gives 2x4, and quadrant/sextant/octant blocks give 2x2/2x3/2x4 with spottier
   font coverage. Half-blocks double vertical samples, matching the cell aspect
   and roughly doubling effective resolution with no new protocol support.
   Braille-style glyphs suit lines and points, not filled regions. This is an
   implementation choice inside the character backend, not a separate backend.
3. **Kitty graphics protocol (optional pixel backend).** APC-framed
   `ESC_G...ESC\` commands transmit RGBA or PNG pixel data, base64-encoded in
   chunks of at most 4096 bytes, and place the image at a cell position with
   pixel-precise offset and scaling for true fidelity. Supported by Kitty,
   WezTerm, Ghostty, Konsole (partial), and iTerm2 (via APC G, per its own
   docs); not supported by macOS Terminal.app, Alacritty, xterm, or the VS Code
   terminal. tmux and zellij need passthrough enabled and the sequence wrapped;
   screen is unreliable.

## Transmission cost is the decisive constraint

The protocol's dominant cost is moving pixel data over the PTY. An 800x480 RGBA
frame is 1.5 MB raw, about 2 MB base64, split into roughly 500 escape
sequences. At 60 Hz that is about 120 MB/s of terminal traffic, sustainable
locally only in bursts and not over SSH. Mitigations in order: shrink the frame
to the visible viewport capped at the terminal's pixel size; compress with zlib
(`o=z`) before base64, which suits flat-colored debug geometry; or use a local
medium (`t=f` file or `t=s` shared memory) so only a name crosses the PTY,
which needs a shared filesystem and an emulator that permits it. The character
backend has none of this cost: one write per changed cell.

## Detection cannot be fully trusted

- Environment hints (`TERM=xterm-kitty`, `KITTY_WINDOW_ID`,
  `TERM_PROGRAM=WezTerm|ghostty`, `WEZTERM_EXECUTABLE`, `GHOSTTY_RESOURCES_DIR`,
  `KONSOLE_VERSION`) are cheap but spoofable and wrong through SSH, tmux, and
  nested sessions.
- The protocol's own probe is a query action (`a=q`) followed by a primary
  device-attributes request (`CSI c`): a device-attributes reply that arrives
  without a graphics reply means unsupported. It requires reading stdin with a
  timeout while other input may arrive, and a multiplexer must forward both
  queries.
- No probe is simultaneously reliable and safe on every terminal, so the safe
  default is conservative: do not emit graphics without a positive signal.
- APC codes are ignored by most terminals, so a stray sequence is usually
  swallowed; the real corruption risks are partial implementations, a
  multiplexer that forwards half a sequence, and images left after a crash
  without a delete.

## Aspect ratio and projection

Terminal cells are roughly twice as tall as wide (cell width:height about 1:2),
so the character path needs its own scaling for a world square to look square;
the pixel path does not share that correction and uses a pixel viewport. The
shared layer must expose world-space geometry plus a viewport and let each
backend adapt, rather than assuming a cell or pixel grid.

## Input

Both backends receive keys and optional mouse events as terminal escape
sequences and normalize them to the shared `ViewCommand` stream. The shared
controller owns the clock and viewport, so backends cannot diverge in behavior.
Mouse reporting needs extra terminal modes and must be restored on exit; it is
optional for the first terminal backend.

## Stack

`crossterm` plus a small cell buffer is sufficient: it owns raw mode, alternate
screen, cursor, color capability, and event parsing. `ratatui` adds a
widget/layout tree, double buffering, and diffing, but Tangle's output is custom
rasterized geometry rather than standard widgets, so most of it would be
unused. Cell diffing and double buffering belong in the character backend's
`present`, not the shared layer, because they are terminal-specific.

## Recommendation

- **Default: character cells with ANSI color**, using half-block subcells where
  they help, degrading truecolor to 256-color to 16-color without losing
  playback state. It is the only backend that works on every target terminal,
  including the primary macOS environment's default Terminal.app, and it has no
  detection or throughput risk.
- **Fallback: the same character backend** at reduced color; it uses only
  well-supported SGR and cursor sequences, so it cannot silently corrupt output.
- **Kitty graphics: opt-in only, conditional on [[TAS-010-kitty-graphics-poc]].**
  Adopt it only if the spike shows a usable frame rate for a realistic scene and
  a detection signal that cannot silently corrupt an unsupporting terminal;
  otherwise reject it, keep character cells as the only terminal backend, and
  dispose [[TAS-013-terminal-viewer-kitty]] as abandoned. This matches the
  direction already recorded in [[DEC-002-terminal-backend-strategy]].

## Risks the proof of concept must retire

1. Throughput: measure encode, transmit, and achieved frame rate for a
   walking-skeleton-sized and a denser synthetic scene, comparing raw, zlib,
   and shared-memory/file transmission, locally and over SSH.
2. Detection: demonstrate an "unsupported" result on a non-supporting terminal,
   under tmux with passthrough off and on, and nested in another multiplexer,
   without emitting a partial sequence.
3. Lifecycle: prove placements survive resize and scroll and are cleared on
   alternate-screen transitions and abnormal exit, with no leaked images or
   quota exhaustion.
4. Framing: chunking at 4096 bytes, first-chunk-only control data, and
   delete/replace semantics, later pinned by byte fixtures in
   [[TAS-015-renderer-conformance-suite]].
5. Multiplexer story: state which of tmux/zellij/screen can carry the protocol
   and what the user must configure.

## Effect on the shared layer

The survey confirms the `SceneFrame` seam in
[[DEF-002-renderer-backend-contract]]: backends receive world-space geometry
plus a viewport, declare capabilities (character cells, truecolor, pixel
graphics, pointer input), and own their own rasterization and presentation. No
cell or pixel assumption belongs in the shared layer; capability-gated
degradation belongs in the backend selection of
[[TAS-014-terminal-capability-detection]].

Parent [[TAS-009-renderer-backends]].
