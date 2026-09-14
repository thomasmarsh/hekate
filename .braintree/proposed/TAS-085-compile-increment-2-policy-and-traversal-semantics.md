---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Compile Increment 2 policies into resolved corridors, permissions, and traversal options.
next: Compile authored Increment 2 policy into identifier-resolved immutable model data.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

The immutable compiled scenario exposes component-driven lateral policy,
clearance definitions, adjacency, and opposing traversal facts that simulation
can consume without source-string lookup or mode-name branching.

# Done when

- Compiled policy resolves mode template, facility, connector, permission, and
  clearance-band IDs once, preserves stable source order, and exposes focused
  accessors used by the controller stages.
- For each eligible body and route position it distinguishes the usable
  interval, nominal and permitted directions, physically connected traversal
  directions, transition targets, and applicable pass/line-crossing policy.
- A missing optional Increment 2 policy compiles to the documented
  no-free-lateral-motion Increment 1 behavior.
- Unit tests cover forward/reverse/either facilities, adjacent connectors,
  permissions and prohibitions, multiple clearance bands, and stable ordering.
- No simulation, event, metric, or presentation behavior is introduced.

# Context

Gated on [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]].
Owns crates/tangle-model/src/compiled.rs, components.rs, mode_template.rs, and
focused model compilation tests. Reuse CompiledReferencePath and
CompiledFacility geometry from TAS-074; do not add a navigation mesh.
