---
context_rev: 1
priority: P1
updated: 2026-09-12T13:10:41Z
summary: Terminal backend selection proves Kitty graphics support and falls back safely to character cells.
next: Implement capability probing and fallback that never emits Kitty sequences on unsupporting terminals.
---

# Outcome

`tangle-tui` selects a terminal backend from proven capabilities: it uses Kitty
graphics only when support is confirmed, falls back to character cells
otherwise, and never leaves the terminal corrupted.

# Done when

- Detection consults environment hints and terminal queries, and treats
  ambiguous answers as unsupported.
- A positive probe gates the backend but is never treated as proof of safety:
  [[TAS-010-kitty-graphics-poc]] shows a terminal that answers the probe `OK`
  and then crashed under the naive same-id lifecycle. Selecting the Kitty
  backend always uses the bounded lifecycle mandated by
  [[DEC-002-terminal-backend-strategy]].
- An explicit `--backend ascii|kitty|auto` flag overrides detection, with `auto`
  as the default.
- Falling back after a partial failure restores the alternate screen, cursor,
  and raw mode, and preserves playback position and selection.
- Detection and fallback are unit-tested against a fake capability responder and
  a fake terminal writer.

# Context

Depends on [[DEC-002-terminal-backend-strategy]] at context_rev 2.
Depends on [[TAS-012-terminal-viewer-ascii]] at context_rev 1.
Detection only matters for an opt-in backend, so this follows the decision and
the character-cell fallback.

Parent [[TAS-009-renderer-backends]].
