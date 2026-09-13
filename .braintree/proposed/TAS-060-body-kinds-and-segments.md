---
context_rev: 1
priority: P1
updated: 2026-09-13T18:01:00Z
summary: Represent arbitrary body kinds and optional body segments in output and presenters.
next: [[TAS-072-body-kind-and-segment-presenters]]
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
