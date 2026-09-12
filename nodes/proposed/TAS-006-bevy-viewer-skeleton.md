---
context_rev: 1
priority: P1
updated: 2026-09-12T01:30:53Z
summary: Build the Bevy top-down walking-skeleton viewer with controls and agent inspection.
next: Render the path, portals, and constant-speed car rectangles from snapshots under an orthographic camera.
---

# Context

Depends on [[TAS-004-sim-fixed-step-kernel]] at context_rev 1.
Depends on [[TAS-005-cli-canonical-trace]] at context_rev 1.

# Outcome

A Bevy viewer that owns a `Simulation`, accumulates presentation time, calls
`step()` zero or more whole ticks, interpolates between snapshots, and lets the
user watch, pause, single-step, change speed, restart with the same or next
seed, and inspect an agent.

# Done when

- Orthographic top-down camera with pan and zoom.
- Rendered traversable polygons, guide path, portals, and instanced/shared car
  rectangles and pedestrian discs; no unique mesh or material per agent.
- Controls for pause/resume, single tick, 1x/4x/max speed, restart same seed,
  restart next seed, and overlay toggles.
- Click-to-inspect shows physical state, route, profile, current intent, and the
  most recent decision reason.
- Pause and speed controls never change the trace hash produced for the same
  seed by the headless command.

Parent [[TAS-001-phase-1-increment-0]].
