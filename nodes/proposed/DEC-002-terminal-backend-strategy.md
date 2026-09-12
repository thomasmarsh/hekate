---
context_rev: 1
updated: 2026-09-12T12:34:01Z
summary: Default terminal backend is character cells; Kitty graphics is opts in only when support is proven.
---

# Context

Depends on [[THO-002-terminal-rendering-landscape]] at context_rev 1.
Depends on [[TAS-010-kitty-graphics-poc]] at context_rev 1.

# Decision

The terminal viewer defaults to a character-cell (ANSI) backend that works on
any terminal. Kitty graphics is an opt-in backend selected at runtime only when
support is proven, and it falls back to character cells without losing playback
state.

This node stays proposed until the survey and spike report. Adopt Kitty graphics
only if the spike shows acceptable throughput and a detection signal that cannot
silently corrupt output; otherwise record the rejection here and keep character
cells as the only terminal backend.

# Rationale

Character cells are universal and sufficient for debug geometry and behavior;
Kitty graphics buys fidelity only on a minority of terminals, and its silent
failure modes are worse than lower resolution.

# Consequences

- The character-cell backend is required; the Kitty backend is conditional on
  this decision and on the spike outcome.
- Capability detection and safe fallback are required for any opt-in backend.
- The shared scene layer cannot assume truecolor or pixel placement.

Parent [[TAS-009-renderer-backends]].
