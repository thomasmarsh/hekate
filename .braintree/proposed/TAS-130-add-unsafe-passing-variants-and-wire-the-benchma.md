---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:43Z
summary: Add unsafe-passing variants and wire the benchmark matrix.
next: Add the unsafe and prohibited passing variants and replace only the planned benchmark-matrix entries.
---

Parent [[TAS-106-check-in-increment-2-passing-fixtures]].

# Outcome

Unsafe, close-clearance, and prohibited-boundary passing variants are checked in
and wired to the benchmark matrix without widening any disposition or tolerance.

# Done when

- Unsafe and prohibited-boundary variants needed by the gate are present and
  assert the documented rejection or violation.
- Tests run every relevant fixture at the matrix presets and preserve Phase 1 and
  Increment 1 behavior when Increment 2 policy is absent.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no disposition or tolerance is widened; schema drift and
  dependency direction pass.

# Context

Extends [[TAS-106-check-in-increment-2-passing-fixtures]]; reads the fixture gates
in [[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the unsafe
variants and the matrix wiring; the core fixtures are the sibling slice.
