---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: A required golden regeneration can sit outside the handed-off exclusive write set
tangle_revision: 0.5.0+g974b178
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Slice-E worker for TAS-030: carry the agent mode through the projection and add viewer safety overlays. The handoff named an exclusive write set of crates/hekate-present/, apps/hekate-viewer/, apps/hekate-tui/, apps/hekate-cli/ if needed, and the assigned node, and in the same breath required regenerating any changed golden scene output.
Friction: The handoff required a deliberate golden regeneration but omitted the golden path from the write set: tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt lives outside every crate directory, and the presentation golden is a Debug dump of the whole frame, so any new field on SceneBody/SceneFrame invalidates it and the change cannot be handed off without editing that file. This is the second instance of FBK-008 (exclusive write set narrower than the compile-and-golden closure); recorded as additional evidence for the accumulating meta-analysis rather than as a new finding, and the worker widened the set itself and reported the addition.
Improvement: Adopt FBK-008 further: the handoff should derive its write set from the change closure (grep the crate for struct literals and exhaustive matches on the changed types, plus the paths of every golden and baseline the change can invalidate, including tests/golden/** and baselines/**). For this repo specifically, the presentation golden is a whole-frame Debug dump, so it is a closure member of any hekate-present public-field change and belongs in the write set by default.
