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

- **Snapshot.** `tangle-sim`'s `MotionSample` now carries `body_kind`
  (`tangle_model::BodyKind`) and `segments` (`Vec<BodySegmentSample>`). A
  `BodySegmentSample` is one segment's world position and heading, ordered front
  to back within its body. `AgentSample` and `MotionSample` are no longer `Copy`,
  because a segment list is not.
- **Scene.** `tangle-present`'s `SceneBody` carries the same `body_kind` and
  `segments`. Segment poses are interpolated from the previous frame by their
  position in the ordered chain; a body without motion detail reads as the
  vehicle box, matching its mode and dimensions fallback.
- **Trajectory artifact.** `tangle-cli`'s `TrajectorySample` carries `body_kind`
  and `segments`. `trajectories.parquet` gains a `body_kind` UTF-8 column after
  `mode` and a `segments` list column of `{x_m, y_m, heading_rad}` structs after
  `speed_mps`. The `TrajectoryArtifact` descriptor gains `format_version`.
- **Phase 1 population.** `AgentMode::body_kind` maps a vehicle to `BodyKind::Box`
  and a pedestrian to `BodyKind::Circle`. A Phase 1 body is a single envelope, so
  its segment list is always empty. The kernel stays free of filesystem, UI, and
  wall-clock types; the new types are plain data.

## Versioned bump

- `RUN_MANIFEST_VERSION` 2 → 3 (`apps/tangle-cli/src/run_dir.rs`). The run
  manifest nests the trajectory descriptor, and that descriptor gained
  `format_version`, so the manifest's serialized shape changed.
- `TRAJECTORY_FORMAT_VERSION` 1 → 2 (`apps/tangle-cli/src/trajectories.rs`, new
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
  `tangle-sim`'s `a_full_snapshot_reports_each_phase1_body_kind_with_no_segments`,
  `tangle-present`'s `a_body_projects_the_body_kind_and_no_phase1_segments` and
  `segment_poses_interpolate_from_the_previous_frame`, and `tangle-cli`'s
  `the_artifact_reports_each_phase1_body_kind_with_no_segments` and
  `the_artifact_round_trips_body_kind_and_segments`.
