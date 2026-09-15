# Body-kind and segment output (Increment 0 output contract)

This note is the versioned explanation for the output-format change that adds a
body kind and an optional ordered list of body-segment poses to snapshot,
trajectory, and scene data (`[[TAS-071-body-kind-and-segment-output]]`). It
records what changed, which declared format versions were bumped, and which
goldens were regenerated, so a later reader can attribute a diff to the change
that caused it.

`PHASE_2_PLAN.md` *Safety, operations, and outputs* fixes the intent:
"Trajectory records add mode template, body-segment poses when requested, …".
Increment 0 populates the shape from the Phase 1 box and circle bodies so later
modes add no mode-specific presenter branch.

## What changed

- **Snapshot.** `hekate-sim`'s `MotionSample` now carries `body_kind`
  (`hekate_model::BodyKind`) and `segments` (`Vec<BodySegmentSample>`). A
  `BodySegmentSample` is one segment's world position and heading, ordered front
  to back within its body. `AgentSample` and `MotionSample` are no longer `Copy`,
  because a segment list is not.
- **Scene.** `hekate-present`'s `SceneBody` carries the same `body_kind` and
  `segments`. Segment poses are interpolated from the previous frame by their
  position in the ordered chain; a body without motion detail reads as the
  vehicle box, matching its mode and dimensions fallback.
- **Trajectory artifact.** `hekate-cli`'s `TrajectorySample` carries `body_kind`
  and `segments`. `trajectories.parquet` gains a `body_kind` UTF-8 column after
  `mode` and a `segments` list column of `{x_m, y_m, heading_rad}` structs after
  `speed_mps`. The `TrajectoryArtifact` descriptor gains `format_version`.
- **Phase 1 population.** `AgentMode::body_kind` maps a vehicle to `BodyKind::Box`
  and a pedestrian to `BodyKind::Circle`. A Phase 1 body is a single envelope, so
  its segment list is always empty. The kernel stays free of filesystem, UI, and
  wall-clock types; the new types are plain data.

## Versioned bump

- `RUN_MANIFEST_VERSION` 2 → 3 (`apps/hekate-cli/src/run_dir.rs`). The run
  manifest nests the trajectory descriptor, and that descriptor gained
  `format_version`, so the manifest's serialized shape changed.
- `TRAJECTORY_FORMAT_VERSION` 1 → 2 (`apps/hekate-cli/src/trajectories.rs`, new
  declared constant recorded in the artifact descriptor). Version 1 was the
  seven columns `tick`, `agent`, `mode`, `x_m`, `y_m`, `heading_rad`, and
  `speed_mps`; version 2 adds `body_kind` and the ordered `segments` list.
- `EVENT_VERSION` stays **2** and the canonical event trace stays **frozen**. The
  change is additive to non-event output only: no event, metric, or trace record
  gained a field, and the event-trace goldens and hashes are unchanged. The
  scene projection carries no declared serialized format version, so none was
  bumped there.

## Goldens and evidence

- Regenerated: `tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt`
  (the scene debug dump gains `body_kind: Box` and `segments: []`).
- Unchanged: the canonical trace goldens (`tests/golden/*.trace.jsonl`,
  `*.trace.sha256`), the renderer cell and Kitty goldens
  (`tests/golden/renderer/**`), and `baselines/phase1/**`. Regenerate the
  renderer and scene goldens with `scripts/regen-goldens.sh` and inspect the diff
  before committing it.
- Tests that pin the new output:
  `hekate-sim`'s `a_full_snapshot_reports_each_phase1_body_kind_with_no_segments`,
  `hekate-present`'s `a_body_projects_the_body_kind_and_no_phase1_segments` and
  `segment_poses_interpolate_from_the_previous_frame`, and `hekate-cli`'s
  `the_artifact_reports_each_phase1_body_kind_with_no_segments` and
  `the_artifact_round_trips_body_kind_and_segments`.

# Facility and capsule scene extension (scene format version 2)

This section is the versioned explanation for the second scene-shape change:
`[[TAS-081-presenter-parity-v2-facilities-and-narrow-modes]]` adds the compiled
facility list and the capsule body shape to the shared `hekate-present` scene
projection, so both presenters draw the Phase 2 Increment 1 features from scene
data. It supersedes nothing above; the Increment 0 change stands as recorded.

## What changed

