---
context_rev: 1
priority: P1
updated: 2026-09-12T12:34:01Z
summary: Terminal viewer renders the shared scene as colored character cells with full playback controls.
next: Implement the tangle-tui binary with a crossterm cell backend and shared view commands on the walking scenario.
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
