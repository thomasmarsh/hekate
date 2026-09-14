---
context_rev: 1
updated: 2026-09-14T15:31:04Z
summary: Order a turned rider's leaders and encounters by its travel direction.
next: Add a focused test that a turned rider's leader query follows its actual travel direction in both reference directions.
---

Parent [[TAS-117-transition-route-and-direction-for-wrong-way-ent]].

# Outcome

A rider that has entered the opposing traversal has its leaders and its
encounters ordered by the direction it actually travels, in both authored
reference directions, so a consumer reading the run sees the same order the
rider experiences.

# Done when

- Focused tests show a turned rider's leader query follows its actual travel
direction rather than the authored nominal one, in both reference directions.
- The within-tick ordering of the turned rider's own records follows the same
travel direction, so no record is ordered by the nominal direction it no
longer travels.

# Context

Extends [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; the
entry, opposing progress, and route completion landed in
[[TAS-117-transition-route-and-direction-for-wrong-way-ent]]. Reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the ordering
evidence for the turned rider's leaders and encounters; the head-on response
(rejection, bounded wait, brake, abort) stays with
[[TAS-118-reject-an-occupied-opposing-corridor-and-keep-th]], and the encounter
event payloads and metric aggregation stay with
[[TAS-099-increment-2-events-metrics-and-output]].
