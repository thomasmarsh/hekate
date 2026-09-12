---
context_rev: 1
priority: P1
updated: 2026-09-12T12:28:46Z
summary: Bevy viewer drives the kernel in whole steps and renders the walking skeleton with controls and inspection.
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
- Rendered guide paths, portals, and shared-mesh car rectangles; every agent
  reuses one mesh and one material with no per-agent assets. Traversable
  polygons and pedestrian discs join the same renderer when the Increment 1
  scenario schema and pedestrian population exist.
- Controls for pause/resume, single tick, 1x/4x/max speed, restart same seed,
  restart next seed, and overlay toggles.
- Click-to-inspect shows physical state, route, profile, current intent, and the
  most recent decision reason. Increment 0 has no decisions, so the viewer
  reports the constant-speed intent and states that no decision has occurred.
- Pause and speed controls never change the trace hash produced for the same
  seed by the headless command.

# Result

Delivered `apps/tangle-viewer` as a Bevy-free library plus a binary. The
`presentation` module owns `PresentationClock`, which accumulates wall time in
integer nanoseconds and yields whole fixed steps; the `scenario` module loads a
JSON5 file through the same parse/validate/compile path as `tangle-cli`. The
binary spawns an orthographic camera fitted to the scenario, draws guide paths
and portals with gizmos, and renders each car from one shared unit-rect mesh and
one shared material, scaled and rotated at the `f64`-to-`f32` boundary.
`R`/`N` restart with the same/next seed, `space` pauses, `.` single steps,
`1`/`2`/`3` select 1x/4x/max, `G`/`V` toggle geometry and velocity overlays,
and a click selects the nearest car for the inspector panel.

Evidence:

- `cargo test -p tangle-viewer`: 11 tests. Clock tests show that the same total
  wall time buys the same whole-step count under different frame partitions,
  that pausing adds no time-based steps, and that a clock-driven run visits the
  identical kernel states as direct stepping for every speed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` and
  `cargo fmt --all --check` pass.
- The binary launches on the pinned toolchain, creates a Metal window, loads the
  walking scenario, and runs without panicking.

Parent [[TAS-001-phase-1-increment-0]].
