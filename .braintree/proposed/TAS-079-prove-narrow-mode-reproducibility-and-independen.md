---
context_rev: 1
updated: 2026-09-13T18:29:33Z
summary: Prove narrow-mode reproducibility and independence from passenger-car dimensions and defaults.
next: Add narrow-mode reproducibility and no-car-defaults gate tests and prove the four increment criteria.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

The Increment 1 verification gates hold as checked evidence: repeated seeded runs
of the narrow fixtures reproduce demand, profiles, decisions, events, and trace
hashes, and each narrow mode passes its independent fixtures without relying on
car-specific dimensions or controller defaults. This leaf adds the reproducible
batch/replay check and the negative guard that fails if shared code substitutes a
car dimension or a car controller default for a sampled narrow value.

# Done when

- A determinism test runs each `narrow_isolated_*` fixture twice at a fixed seed
  and asserts an identical trace hash, and a batch over a declared seed bank
  reproduces the per-seed hashes and event streams.
- A stream-isolation / no-defaults check fails when a narrow agent's body or
  limit comes from a Phase 1 passenger-car constant rather than its compiled
  template, proven with a falsification probe as in
  [[TAS-070-synthetic-template-gate]].
- `cargo test --workspace` passes and every narrow fixture's recorded trace hash
  is stable across the checked-in presets tested.
- The increment's four gate criteria are each traced to a passing test in the
  node result.

# Context

Gated on [[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]],
[[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]], and
[[TAS-078-check-in-narrow-mode-isolated-fixtures-and-tests]]. Per
`PHASE_2_PLAN.md` *Increment 1* gate ("repeated seeded runs reproduce...; each mode
passes independent fixtures without relying on car-specific dimensions or
controller defaults") and *Validation strategy* (deterministic tests). Read
[[TAS-070-synthetic-template-gate]] and [[TAS-059-benchmark-matrix-and-tolerances]].
Owns the determinism and no-car-defaults tests in `crates/tangle-sim/tests/` and
any replay/batch wiring in `apps/tangle-cli` they require.
