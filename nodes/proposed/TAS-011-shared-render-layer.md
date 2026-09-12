---
context_rev: 1
priority: P1
updated: 2026-09-12T12:34:01Z
summary: Extract a Bevy-free and terminal-free presentation crate shared by every backend.
next: Create crates/tangle-present with the clock, scenario loading, scene projection, and view commands, then repoint tangle-viewer at it.
---

# Outcome

`crates/tangle-present` owns scenario loading, the playback clock, scene
projection from snapshots, the viewport model, view commands, and selection; the
Bevy viewer consumes it through the contract in
[[DEF-002-renderer-backend-contract]] with unchanged playback behavior.

# Done when

- `crates/tangle-present` has no Bevy, window, wall-clock, or terminal
  dependency, and `tangle-model` and `tangle-sim` do not depend on it.
- `PresentationClock` and the scene projection move out of `tangle-viewer`; the
  existing clock tests move with them and stay green.
- `tangle-viewer` becomes a Bevy backend on the shared contract; its controls,
  interpolation, overlays, and inspector are unchanged for the user.
- The headless trace hash and the viewer/headless equivalence tests still pass.
- `scripts/check-dependency-direction.sh` forbids Bevy and terminal crates from
  `tangle-present`, `tangle-model`, and `tangle-sim`, and CI runs it.

# Context

Depends on [[DEF-002-renderer-backend-contract]] at context_rev 1.
Depends on [[TAS-006-bevy-viewer-skeleton]] at context_rev 1.
This is a behavior-preserving refactor plus the seam every later backend needs;
it should keep the goldens from [[TAS-005-cli-canonical-trace]] unchanged.

Parent [[TAS-009-renderer-backends]].
