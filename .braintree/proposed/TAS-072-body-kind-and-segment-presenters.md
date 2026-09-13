---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Render arbitrary body kinds and body segments in the Bevy viewer and terminal presenters.
next: Render every represented body kind and its segments in the Bevy viewer and terminal presenters.
---

Parent [[TAS-060-body-kinds-and-segments]].

# Outcome

The Bevy viewer and terminal presenters render every represented body kind and
its ordered segments through the shared `tangle-present` scene seam, with no
mode-specific presenter branch.

# Done when

- Both presenters render the Phase 1 box and circle bodies, including segments, from scene data.
- A viewer test or golden demonstrates multi-kind and multi-segment rendering.
- No presenter branch names a scenario or a Phase 2 mode.

# Context

Gated on [[TAS-071-body-kind-and-segment-output]].
