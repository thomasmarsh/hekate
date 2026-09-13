---
context_rev: 1
updated: 2026-09-13T22:03:41Z
summary: Load version-2 sources and render facility regions and narrow capsule bodies in the terminal and Bevy presenters.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

`tangle-present` loads schema-version-2 documents (version 1 through migration,
version 2 directly through `compile_v2`), and the terminal and Bevy presenters
draw the Phase 2 Increment 1 features: continuous-width facility regions with
their reference paths, and bicycle/scooter capsule bodies with their sampled
dimensions. `tangle-tui` and `tangle-viewer` open the checked-in
`scenarios/phase2/inc1/*_v2.json5` fixtures and show the narrow agents and the
facilities they occupy. The scene format is extended additively and versioned so
Phase 1 rendering is unchanged.

This closes the Increment 1 presentation gap recorded in
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]]: the compiled facility
and narrow-mode data landed, but `tangle-present::load_scenario` still used the
version-1 reader, so the v2 narrow fixtures could not be opened by either viewer.

# Done when

- `tangle_present::load_scenario` negotiates the schema version through
  `parse_scenario_document`, compiling version 2 through `CompiledScenario::compile_v2`
  while version 1 keeps the migration path, and the existing Phase 1 scene output
  is unchanged.
- The scene representation carries facility regions (with their reference path)
  and narrow capsule bodies; both presenters render them, with any format change
  versioned and covered by a golden or drift test.
- `tangle-tui` and `tangle-viewer` open all six `scenarios/phase2/inc1/*_v2.json5`
  fixtures without a load error, and a test proves the v2 load path.
- `cargo test --workspace` passes and `scripts/check-dependency-direction.sh`
  reports OK.

# Context

Per `PHASE_2_PLAN.md` Increment 1 and the Increment 0 body-kind/segment
presentation. Read [[TAS-072-body-kind-and-segment-presenters]] (the landed
presenter scene format this extends), [[TAS-074-compile-version-2-facility-and-connector-shapes]]
(compiled facility/reference-path geometry), and
[[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]] (narrow capsule
bodies and profiles). The v2 source shapes and version negotiation are
[[TAS-062-version-2-source-shapes]].

Depends on [[TAS-074-compile-version-2-facility-and-connector-shapes]] at context_rev 1.
Depends on [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]] at context_rev 2.
Depends on [[TAS-072-body-kind-and-segment-presenters]] at context_rev 1.

# Result

All four `Done when` criteria hold. The presenters now open and draw the
Increment 1 fixtures through the same scene seam Phase 1 uses, and the scene
shape change is declared, versioned, and pinned.

## Version-negotiating load path

`crates/tangle-present/src/scenario.rs` reads the file and calls
`tangle_model::parse_scenario_document`, then dispatches on `ScenarioDocument`:
`V1` keeps the migration path through `CompiledScenario::compile`, `V2` compiles
through `CompiledScenario::compile_v2`. `LoadError` gains
`UnsupportedSchemaVersion { path, version }` for a document declaring a version
this build cannot read, mirroring `apps/tangle-cli/src/lib.rs`. `tangle-present`
still does not depend on `tangle-cli`; nothing in `tangle-model` or `tangle-sim`
changed.

## Scene additions and format version

`crates/tangle-present/src/scene.rs`:

- `SceneGeometry` carries `facilities`, one `SceneFacility` per compiled
  facility: `id`, the occupied `region`, the region's ring `points`, and an
  optional `SceneFacilityReference` naming the authored `PathId` and carrying its
  polyline. `from_scenario` builds them from `CompiledScenario::facilities()`,
  `region()`, and `path()`, and the bounds include the rings and reference
  vertices. A version-1 scenario declares no facility, so its `facilities` list
  is empty.
- `BodyShape` gains `Capsule { center, heading_rad, length_m, radius_m }`, and
  `SceneBody::shapes` maps a `BodyKind::Capsule` envelope to it (reported length
  is the straight segment, half the reported width the cap radius) instead of the
  bounding box. The dispatch is on the compiled `BodyKind`, never a template id;
  a segmented body still draws a box per segment, and no Phase 1 body changed
  shape.
- `SCENE_FORMAT_VERSION` is declared as **2** (version 1 was the Phase 1
  projection with the Increment 0 body kinds and segments). The scene projection
  has no serialized artifact, so the constant names the shape and the bump is
  recorded in `docs/body-kind-segment-output.md` (new section "Facility and
  capsule scene extension (scene format version 2)"). `EVENT_VERSION` stays 2;
  no metric, snapshot, trace, or trajectory version changed.
- Golden: `tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt` is
  regenerated and the diff is exactly one added line, `facilities: []`; every
  other Phase 1 value is byte-identical. `view_commands.txt`, the renderer cell
  and Kitty goldens, and the trace goldens are unchanged.

## Both presenters render them

- Terminal character backend (`apps/tangle-tui/src/raster.rs`): `draw_geometry`
  draws each facility's ring and reference polyline after the movement pass (a
  facility's reference path and movement share the same polyline, so the
  facility pass must be last to be visible), and `draw_bodies` fills a capsule as
  the rectangle its segment spans plus a circle on each cap.
