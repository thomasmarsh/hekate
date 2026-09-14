---
context_rev: 1
priority: P1
updated: 2026-09-14T16:29:43Z
summary: Select and execute contextual wrong-way travel through ordinary opposing-flow systems.
---

Parent [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]].

# Outcome

A bicycle or scooter may reproducibly choose a physically connected opposing
traversal when its profile and observed context allow it, then use the same
routing, steering, yielding, collision, and lifecycle paths as nominal travel.

# Done when

- [[TAS-097-make-contextual-wrong-way-decisions-reproducible]] produces a
  stable perceived-rule and reasoned decision from the declared context.
- [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] executes the
  selected opposing route without scripted trajectories or collision bypasses.
- A physically disconnected or occupied opposing corridor is rejected or
  waited for through normal feasibility and gap mechanisms.

# Context

Depends on [[TAS-087-continuous-lateral-motion-and-gap-machinery]] at context_rev 1.
No detailed visibility-error model or sidewalk-specific behavior belongs here.

# Result

Complete. [[TAS-097-make-contextual-wrong-way-decisions-reproducible]] produces
the stable perceived-rule and reasoned decision (with
[[TAS-115-define-contextual-wrong-way-decision-inputs-and]] and
[[TAS-116-key-the-wrong-way-draw-and-reject-impossible-opp]] resolved), and
[[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] executes the
selected opposing route through ordinary routing, steering, collision, yielding,
and lifecycle paths with no scripted trajectory or collision bypass. A physically
disconnected or occupied opposing corridor is rejected or bounded by normal
feasibility and gap mechanisms.
