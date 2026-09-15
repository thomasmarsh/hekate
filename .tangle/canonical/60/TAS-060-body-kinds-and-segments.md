---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Snapshot, trace, and scene carry body kinds and optional segments, and both presenters render them.
---

Parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]].

# Outcome

Per `PHASE_2_PLAN.md` Increment 0 (output contract): output and presentation
carry arbitrary body kinds and optional ordered body segments, initially
populated by the Phase 1 box and circle bodies, so later modes add no
mode-specific presenter branch.

# Done when

- Snapshot, trace, and scene representations carry a body kind and optional ordered body segments.
- The Bevy viewer and terminal presenters render every represented body kind and its segments.
- Phase 1 vehicle and pedestrian output stays byte-identical apart from the additive fields and declared version bumps.

# Context

Decomposed just in time into direct children; this node stays open until every child is resolved or disposed and the criteria above hold.

# Result

The Increment 0 output contract is complete; all three criteria hold through
resolved children, with no child disposed.

- **Representations.** [[TAS-071-body-kind-and-segment-output]] added
  `body_kind` and an optional ordered segment pose list to the `hekate-sim`
  snapshot observer, the `hekate-cli` trajectory artifact, and the
  `hekate-present` scene. Phase 1 vehicles report `BodyKind::Box` and
  pedestrians `BodyKind::Circle` with empty segment lists.
- **Presenters.** [[TAS-072-body-kind-and-segment-presenters]] added the shared
  `hekate-present::BodyShape` decision and rendered it in the terminal
  raster/pixel presenters and the Bevy viewer, so both draw every body kind and
  each ordered segment from scene data with no scenario or mode branch; a
  source-text guard fails if a shape module reintroduces one.

Evidence: `cargo test --workspace` passes with no golden regenerated except the
scene golden from TAS-071; the canonical event trace hashes and
`baselines/phase1/**` are unchanged; `tangle check` passes (110 nodes);
`scripts/check-dependency-direction.sh` reports `dependency direction OK`.

Declared format bumps (TAS-071, documented in
`docs/body-kind-segment-output.md`): `TRAJECTORY_FORMAT_VERSION` 1->2 and
`RUN_MANIFEST_VERSION` 2->3. `EVENT_VERSION` stayed 2, so Phase 1 event and
metric versions remain frozen.

Limitation carried to later increments: a `BodySegmentSample` carries only a
pose, so the presenter splits the authored body length evenly across a chain;
per-segment dimensions can be added when the sample carries them. Capsule and
segment-less articulated bodies render as their oriented bounding box.
