---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Define compiled agent components: body, motion, tactics, access, occupancy, social state.
next: Define the compiled component structs and the optional/variant composition the kernel dispatches on.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

`tangle-model` compiled data carries each simulated road user as a compact core
plus composable optional components: body (circle, oriented box, capsule, or
ordered articulated chain), motion (holonomic walking, single-body wheeled, or
articulated wheeled), tactical capabilities, access, occupancy, and social
state.

# Done when

- The compiled representation defines each component from the agent-composition section of `PHASE_2_PLAN.md`.
- Components compose without a mode name in the kernel and dispatch on a small number of body/motion families.
- `cargo test -p tangle-model` passes with the existing Phase 1 compilation unchanged.

# Context

No gate; starts the component stream of [[TAS-058-agent-components-and-controller-stages]].