- **Load path.** `hekate_present::load_scenario` now negotiates the schema
  version through `hekate_model::parse_scenario_document`: a version-1 document
  keeps the migration path through `CompiledScenario::compile`, and a
  version-2 document compiles through `CompiledScenario::compile_v2`. A
  document declaring a version this build cannot read is reported as
  `LoadError::UnsupportedSchemaVersion`. The load path mirrors
  `hekate-cli`'s; the scene projection itself is unchanged by it.
- **Scene.** `SceneGeometry` carries `facilities`, one `SceneFacility` per
  compiled facility: its `FacilityId`, the traversable `RegionId` it occupies,
  the region's ring, and, when the facility declares one, a
  `SceneFacilityReference` naming the authored `PathId` and carrying that path's
  polyline. Both presenters draw each facility's ring and reference polyline
  over the region, path, and movement passes; no presenter branches on a mode or
  scenario.
- **Bodies.** `BodyShape` gains `Capsule`, and `SceneBody::shapes` maps a
  `BodyKind::Capsule` envelope to it: the reported length is the straight
  segment and half the reported width the cap radius. The terminal character and
  pixel backends fill the rectangle the segment spans plus a circle at each end;
  the Bevy viewer resolves the capsule to that same rectangle and two cap
  circles, because its shared unit meshes cannot scale a cap radius independently
  of the straight length. A capsule body is a single unsegmented envelope, so the
  ordered-segment path (a box per segment) is unchanged.

## Versioned bump

- `SCENE_FORMAT_VERSION` 1 → 2 (`crates/hekate-present/src/scene.rs`, new
  declared constant). Version 1 was the Phase 1 projection with the Increment 0
  body kinds and ordered segments; version 2 adds `SceneGeometry::facilities`
  and `BodyShape::Capsule`. Nothing serializes a scene, so no artifact records
  the version: it names the projection's shape, and a change to that shape bumps
  it here.
- `EVENT_VERSION` stays **2**, and no metric, trace, snapshot, or trajectory
  format version changes. The scene projection is not part of any artifact
  schema.

## Goldens and evidence

- Regenerated: `tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt`
  — the Phase 1 walking projection gains exactly one line, `facilities: []`,
  because a version-1 scenario has no facilities. Every other line, including
  every body, path, portal, movement, rule, and signal value, is byte-identical.
- Unchanged: `tests/golden/present/view_commands.txt` (it records command
  effects and viewport state, not the geometry dump), the renderer cell and
  Kitty goldens (`tests/golden/renderer/**`), the canonical trace goldens, and
  `baselines/phase1/**`. Regenerate the scene and renderer goldens with
  `scripts/regen-goldens.sh` and inspect the diff before committing it.
- Tests that pin the new output: `hekate-present`'s
  `a_facility_projects_its_region_and_reference_path`,
  `a_capsule_body_draws_its_capsule`,
  `the_declared_scene_format_version_is_the_facility_extension`,
  `a_phase_1_scenario_projects_no_facilities`, and the fixture-load test
  `every_increment_1_fixture_loads_with_its_facilities_and_narrow_bodies`;
  `hekate-tui`'s
  `a_facility_band_and_reference_path_are_rasterized_from_scene_data`,
  `a_facility_band_and_reference_path_are_drawn_from_scene_data`,
  `a_capsule_body_is_rasterized_as_a_capsule`, and
  `a_capsule_body_is_drawn_as_a_capsule`; and `hekate-viewer`'s
  `a_capsule_body_draws_its_straight_part_and_both_caps` and
  `a_version_2_frame_carries_each_facility_band_and_reference_path`.

# Route-relative tactical overlays (scene format version 3)

This section is the versioned explanation for the third scene-shape change:
`[[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]` adds the
Increment 2 route-relative tactical overlays to the shared `hekate-present`
scene projection, so both presenters draw the usable corridor, target offset,
predicted gap, maneuver state, and wrong-way rule state from scene data. It
supersedes nothing above; the Increment 0 and Increment 1 changes stand as
recorded.

## What changed

- **Bodies.** `SceneBody` carries `route_state`, the versioned
  `hekate_sim::RouteStateSample` the snapshot already projects for a steering
  body whose route lies on a compiled facility. It is `None` for a Phase 1 body
  and for an Increment 1 body that carries no route coordinates, exactly as the
  sample itself is absent there.