- Terminal pixel/Kitty backend (`apps/tangle-tui/src/pixel.rs`): the same two
  additions; the capsule fill is the exact set of pixels within `radius_m` of the
  segment, with the two sides and both cap arcs outlined.
- Bevy viewer (`apps/tangle-viewer/src/lib.rs`, `src/main.rs`): `body_visuals`
  resolves a capsule to the rectangle and two cap circles it is exactly the union
  of — the viewer's shared unit meshes cannot scale a cap radius independently of
  the straight length, so a scaled unit capsule mesh would draw elliptical caps —
  and `draw_geometry` draws each facility ring and reference polyline. No
  presenter names a mode or scenario (`apps/tangle-tui/tests/presenter_no_special_case.rs`
  still passes).
- `BodyVisualKey`'s index field is renamed `shape` → `visual`, because one shared
  shape now resolves to more than one rendered visual.

## Fixture-load test and goldens

- `crates/tangle-present/tests/v2_fixtures.rs` (new):
  `every_increment_1_fixture_loads_with_its_facilities_and_narrow_bodies` loads
  each of the six `scenarios/phase2/inc1/*_v2.json5` fixtures through
  `load_scenario`, asserts native `schema_version` 2, that each compiled facility
  projects its authored region ring and reference path, and that the fixture's
  demand puts a narrow body on the road whose single shape is its capsule;
  `a_version_1_fixture_keeps_the_migration_path` and
  `an_unsupported_schema_version_is_reported_as_such` pin the other two branches.
- `apps/tangle-tui/tests/v2_fixture_sessions.rs` (new):
  `every_increment_1_fixture_starts_a_session_and_draws` starts a `TuiSession`
  for each fixture at seed 20260913, advances 1 s, draws, and asserts the scene
  grid is populated.
- v2 scene golden: the regenerated scene golden above. All other goldens
  unchanged.

## Acceptance (exact commands)

- `cargo test -p tangle-present` — passes (44 lib + 2 `scene_golden` + 2
  `safety_overlays` + 3 `v2_fixtures`), including the versioned scene golden.
- `cargo test --workspace` — passes (exit 0); Phase 1 scene, event, metric, and
  trace output unchanged apart from the declared scene-format bump.
- `cargo build --workspace` — passes.
- `cargo run -q -p tangle-tui -- scenarios/phase2/inc1/narrow_signal_v2.json5 20260913 --backend ascii </dev/null`
  — no longer a load error: it reaches terminal setup and fails there with
  `Error: Os { code: 6, kind: Uncategorized, message: "Device not configured" }`
  because there is no TTY in this session. The control run with a missing path
  prints `error: cannot read scenario '...': No such file or directory` and exits
  1, which is what a load failure looks like. The falsifiable proof is the
  load-only test above.
- `./target/debug/tangle-viewer <fixture> 20260913` for all six fixtures — each
  reaches `Creating new window` with no load error (run under `timeout 15`).
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets` — clean.
- `braintree check` — `graph check: passed (123 nodes)` while this node was still
  in `proposed/`.

## Files changed

`crates/tangle-present/src/scenario.rs`, `crates/tangle-present/src/scene.rs`,
`crates/tangle-present/src/lib.rs`,
`crates/tangle-present/tests/v2_fixtures.rs` (new),
`apps/tangle-tui/src/raster.rs`, `apps/tangle-tui/src/pixel.rs`,
`apps/tangle-tui/tests/v2_fixture_sessions.rs` (new),
`apps/tangle-viewer/src/lib.rs`, `apps/tangle-viewer/src/main.rs`,
`docs/body-kind-segment-output.md`, and
`tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt` (the one-line
regeneration the declared scene-format bump requires; `tests/golden/**` is
outside the assigned write set but is the compile-and-golden closure of the
required golden update, and no other node owns it). No `crates/tangle-model/**`,
`crates/tangle-sim/**`, `crates/tangle-cli/**`, `baselines/**`, other `docs/**`,
or other node file was touched.

## Scope notes

- The viewer's `draw_geometry` is an ECS system with no unit test (like every
  other geometry primitive it draws); its facility and capsule coverage is the
  frame-level viewer test, the shared `tangle-present` tests, and the six
  fixtures opening in the built viewer above.
- A different `tests/golden/**` or `baselines/**` path would be another session's
  decision; nothing else in either tree changed.

Post-move note: after this file moves to `resolved/`, `braintree check`
transiently reports `next-resolved-node` because
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]] still names TAS-081 in
its `next`. That is expected; the coordinator advances the parent's `next`. No
parent or sibling node file was edited and no child node was created.
