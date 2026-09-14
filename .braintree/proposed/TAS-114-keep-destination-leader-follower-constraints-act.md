---
context_rev: 1
updated: 2026-09-14T13:25:04Z
summary: Keep destination leader/follower constraints active through a handoff.
next: Select leaders and followers from the current and destination facility before and during a handoff, with tests.
---

Parent [[TAS-095-complete-lane-transitions-and-safe-aborts]].

# Outcome

A committed facility transition keeps both the current and the destination
facility leader and follower constraints active through the handoff, so no
constraint gap opens across the ownership change and the ordinary spatial index
and collision scan see every intermediate world pose.

# Done when

- Leader and follower selection reads the destination facility before the handoff
  as well as the current one, so a body on either side constrains the maneuver.
- The per-tick spatial rebuild and collision scan are unchanged and see every
  intermediate pose; no transition creates an unconstrained step.
- Focused tests cover a leader and a follower on each side of the shared boundary
  during a committed change of lane.

# Context

Depends on [[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]] at context_rev 1.
Reads the sim seams in [[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]].
Owns the sim-side leader and follower constraint selection; do not change the
authored schema or the maneuver lifecycle.
