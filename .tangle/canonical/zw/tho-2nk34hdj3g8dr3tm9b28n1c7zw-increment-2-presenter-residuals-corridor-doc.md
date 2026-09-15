---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:54Z
summary: Increment 2 presenter residuals: corridor doc drift, pixel overlays, gap half.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 presenter surface (TAS-111,
TAS-136). Verified at commit 98ba8d3.

# Residuals

1. **Corridor-overlay doc comment disagrees with the committed derivation.**
   `CorridorOverlay`'s doc comment
   (`crates/hekate-present/src/tactical.rs:194-195`) says a backend draws the
   segment "from `anchor + left * d_min_m` to `anchor + left * d_max_m`", but
   `corridor_of` (`tactical.rs:669`) derives `d_min_m`/`d_max_m` as absolute
   lateral offsets from the reference
   (`d_max_m = left_extent_m + state.d_m - inset_m`,
   `d_min_m = inset_m + state.d_m - right_extent_m`). Both committed consumers
   therefore draw `anchor + left * (d - offset_m)`
   (`apps/hekate-viewer/src/main.rs:805-806`,
   `apps/hekate-tui/src/raster.rs:711-713`). The comment describes the
   body-relative form and omits the `- offset_m` shift.
2. **The Kitty pixel renderer draws no tactical overlays.** `apps/hekate-tui/src/pixel.rs`
   rasterizes background, geometry, bodies, safety, and vectors
   (`pixel.rs:180-200`); it never reads the frame's tactical overlay set, so
   the Increment 2 route-relative overlays (corridor, target offset, predicted
   gap, maneuver, wrong-way) are absent from the pixel backend. `TacticalOverlay`
   appears in that file only inside test fixtures. The cell grid backend does
   draw them (`apps/hekate-tui/src/raster.rs:296-314`), so pixel/K character
   parity does not hold for the tactical set.
3. **The predicted-gap primitive has no realised half.**
   `TacticalOverlay::observe` ignores `Event::ClosePass`
   (`crates/hekate-present/src/tactical.rs:168`), so `PredictedGapOverlay`
   (`tactical.rs:322`) reflects only the route sample's predicted clearance
   and margin; a realised close pass updates nothing in the overlay, and there
   is no realised-clearance counterpart to the predicted value.

# Seam

`crates/hekate-present/src/tactical.rs`, `apps/hekate-tui/src/pixel.rs`,
`apps/hekate-tui/src/raster.rs`, `apps/hekate-viewer/src/main.rs`.
