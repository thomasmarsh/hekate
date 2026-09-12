---
context_rev: 1
priority: P1
updated: 2026-09-12T12:40:50Z
summary: Render one simulation through interchangeable GUI and terminal renderer backends.
next: Execute the Kitty graphics go/no-go spike scoped by the survey ([[TAS-010-kitty-graphics-poc]]).
---

# Outcome

Tangle can present the same live `Simulation` through more than one renderer
backend: the existing Bevy GPU viewer and at least one terminal (TUI) viewer,
sharing a Bevy-free and terminal-free presentation layer behind a backend
contract that each renderer implements.

# Done when

- A Bevy-free, terminal-free presentation layer owns scenario loading, the
  playback clock, the scene and viewport model, input commands, and selection,
  and is the only path by which any backend reads simulation state.
- The Bevy viewer is reimplemented as one backend on that contract with no
  change to playback semantics or to the headless trace hash.
- A terminal viewer renders the same scene as colored character cells and runs
  the walking-skeleton scenario with pause, single-step, speed, restart, and
  selection controls.
- Kitty graphics is either delivered as an opt-in terminal backend or
  explicitly rejected by a recorded decision backed by spike evidence.
- The dependency guard forbids Bevy and terminal crates from the shared layer
  and from the kernel.
- Backend rendering is pinned by deterministic golden-output tests.

# Context

Depends on [[TAS-006-bevy-viewer-skeleton]] at context_rev 1.
That node left the only presentation path inside the Bevy app; this epic extracts
presentation into a shared layer plus interchangeable backends and adds the
terminal backends.

The kernel remains authoritative: no backend may read wall-clock time or mutate
simulation state. Backend choice and frame pacing must not change which ticks
the kernel visits, matching the Increment 0 trace-hash gate.

# Sequencing

The terminal rendering survey scopes the Kitty proof of concept and the terminal
backend decision. The shared layer and its contract unlock the character-cell
backend, the conditional Kitty backend, and capability detection. The
conformance suite closes the epic once the backends are deterministic.

Parent [[IDX-001-tangle]].
