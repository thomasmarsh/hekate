---
context_rev: 1
status: proposed
updated: 2026-09-15T20:10:01Z
summary: Integrate articulated hitch and trailer motion with segment-level swept collision.
next: Implement deterministic hitch and trailer integration, segment broad and narrow phase with swept queries, and the articulation-limit, off-tracking, and curb-encroachment events, with S-turn and conflict fixtures.
---

Parent [[TAS-021-phase-2-increment-3-heavy-articulated-vehicles]].

# Context

`PHASE_2_PLAN.md` (around line 185) fixes deterministic hitch and trailer integration and makes every segment contribute to the exact and swept body envelope; (around line 213) fixes articulated off-tracking, curb-encroachment, articulation-limit events, and segment-level contacts.

This child follows the articulated geometry and compile child and needs it in place.

# Outcome

A tractor-semitrailer integrates deterministically, every segment contributes to exact and swept collision, and off-tracking, curb-encroachment, and articulation-limit events are recorded, proven by S-turn and conflict fixtures.

# Done when

- Hitch and trailer integration is deterministic and matches analytic or high-resolution reference trajectories within declared tolerances.
- Segment-level broad and narrow phase and swept tests detect contacts by any segment even when no endpoint overlaps.
- Articulation-limit, off-tracking, and curb-encroachment events are reported, with S-turn and conflict fixtures.
- Model-card evidence and the Increment 3 golden suite land with the change.
- The five gates pass.
