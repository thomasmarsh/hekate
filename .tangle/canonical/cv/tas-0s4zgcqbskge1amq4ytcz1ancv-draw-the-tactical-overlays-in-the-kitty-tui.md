---
context_rev: 1
status: resolved
updated: 2026-09-15T13:10:41Z
summary: Draw the tactical overlays and close-pass evidence in every backend, and open a wrong-way key
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

## Close-pass visibility (Gap 2)

`EventKind::ClosePass` is now a safety record, so a close pass enters the shared
`SceneFrame` and every backend can draw it. No `SCENE_FORMAT_VERSION`,
`EVENT_VERSION`, or simulation change was needed, and no golden changed.

- `crates/hekate-present/src/safety.rs`: `is_safety_record` carries
  `EventKind::ClosePass`. `EventParticipants` already sorted its passing agent
  and passed body to ascend, and `marker_anchor` already falls through to the
  midpoint of the participants alive in the frame, so a two-agent close pass
  needs no new arm there. New test
  `a_close_pass_record_becomes_a_midpoint_safety_marker` drives a close pass
  whose passing agent is the larger id and asserts ascending participants, no
  region, and the midpoint anchor.
- `apps/hekate-tui/src/raster.rs`: `marker_glyph` returns `P` and `marker_color`
  returns a new `CLOSE_PASS_COLOR` (`Rgb::new(255, 105, 180)`). The pixel
  backend shares both helpers, so it needed only a test. No footer legend
  change: the marker is governed by the existing `b safety` toggle the legend
  already names.
- `apps/hekate-viewer/src/main.rs`: the Bevy safety-shape pass already draws
  every `frame.safety_markers()` entry generically; the minimal additive change
  is a `ClosePass` color arm in `marker_color`, which previously fell into the
  white non-marker group whose comment was then false.

Tests: pixel `a_close_pass_marker_is_drawn_and_flag_gated` and raster
`a_close_pass_marker_is_rasterized_and_flag_gated` build a two-body frame with
only a close-pass record, assert the marker draws, and assert it vanishes with
the safety flag off. Falsified: removing the `is_safety_record` arm fails the
presenter test and both backend tests.

Verified: `cargo fmt --all`; `cargo test -p hekate-present` (53 lib + the
integration binaries); `cargo test -p hekate-tui` (86 lib + the integration
binaries); `cargo test -p hekate-viewer`; `cargo clippy -p hekate-present -p
hekate-tui -p hekate-viewer --all-targets -- -D warnings`;
`./scripts/check-dependency-direction.sh`. Goldens unchanged: the walking
fixture is Phase 1 with no facilities, so no `ClosePass` is produced and every
checked-in golden stays byte-identical.

## Five-gate validation (ui-gate)

Run from the repo root at HEAD `e3c0736` with a clean working tree.

| # | gate | exit | wall | result |
| --- | --- | --- | --- | --- |
| 1 | `cargo fmt --all --check` | 0 | 1 s | clean |
| 2 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 3 s | clean |
| 3 | `cargo test --workspace --all-features` | 0 | 419 s | 1055 passed, 0 failed, 3 ignored across 97 test binaries (no binary failed) |
| 4 | `./scripts/check-dependency-direction.sh` | 0 | 1 s | `dependency direction OK` |
| 5 | `tangle check` | 0 | 0 s | `graph check: passed (205 nodes)` |

## Clause disposition

- **Kitty path draws all five tactical overlays, in `Overlay` declaration order,
  each flag-gated and built only from frame projected data — met.**
  `PixelRasterizer::rasterize` (`apps/hekate-tui/src/pixel.rs:191`) dispatches
  `draw_corridors` → `draw_target_offsets` → `draw_predicted_gaps` →
  `draw_maneuvers` → `draw_wrong_way` after `draw_safety` and before
  `draw_vectors`, the same order as `Overlay` (`crates/hekate-present/src/scene.rs:1040`)
  and as the cell backend's own `Rasterizer::rasterize`
  (`apps/hekate-tui/src/raster.rs:282`); every pass reads only the frame's
  `corridors()`, `target_offsets()`, `predicted_gaps()`, `maneuver_overlays()`,
  and `wrong_way_overlays()`. Pixel test
  `pixel::tests::every_tactical_overlay_is_flag_gated_and_drawn_from_the_frame`
  rasterizes a fixture frame through the same entry point and asserts each
  colour is present with its flag alone on and absent with the flag off, both
  predicted-gap margins, and a Phase 1 frame drawing none of the five.
- **`c/t/p/m/o` affect the Kitty image — met.** `KittyBackend::draw`
  (`apps/hekate-tui/src/kitty.rs:402`) rasterizes through
  `PixelRasterizer::rasterize`, and the five keys apply
  `ViewCommand::ToggleOverlay(Overlay::…)` on the shared session
  (`apps/hekate-tui/src/main.rs:233`–`237`), so each toggle changes the
  transmitted image rather than only the HUD.
- **A TUI key (`e`) requests a wrong-way entry for the selected agent, so the
  wrong-way overlay is reachable in a normal run in both backends — met.**
  `main.rs:241` binds `e` to `TuiSession::request_wrong_way_entry`
  (`apps/hekate-tui/src/session.rs:248`), which requires a selection whose body
  carries route state and forwards to `Simulation::request_wrong_way_entry`.
  The session is the backend-independent half of `BackendPair`, so the same key
  works under `--backend ascii` and `--backend kitty`; the cell backend's
  wrong-way glyph and the pixel backend's wrong-way ring are both gated on
  `overlays.wrong_way`. The footer legend names it
  (`apps/hekate-tui/src/hud.rs:112`, pinned by
  `hud::tests::the_footer_legend_names_every_overlay_toggle`). Tests
  `session::tests::the_wrong_way_trigger_needs_a_selected_agent_with_route_state`
  and `session::tests::the_wrong_way_trigger_opens_the_selected_agents_opposing_traversal`,
  the latter driving `narrow_wrong_way_v2.json5` to corridor `a`'s entry window,
  selecting the rider, requesting the entry, and asserting the inspector reports
  the open opposing traversal.
- **Close-pass evidence enters the shared frame and is rendered in both TUI
  backends — met.** `crates/hekate-present/src/safety.rs:32` carries
  `EventKind::ClosePass` in `is_safety_record`, so a close pass reaches
  `SceneFrame`; both TUI backends read the shared `marker_glyph` (`'P'`) and
  `marker_color` (`CLOSE_PASS_COLOR`) in `apps/hekate-tui/src/raster.rs:117,134`,
  and the Bevy viewer draws it (`apps/hekate-viewer/src/main.rs:1026`). Tests
  `safety::tests::a_close_pass_record_becomes_a_midpoint_safety_marker`,
  `raster::tests::a_close_pass_marker_is_rasterized_and_flag_gated`, and
  `pixel::tests::a_close_pass_marker_is_drawn_and_flag_gated`.
- **The pixel, session, presenter, viewer, and dependency-direction checks pass —
  met.** Gate 3 covers the pixel, session, presenter, and viewer suites (1055
  passed, 0 failed) and gate 4 reports `dependency direction OK`.

No clause is knowingly unmet. Residual risk: no Kitty-capable terminal was
available, so the Kitty path was validated by rasterizing fixture frames through
the same `PixelRasterizer::rasterize` the `KittyBackend` calls rather than
inspecting a live APC transmission.
