---
status: resolved
context_rev: 2
updated: 2026-09-15T01:22:43Z
summary: Default terminal backend is character cells; Kitty graphics is opt-in and must use the bounded pair lifecycle.
---

# Context

Depends on [[THO-002-terminal-rendering-landscape]] at context_rev 2.
Depends on [[TAS-010-kitty-graphics-poc]] at context_rev 1.

# Decision

The terminal viewer defaults to a character-cell (ANSI) backend that works on
any terminal. Kitty graphics is an opt-in backend, selected only when the user
requests it or when support is proven and the backend uses the mandated
lifecycle, and it falls back to character cells without losing playback state.

The mandated lifecycle is not optional:

- transmit-only `a=t` followed by a stable placement
  `a=p,i=<id>,p=<placement>`;
- never rely on same-id `a=T` re-transmission to replace image data and
  placements;
- `C=1` so cursor advance cannot push placements into scrollback;
- delete images explicitly and bound in-flight frames and raw transmit bytes.

# Rationale

Character cells are universal and sufficient for debug geometry and behavior.
Kitty graphics buys fidelity only on a minority of terminals, and its support
probe proves protocol support, not stability: [[TAS-010-kitty-graphics-poc]]
records a WezTerm 0.1.0 abort inside placement removal under the naive same-id
re-transmission lifecycle, and a clean run under the bounded `pair` lifecycle on
the same terminal and multiplexer. The lifecycle discipline, not detection, is
what makes an opt-in pixel backend safe, so it is required wherever the backend
is used.

# Consequences

- The character-cell backend is required and remains the only automatic
  fallback; the Kitty backend is conditional on this decision.
- [[TAS-013-terminal-viewer-kitty]] must implement the mandated bounded
  lifecycle and must not use a same-id `a=T` re-transmission path.
- [[TAS-014-terminal-capability-detection]] may gate the backend, but a positive
  probe is necessary and not sufficient; the explicit `--backend` override and
  safe fallback remain required.
- The shared scene layer cannot assume truecolor or pixel placement.

Parent [[TAS-009-renderer-backends]].
