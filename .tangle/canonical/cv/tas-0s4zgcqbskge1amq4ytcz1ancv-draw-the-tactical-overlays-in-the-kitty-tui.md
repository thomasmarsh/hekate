---
context_rev: 1
status: active
updated: 2026-09-15T12:53:59Z
summary: Draw the tactical overlays in the Kitty TUI path and open a wrong-way key
next: Wire the close-pass visibility into both TUI backends and the safety overlay.
---

Area [[IDX-001-hekate]].

# Outcome

Both TUI backends draw the five Increment 2 tactical overlays, and a normal TUI
run can open a wrong-way interval so the rule-state overlay is reachable.

# Done when

- `PixelRasterizer::rasterize` draws the usable corridor, target offset,
  predicted gap, maneuver, and wrong-way overlays in `Overlay` declaration
  order, each gated on its own `frame.overlays` flag and drawn only from the
  frame projected data.
- A TUI key requests a wrong-way entry for the selected agent that carries
  route state, and the footer legend names it.
- The pixel, session, presenter, viewer, and dependency-direction checks pass.

# Context

Reads [[tho-2nk34hdj3g8dr3tm9b28n1c7zw-increment-2-presenter-residuals-corridor-doc]].
TAS-135 deferred the Kitty pixel path, and no production path calls
`Simulation::request_wrong_way_entry`, so both gaps are real rather than
regressions.

# Result

Both gaps are closed in the host, so no shared-layer change is needed.

Gap 1 (Kitty parity): `PixelRasterizer::rasterize` now dispatches the five
tactical overlays in `Overlay` declaration order after `draw_safety` and before
`draw_vectors`, each gated on its own `frame.overlays` flag: `draw_corridors`
(segment from `anchor + left * (d - offset_m)`), `draw_target_offsets`,
`draw_predicted_gaps` (world-space ring via the cell backend's own
`circle_ring`, now `pub(crate)`), `draw_maneuvers` and `draw_wrong_way`. The
pixel vocabulary has no glyphs, so a body marker is a hollow pixel-space ring
in the shared `maneuver_color`/`wrong_way_color` of its live state, the
maneuver ring inside the wrong-way ring so one body carrying both markers shows
both; the corridor, target, gap and color helpers are the cell backend's own.
The kitty call chain needed no change: `KittyBackend::draw` already calls
`PixelRasterizer::rasterize`, so `c/t/p/m/o` now change the transmitted image.

Gap 3 (wrong-way seam): `TuiSession::request_wrong_way_entry` projects the
frame, requires a selected body carrying route state, and forwards to
`Simulation::request_wrong_way_entry`; `main.rs` binds it to `e` (the five
letters the pan and overlay bindings leave free), and the shared footer legend
names it beside the `o wrong-way` toggle.

Tests: pixel `every_tactical_overlay_is_flag_gated_and_drawn_from_the_frame`
(each of the five is drawn and flag-gated from a fixture frame, both predicted-
gap margins draw, and a Phase 1 frame draws none) and session
`the_wrong_way_trigger_needs_a_selected_agent_with_route_state` plus
`the_wrong_way_trigger_opens_the_selected_agents_opposing_traversal`, which
drives the checked-in `narrow_wrong_way_v2` fixture to corridor a's own entry
window (5..=20 m), selects its lone rider, requests the entry, and asserts the
inspector then reports the open opposing traversal. The stale claim in
`tests/inc2_fixture_sessions.rs` that the session API cannot record a request is
corrected.

Verified: `cargo fmt --all --check`; `cargo test -p hekate-tui` (84 lib + 18
integration); `cargo test -p hekate-present`; `cargo test -p hekate-viewer`;
`cargo clippy -p hekate-tui --all-targets -- -D warnings`;
`./scripts/check-dependency-direction.sh`. Goldens unchanged: the walking
fixture is Phase 1, so its pixel image carries no tactical draw, and the kitty
golden truncates the footer row before the new legend key.

Left in scope for the next slice: close-pass visibility (Gap 2), which is a
shared-presenter change (`is_safety_record`) plus a marker arm in each backend.
