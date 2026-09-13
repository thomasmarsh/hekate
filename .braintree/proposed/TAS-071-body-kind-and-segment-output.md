---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Add body kind and optional ordered body segments to snapshot, trace, and scene representations.
next: Add body kind and optional ordered body segments to the snapshot, trace, and scene types.
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
