---
context_rev: 1
updated: 2026-09-12T12:34:01Z
summary: Renderer backends implement one trait over a Bevy-free scene frame and view-command stream.
---

# Invariant

Every renderer backend consumes the same backend-agnostic data and produces no
simulation-affecting state:

1. A `SceneFrame` is a pure projection of a `tangle-sim` snapshot plus the
   compiled scenario and presentation state: viewport and camera, geometry
   polylines, portals, agent bodies with world transforms and style, overlays,
   selection, and a status summary. It references no Bevy, window, or terminal
   type.
2. A backend implements one trait with `resize`, `draw(&SceneFrame)`, and
   `present`, and declares a capability set (character cells, truecolor, Kitty
   graphics, pointer input, alternate screen).
3. Input is normalized to a `ViewCommand` stream: pause, single step, set speed,
   restart with the same or next seed, pan, zoom, select, clear selection, and
   toggle overlay. Backends translate device events into commands; they never
   own the playback clock and never mutate the simulation directly.
4. The presentation controller owns the clock, viewport, selection, and overlays,
   and applies commands identically for every backend, so backend choice cannot
   change which ticks the kernel visits.
5. The contract lives in a Bevy-free and terminal-free crate
   (`crates/tangle-present`) that `tangle-model` and `tangle-sim` never depend
   on, and that the dependency guard keeps free of Bevy and terminal crates.

# Consequences

The Bevy viewer becomes one implementation of the trait and terminal backends
become others. Scene projection and command handling are testable without a
window or a terminal, and the headless trace-hash gate extends to every backend.

Parent [[TAS-009-renderer-backends]].
