---
context_rev: 1
priority: P1
updated: 2026-09-12T01:30:53Z
summary: Add the headless CLI that emits a canonical trace and hashes it.
next: Implement the run command that loads the scenario, steps the kernel, and writes a canonical trace.
---

# Context

Depends on [[TAS-003-scenario-source-parse]] at context_rev 1.
Depends on [[TAS-004-sim-fixed-step-kernel]] at context_rev 1.

# Outcome

`tangle-cli run` produces a deterministic canonical trace from the
walking-skeleton scenario, suitable for golden tests and hash comparison.

# Done when

- Canonical event serialization uses stable field and event ordering.
- Running the same seed twice yields the same trace hash.
- A checked-in golden trace hash fails loudly on unintended serialization or
  behavior change.

Parent [[TAS-001-phase-1-increment-0]].
