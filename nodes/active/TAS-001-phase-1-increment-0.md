---
context_rev: 1
priority: P1
updated: 2026-09-12T02:29:21Z
summary: Deliver Phase 1 Increment 0, the pinned workspace and visible walking skeleton.
next: [[TAS-003-scenario-source-parse]]
---

# Outcome

A running Bevy viewer and a headless command both drive the same `tangle-sim`
kernel on one hand-authored JSON5 scenario with a single guide path and a portal
at each end, showing deterministic constant-speed cars.

# Done when

- Running the same seed twice produces the same canonical trace hash.
- Viewer frame rate, pause, and speed controls do not change that hash.
- No Bevy type appears in the public API or dependency tree of `tangle-model`
  or `tangle-sim`.

Area [[IDX-001-tangle]].
