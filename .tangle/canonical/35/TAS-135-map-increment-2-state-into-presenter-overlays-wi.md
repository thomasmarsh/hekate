---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T11:13:30Z
summary: Map Increment 2 state into presenter overlays with backend parity.
---

Parent [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]].

# Outcome

The shared scene and both viewer backends can inspect an Increment 2 run usable
corridor, target offset, predicted gap, maneuver state, and wrong-way rule state
without simulation or scenario-specific branches.

# Done when

- `hekate-present` maps the versioned snapshot and event state into backend-neutral
  overlay primitives and inspector text with agent, partner, facility or movement,
  clearance, state, and reason identifiers.
- Bevy and terminal backends expose all five required overlays with deterministic
  draw and order behavior and graceful absence for Phase 1 and Increment 1 runs.

# Context

Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]; reads the
presenter and gate seams in
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the mapping and
backend parity; fixtures and the negative guard are the sibling slice.

# Result

Shared presenter layer landed (`crates/hekate-present` only). `SceneBody` carries
`route_state: Option<RouteStateSample>`; new `tactical` module folds maneuver and
wrong-way intervals from the typed edge records exactly as `SafetyOverlay` folds
occupancy (open on the leaving/entering edge, close on the returning/leaving edge
or despawn), and `SceneFrame` exposes the five pure accessors `corridors`,
`target_offsets`, `predicted_gaps`, `maneuver_overlays`, `wrong_way_overlays`
plus the `corridor_summary`/`target_offset_summary`/`predicted_gap_summary`/
`maneuver_summary`/`wrong_way_summary` inspector functions. `Overlay` gains
`Corridor`, `TargetOffset`, `PredictedGap`, `Maneuver`, `WrongWay` and `Overlays`
the matching flags and toggle arms, all defaulting on. `SCENE_FORMAT_VERSION`
bumped 2 -> 3 with the rationale declared in `docs/body-kind-segment-output.md`;
`tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt` regenerated and the
diff is only the six `route_state: None` lines, the five overlay flags, and the
empty tactical fold. `cargo test -p hekate-present` (52 unit + 7 integration
tests), the three named integration tests, `scripts/check-dependency-direction.sh`,
and `cargo build --workspace` pass. Backends are not yet updated, so the two apps'
test and example frames still fail to compile (`cargo test -p hekate-tui`/`-p
hekate-viewer`): the shared write set excludes `apps/`.

Both backends now consume that frame (`apps/hekate-viewer/**`, `apps/hekate-tui/**` only).
Bevy: `c`/`t`/`p`/`m`/`o` bind `Overlay::{Corridor, TargetOffset, PredictedGap, Maneuver,
WrongWay}`; one pure shape-derivation function per overlay (`corridor_shapes`,
`target_offset_shapes`, `predicted_gap_shapes`, `maneuver_shapes`, `wrong_way_shapes`), each
gated on its own `frame.overlays` flag, is drawn in `Overlay` declaration order by a new
`draw_tactical_overlays` system registered directly after `draw_safety_overlays`; the internal
`SafetyOverlayShape` is generalized to `OverlayShape` with a `Segment` arm, and `describe_agent`
appends the five shared summaries for the selected body. Terminal: the same five keys, the
footer legend names them, `marker_glyph`/`marker_color`-style functions cover the maneuver
state, the wrong-way violation, and the predicted-gap margin, `Rasterizer::rasterize` gains
`draw_corridors`/`draw_target_offsets`/`draw_predicted_gaps`/`draw_maneuvers`/`draw_wrong_way`
branches after `draw_safety`, and `Hud::describe` appends the same five summaries. Both read
only `SceneFrame` and branch on nothing but that frame's data. Graceful absence: every
primitive is Option-gated and each fold is empty without its records, so Phase 1 and Increment 1
frames draw nothing and their inspectors are unchanged (the five new summaries appear only for a
body that carries route state).

