---
context_rev: 1
priority: P1
updated: 2026-09-14T00:58:15Z
summary: Fix and implement the Increment 2 authored and compiled policy contract.
next: [[TAS-085-compile-increment-2-policy-and-traversal-semantics]]
---

Parent [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]].

# Outcome

Version 2 has one documented, parsed, compiled, and validated Increment 2
surface for lateral tactics, permissions, clearance bands, maneuver policy, and
contextual opposing traversal. Consumers can distinguish the additive surface
without inferring semantics from mode names.

# Done when

- [[TAS-083-define-the-increment-2-schema-and-semantics]] fixes exact fields,
  units, defaults, state meanings, event evidence, and deferral boundaries.
- [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]] parses and
  regenerates the checked schema for the new authored shapes.
- [[TAS-085-compile-increment-2-policy-and-traversal-semantics]] compiles them
  into identifier-resolved policy and physically possible traversal data.
- [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] rejects every
  malformed or internally impossible combination with stable diagnostics.
- All four children are resolved or deliberately disposed, the schema drift
  gate and model tests pass, and the result maps each downstream consumer to
  the compiled field it reads.

# Context

Extends the resolved Increment 1 contract in place; it does not reinterpret an
existing version-2 field. This coordinating node owns ledger roll-up only.
