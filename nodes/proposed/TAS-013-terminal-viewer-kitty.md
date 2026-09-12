---
context_rev: 1
priority: P2
updated: 2026-09-12T13:10:41Z
summary: Opt-in Kitty graphics terminal backend renders pixel frames of the shared scene with a bounded lifecycle.
next: Implement the Kitty graphics backend behind the shared contract with the mandated pair lifecycle.
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
