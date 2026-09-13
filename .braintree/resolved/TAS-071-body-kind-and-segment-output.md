---
context_rev: 1
priority: P1
updated: 2026-09-13T17:56:43Z
summary: Add body kind and optional ordered body segments to snapshot, trace, and scene representations.
---

Parent [[TAS-060-body-kinds-and-segments]].

# Outcome

Snapshot, trace, and `tangle-present` scene data carry a body kind and an
optional ordered list of body segments, populated by the Phase 1 box and circle
bodies.

# Done when

- The snapshot, trace, and scene types carry a body kind and optional ordered segment list with segment poses.
- Phase 1 vehicle and pedestrian output populates the new fields and existing consumers keep working.
- Declared format versions are bumped and goldens regenerated with a versioned explanation.

# Context

No gate; first frontier action of [[TAS-060-body-kinds-and-segments]].

# Result

Landed the additive body-kind and segment fields across the snapshot, the
sampled-trajectory artifact, and the scene projection. Phase 1 output populates
them: a vehicle is a box and a pedestrian a circle, each a single envelope with
an empty segment list. Every existing consumer still compiles and renders; the
viewer and TUI ignore the new fields for now.

## What landed

- `crates/tangle-sim/src/snapshot.rs`: `MotionSample` gains
  `body_kind: tangle_model::BodyKind` and `segments: Vec<BodySegmentSample>`;
  new `BodySegmentSample` carries one segment's world position and heading.
  `MotionSample` and `AgentSample` drop `Copy` (a segment list is not `Copy`).
  `crates/tangle-sim/src/agent.rs`: `AgentMode::body_kind` maps a vehicle to
  `BodyKind::Box` and a pedestrian to `BodyKind::Circle`; `sim.rs` populates
  both fields from the agent store.
- `crates/tangle-present/src/scene.rs`: `SceneBody` gains `body_kind` and
  `segments` (dropping `Copy`); `SceneBody::project` carries the kind and
  interpolates segment poses by chain position.
- `apps/tangle-cli/src/trajectories.rs`: `TrajectorySample` gains `body_kind`
  and `segments`; `trajectories.parquet` gains a `body_kind` column and a
  `segments` list column of `{x_m, y_m, heading_rad}` structs;
  `TrajectoryArtifact` gains `format_version`.
- New versioned explanation `docs/body-kind-segment-output.md`; module doc
  notes on `TRAJECTORY_FORMAT_VERSION` and `RUN_MANIFEST_VERSION`.

## Versioned bump

- `RUN_MANIFEST_VERSION` 2 -> 3 (the nested trajectory descriptor grew a
  field).
- `TRAJECTORY_FORMAT_VERSION` 1 -> 2 (added `body_kind` and `segments`).
- `EVENT_VERSION` stays 2 and the canonical event trace stays frozen; no event
  or metric record changed. The scene projection has no declared serialized
  format version, so none was bumped.

## Evidence

- `cargo test --workspace` passes (62 suites, 0 failures), including the new
  `tangle-sim` `a_full_snapshot_reports_each_phase1_body_kind_with_no_segments`,
  `tangle-present` `a_body_projects_the_body_kind_and_no_phase1_segments`,
  `segment_poses_interpolate_from_the_previous_frame`, `tangle-cli`
  `the_artifact_reports_each_phase1_body_kind_with_no_segments` and
  `the_artifact_round_trips_body_kind_and_segments`.
- `scripts/check-dependency-direction.sh` passes; `cargo clippy --workspace
  --all-targets` and `cargo fmt --all --check` are clean.
- Regenerated only `tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt`
  (adds `body_kind: Box` and `segments: []`). The canonical trace goldens and
  hashes and `baselines/phase1/**` are unchanged.

## Handoff

Moving this node to `resolved/` leaves parent [[TAS-060-body-kinds-and-segments]]
with a `next-resolved-node` diagnostic pointing at this node; the parent should
advance its `next` to [[TAS-072-body-kind-and-segment-presenters]] without
editing TAS-071. Presenter rendering of body kinds and segments is TAS-072's
scope and was left out here.