- **Frame.** `SceneFrame` carries `tactical`, a `TacticalOverlay` folded from the
  typed maneuver and opposing-traversal edge records exactly as `SafetyOverlay`
  folds region occupancy: a maneuver interval opens on the edge that leaves
  `ManeuverState::Following` and closes when an edge returns the agent to it, and
  a wrong-way interval opens on its `entering: true` record and closes on its
  `entering: false` record or on the traversing agent's despawn. The fold keeps
  its intervals ascending by agent id and reads no time or randomness.
- **Overlays.** `SceneFrame` exposes five pure accessors, each a function of the
  frame alone: `corridors`, `target_offsets`, `predicted_gaps`,
  `maneuver_overlays`, and `wrong_way_overlays`. The corridor, target offset, and
  predicted gap primitives are read from each body's route-state sample; the
  maneuver and wrong-way primitives join that sample to the folded interval and
  name only identifiers — agent, partner, facility, movement, clearance, state,
  and reason codes. A body with no route state and a frame with no open interval
  draw nothing, so a Phase 1 or Increment 1 run still projects no tactical
  overlay.
- **Usable corridor derivation.** The corridor is the body's facility band inset
  by the body's half width and its resolved target clearance, measured
  perpendicular to the body's heading; the band is the projected facility whose
  reference path is the body's guide path. The compiled `LateralCorridor` the
  kernel steers within is not projected onto a snapshot, so the frame derives its
  own from the band geometry it carries rather than reading simulation internals;
  for a straight band the two agree wherever the mode's resolved target clearance
  equals its compiled lateral clearance, and the inspector line reports the
  clearance and horizon the derivation used.
- **Inspector text.** `corridor_summary`, `target_offset_summary`,
  `predicted_gap_summary`, `maneuver_summary`, and `wrong_way_summary` join
  `intent_summary`, `profile_summary`, and `decision_summary`, so both backends
  describe the same identifiers with the same wording.
- **Overlay toggles.** `Overlay` gains `Corridor`, `TargetOffset`,
  `PredictedGap`, `Maneuver`, and `WrongWay`, in that declaration order after
  `Geometry`, `Vectors`, and `Safety`; that order is the draw order every backend
  follows. `Overlays` gains the matching flags and toggle arms, all defaulting on
  because each draws nothing on a body without the state.
- **Untouched.** `RouteStateSample`, `MotionSample`, `Snapshot`, `Event`,
  `EventKind`, and `EVENT_VERSION` are read-only here; no simulation or scenario
  branch enters the shared projection.

## Versioned bump

- `SCENE_FORMAT_VERSION` 2 → 3 (`crates/hekate-present/src/scene.rs`). Version 2
  was the facility and capsule projection; version 3 adds
  `SceneBody::route_state`, `SceneFrame::tactical`, the five overlay accessors
  and their inspector text, and the new overlay toggles. Nothing serializes a
  scene, so no artifact records the version: it names the projection's shape, and
  a change to that shape bumps it here.
- `EVENT_VERSION` stays **3**, and no metric, trace, snapshot, trajectory, or
  manifest format version changes. The scene projection is not part of any
  artifact schema, and no Increment 0 or Increment 1 projection value changed.

## Goldens and evidence

- Regenerated: `tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt`
  — the Phase 1 walking projection gains exactly the declared additive fields:
  `route_state: None` on each of its six bodies, the five new overlay flags, and
  an empty `tactical` fold. Every other line, including every body, path, portal,
  movement, rule, signal, and safety value, is byte-identical.
- Unchanged: `tests/golden/present/view_commands.txt` (it records command effects
  and viewport state, not the geometry dump), the renderer cell and Kitty goldens
  (`tests/golden/renderer/**`), the canonical trace goldens, and
  `baselines/phase1/**`.
- The version-pinning test `the_declared_scene_format_version_is_the_facility_extension`
  is renamed to `the_declared_scene_format_version_is_the_tactical_extension`,
  because it now pins version 3.
- Tests that pin the new output: `hekate-present`'s
  `the_declared_scene_format_version_is_the_tactical_extension`,
  `the_fold_opens_and_closes_maneuver_and_wrong_way_intervals`,
  `a_maneuver_overlay_names_its_state_tactic_edge_and_reason`,
  `a_corridor_is_the_band_inset_by_the_body_and_its_clearance`,
  `a_target_offset_and_predicted_gap_project_from_the_route_sample`,
  `a_wrong_way_overlay_names_its_open_interval_and_live_rule`,
  `a_body_with_no_route_state_carries_no_tactical_overlay`,
  `the_inspector_summaries_name_every_identifier`, and
  `every_tactical_overlay_flag_toggles_independently`.
