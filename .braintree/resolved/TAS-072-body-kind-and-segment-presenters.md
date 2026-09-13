---
context_rev: 1
priority: P1
updated: 2026-09-13T18:13:34Z
summary: Render arbitrary body kinds and body segments in the Bevy viewer and terminal presenters.
---

Parent [[TAS-060-body-kinds-and-segments]].

# Outcome

The Bevy viewer and terminal presenters render every represented body kind and
its ordered segments through the shared `tangle-present` scene seam, with no
mode-specific presenter branch.

# Done when

- Both presenters render the Phase 1 box and circle bodies, including segments, from scene data.
- A viewer test or golden demonstrates multi-kind and multi-segment rendering.
- No presenter branch names a scenario or a Phase 2 mode.

# Context

Gated on [[TAS-071-body-kind-and-segment-output]].

# Result

Both presenters now render every represented body kind and its ordered
segments from the shared `tangle-present` scene seam, and no presenter branch
names a scenario or a mode. All three `Done when` criteria hold.

- **Box and circle bodies, and ordered segments, render from scene data.**
  `crates/tangle-present/src/scene.rs` gains `BodyShape` and
  `SceneBody::shapes`, the one backend-independent decision that maps a body's
  reported `body_kind` and ordered segment poses to a list of drawable `Box` or
  `Circle` shapes. A `Circle` body draws its inscribed circle and every other
  kind an oriented box of its reported extent; a body carrying ordered segments
  draws one box per segment at the segment's own pose (a scene segment carries
  only a pose, so the authored length is split evenly across the chain). The
  terminal `raster.rs` and `pixel.rs` `draw_bodies` iterate those shapes; the
  Bevy `apps/tangle-viewer/src/lib.rs` resolves them to shared meshes and
  transforms in `body_visuals`, which `sync_agents` applies one entity per
  shape.
- **A viewer test demonstrates multi-kind and multi-segment rendering.**
  `tangle-viewer`'s `a_body_selects_its_mesh_from_kind_and_draws_a_segment_each`
  (headless; no window or GPU) proves a box body selects a box mesh scaled to its
  extent, a circle body a circle mesh scaled to its diameter, and a two-segment
  body one box entity per segment at the segment's pose. The terminal presenters
  add `each_body_kind_and_ordered_segment_is_rasterized_from_scene_data` and
  `each_body_kind_and_ordered_segment_is_drawn_from_scene_data`, which build a
  synthetic box + circle + two-segment frame: the circle leaves its bounding
  square's corner empty where the box fills it, and both segment centres draw at
  their own poses. `tangle-present`'s
  `body_shapes_follow_kind_and_ordered_segments` pins the shared decision.
- **No presenter branch names a scenario or a mode.** The presenters no longer
  read `body.mode`: the viewer's mesh choice moved from an `AgentMode` match to
  the shared shape list. `apps/tangle-tui/tests/presenter_no_special_case.rs`
  (new) is the TAS-070 source-text guard: it includes `raster.rs`, `pixel.rs`,
  `tangle-viewer/src/lib.rs`, and `tangle-viewer/src/main.rs`, strips each
  `#[cfg(test)]` module, and fails if a shape module names a Phase 2 mode, the
  `AgentMode` type, a body's `.mode`, or the checked-in `walking_guide_v1`
  scenario id, or if the viewer's body systems name a mode.

Additive and byte-invariant: the shared shape decision reproduces the Phase 1
box fill exactly, so no renderer golden changed and none was regenerated.

Evidence: `cargo test --workspace` passes, including the new tests; the cell,
Kitty, and scene goldens are unchanged and still pass;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`;
`cargo clippy --workspace --all-targets --all-features` and
`cargo fmt --all --check` are clean.

Handoff: with this node resolved, `braintree check` reports the expected
`next-resolved-node` diagnostic for [[TAS-060-body-kinds-and-segments]] because
its `next` still names this now-resolved child. TAS-060 stays open for its
remaining criteria and must not be edited from this session.
