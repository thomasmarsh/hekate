---
context_rev: 1
priority: P2
updated: 2026-09-12T12:34:01Z
summary: Opt-in Kitty graphics terminal backend renders pixel frames of the shared scene.
next: Implement the Kitty graphics backend behind the shared contract only if DEC-002 adopts it, otherwise dispose this node.
---

# Outcome

When [[DEC-002-terminal-backend-strategy]] adopts Kitty graphics, `tangle-tui`
can render the shared scene as placed pixel images through the Kitty graphics
protocol at a measured frame rate, with no change to simulation results.

# Done when

- An RGBA rasterizer renders the same `SceneFrame` as the character backend,
  sharing geometry and style definitions.
- Frames are transmitted with chunked base64 payloads, placed, replaced, and
  deleted without leaking images or flicker.
- tmux and screen passthrough is handled, or the backend declines when it
  detects it cannot transmit.
- Frame rate and transmit cost are documented for the walking scenario and for
  a denser synthetic scene.
- Selecting the backend never changes which ticks the kernel visits.

# Context

Depends on [[DEC-002-terminal-backend-strategy]] at context_rev 1.
Depends on [[TAS-010-kitty-graphics-poc]] at context_rev 1.
Depends on [[TAS-011-shared-render-layer]] at context_rev 1.
If the decision rejects Kitty graphics, resolve this node with a `disposition`
of `abandoned` and record the character-cell backend as the replacement.

Parent [[TAS-009-renderer-backends]].
