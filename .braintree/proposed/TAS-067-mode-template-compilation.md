---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Compile and validate mode templates into agent components, rejecting impossible combinations.
next: Compile mode templates into agent components and reject impossible combinations with stable diagnostics.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

Mode templates compile into validated agent components, and the compiler rejects
impossible combinations such as transit dwell without capacity or articulation
parameters on a holonomic body, with stable diagnostics naming the authored
object.

# Done when

- Compilation of a valid template produces the expected component bundle.
- Invalid combinations fail validation with a stable diagnostic code and the authored template id.
- `cargo test -p tangle-model` passes with golden compiled output for a valid template.

# Context

Gated on [[TAS-066-compiled-agent-component-model]].
