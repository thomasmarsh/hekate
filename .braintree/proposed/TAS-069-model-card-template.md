---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Author a reusable model-card template and re-express the two Phase 1 model cards in it.
next: Write the model-card template and migrate the IDM and pedestrian cards to its section order.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

A reusable model-card template states the required sections for a Phase 2 model
— state, sampled parameters, constants, decision inputs, bounds, tie-breaks,
emergency backstop, assumptions, parameter sources, validated ranges, known
failure modes, and incompatible fidelity settings — and both Phase 1 cards
conform to it.

# Done when

- The template is checked in and names every required section from `PHASE_2_PLAN.md`.
- Both Phase 1 model cards conform to the template section order.
- The existing model-card drift test passes against the template inventory.

# Context

No gate; independent documentation deliverable of [[TAS-058-agent-components-and-controller-stages]].
