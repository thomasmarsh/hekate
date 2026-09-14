---
context_rev: 1
updated: 2026-09-14T13:25:49Z
summary: Define contextual wrong-way decision inputs and reason-coded output.
next: Add the wrong-way decision input set, its reason codes, and the perceived-rule output with table-driven tests.
---

Parent [[TAS-097-make-contextual-wrong-way-decisions-reproducible]].

# Outcome

An eligible narrow agent has one documented, reason-coded decision surface for
opposing traversal, built only from the observer-stage context the contract fixes,
so a consumer can read why the agent chose nominal or opposing travel.

# Done when

- The decision reads only the observer-stage context fixed by TAS-083 and adds no
  detailed visibility-error subsystem.
- Output contains perceived permission or obligation, the selected nominal or
  opposing option, reason code, affected movement IDs, and the context values
  used.
- Table-driven tests cover each reason and each threshold tie.

# Context

Extends [[TAS-097-make-contextual-wrong-way-decisions-reproducible]]; reads the
compiled surfaces in [[THO-015-increment-2-compiled-contract-and-model-seams-sc]].
Owns the decision input and output shape; do not add the random draw or motion.
