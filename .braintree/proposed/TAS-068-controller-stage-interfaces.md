---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Split the agent update into four explicit controller-stage interfaces.
next: Expose the four controller stages as explicit interfaces and route the existing vehicle and pedestrian updates through them.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

`tangle-sim` exposes the four `PHASE_2_PLAN.md` controller stages as explicit
interfaces — relevant-world query, tactical choice, motion control, and physical
advance — with the existing vehicle and pedestrian models routed through them
and unchanged behavior.

# Done when

- Each stage has a named interface with immutable observations and a returned command.
- The Phase 1 vehicle and pedestrian updates use the stages with no trace-hash change.
- The controller seam stub-swap test still proves the kernel reaches models only through the interface.

# Context

No gate; independent of the component stream of [[TAS-058-agent-components-and-controller-stages]].
