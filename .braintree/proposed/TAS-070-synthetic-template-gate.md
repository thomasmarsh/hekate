---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Prove a synthetic template changes dimensions, limits, and access without a mode branch.
next: Add the synthetic-template fixture and assert the shared interaction code has no named-mode branch for it.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

The Increment 0 extension gate: a synthetic mode template changes body
dimensions, kinematic limits, and facility access and runs through the shared
interaction code with no new named-mode branch.

# Done when

- A checked-in synthetic template compiles to components and runs at least one scenario.
- A test or review check shows the shared interaction, event, and metric code gained no branch on the synthetic mode name.
- The fixture fails if mode-specific dispatch is reintroduced.

# Context

Gated on [[TAS-067-mode-template-compilation]].
