---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T10:52:36Z
summary: Map Increment 2 state into presenter overlays with backend parity.
next: Add the new scene fields to the Bevy and terminal test frames, then draw all five route-relative overlays and bind their toggles in both backends.
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
