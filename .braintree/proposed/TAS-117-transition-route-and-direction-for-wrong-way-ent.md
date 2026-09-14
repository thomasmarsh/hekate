---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Transition route and direction for wrong-way entry and completion.
next: Move route state to the opposing traversal through the connected-transition contract and keep progress correct in both reference directions.
---

Parent [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].

# Outcome

A wrong-way agent enters the connected opposing traversal through the ordinary
transition contract and makes correct progress and completion in both authored
reference directions.

# Done when

- Route state changes direction and facility through the same connected-transition
  contract as an ordinary lateral maneuver, with the world pose continuously
  authoritative.
- Opposing leaders and encounters are ordered in the actual travel direction, and
  route progress and completion work in both authored reference directions.
- Focused tests cover clear opposing entry and route completion.

# Context

Gated on [[TAS-097-make-contextual-wrong-way-decisions-reproducible]].
Extends [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; reads
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the route and
direction transition; do not disable collision or add a scripted trajectory.
