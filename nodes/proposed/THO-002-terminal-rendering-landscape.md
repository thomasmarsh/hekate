---
context_rev: 1
updated: 2026-09-12T12:34:01Z
summary: Terminal rendering can use character cells or Kitty graphics; tradeoffs set the default backend and fallback.
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

Pending. Record which techniques are viable, the recommended default and
fallback, and the risks the proof of concept must retire.

Parent [[TAS-009-renderer-backends]].