Two mapping notes. (1) The corridor segment is laid out from the body's own offset,
`anchor + left * (d - offset_m)`, because `CorridorOverlay`'s interval is absolute in the body's
travel frame (`tactical.rs` test `a_corridor_is_the_band_inset_by_the_body_and_its_clearance`
pins that for a body at `d_m = 0.5`); the committed doc comment on the struct states the
anchor-only form, which holds only at `offset_m == 0`, so a one-line doc fix is owed in
`crates/hekate-present/src/tactical.rs` (outside this slice's write set). (2) The Kitty pixel
renderer `apps/hekate-tui/src/pixel.rs` draws the safety overlay but not the tactical ones; the
character-cell `Rasterizer` is the terminal renderer this slice wired per the brief, so the
pixel path remains scope.

Validation: `cargo fmt --all`; `cargo test -p hekate-viewer` 9 pass; `cargo test -p hekate-tui`
98 pass (81 lib + 17 integration); `cargo test -p hekate-present` 59 pass; `cargo clippy -p
hekate-viewer -p hekate-tui --all-targets` clean; `scripts/check-dependency-direction.sh` OK.
No golden changed: the cell golden is a Phase 1 run with no tactical data, and the kitty golden
truncates the footer row to its 8-column test width, so the new legend keys never reach it.

## Five-gate validation (HEAD `94a19d9`)

| # | Gate | Exit | Wall | Result |
|---|------|------|------|--------|
| 1 | `cargo fmt --all --check` | 0 | 1s | green |
| 2 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 13s | green |
| 3 | `cargo test --workspace --all-features` | 0 | 320s | green: 93 binaries, 1037 passed, 0 failed, 3 ignored |
| 4 | `./scripts/check-dependency-direction.sh` | 0 | 1s | green: `dependency direction OK` |
| 5 | `tangle check` | 0 | 0s | green: `graph check: passed (195 nodes)` |

Done-when disposition (verified against the code, not the handoffs):

- Presenter mapping with agent, partner, facility/movement, clearance, state, and
  reason identifiers — met: `crates/hekate-present/src/tactical.rs` defines the
  five `get`-identified overlay primitives and the `*_summary` inspector
  functions; `SCENE_FORMAT_VERSION == 3`.
- Both backends expose all five with deterministic declaration order — met:
  viewer `draw_tactical_overlays` chains `corridor_shapes`→`target_offset_shapes`
  →`predicted_gap_shapes`→`maneuver_shapes`→`wrong_way_shapes`; terminal
  `Rasterizer::rasterize` calls `draw_corridors`→`draw_target_offsets`
  →`draw_predicted_gaps`→`draw_maneuvers`→`draw_wrong_way` after `draw_safety`,
  both matching `Overlay` declaration order.
- Graceful absence for Phase 1/Increment 1 — met: every primitive is `Option`
  gated and each fold is empty without its records (Phase 1 presenter golden has
  `route_state: None` and an empty `tactical` fold); guarded by
  `every_route_relative_overlay_is_flag_gated_and_absent_for_phase_1` and
  `no_tactical_overlay_draws_without_route_state_or_an_open_interval`.
- Version bump with rationale and regenerated goldens — met:
  `SCENE_FORMAT_VERSION` 2 → 3 with the rationale in
  `docs/body-kind-segment-output.md`; `tests/golden/present/
  walking_guide_v1.seed0.tick20.scene.txt` regenerated with only the declared
  additive diff.
- No sim/scenario branch in shared modules — met:
  `apps/hekate-tui/tests/presenter_no_special_case.rs`
  (`no_shape_module_names_a_scenario_or_mode`) passes; `crates/hekate-present`
  production code has no scenario/mode branch.

Deferred (recorded, not blocking):

- Kitty pixel renderer `apps/hekate-tui/src/pixel.rs` draws the safety overlay but
  not the tactical ones; the default character-cell `Rasterizer` is the terminal
  backend in scope. Parent pre-adjudicated this as a deferred follow-up.
- One-line `CorridorOverlay` doc-comment inconsistency in
  `crates/hekate-present/src/tactical.rs` (doc says `anchor + left*d`; committed
  derivation and both backends use `anchor + left*(d - offset_m)`). Parent
  pre-adjudicated this as a deferred doc nit.
