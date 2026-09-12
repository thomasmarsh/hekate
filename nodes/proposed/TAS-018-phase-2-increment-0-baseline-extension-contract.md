---
context_rev: 1
priority: P1
updated: 2026-09-12T15:12:03Z
summary: Increment 0 captures the Phase 1 baseline and lands the schema-v2 migration and compiled-component extension contract.
next: Capture the versioned Phase 1 baseline and confirm the entry gate before any behavior change.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 0:

- Captured Phase 1 trace, metric, convergence, and performance baseline:
  manifests, trace hashes, summaries, convergence report, and performance
  measurements.
- Schema-version-2 design, deterministic version-1 `migrate` command, and run
  manifest provenance for source schema version, source content hash,
  normalized version-2 hash, and migration version.
- Compiled agent components (body, motion, tactical capability, access,
  occupancy, social state), mode-template validation, controller-stage
  interfaces, and a model-card template.
- Benchmark matrix and quantitative tolerances for independent, pairwise, and
  mixed-mode validation, including fidelity presets.

# Done when

- All Phase 1 acceptance scenarios pass through either their original reader or
  the explicit migration path.
- Phase 1 event and metric versions are frozen in the baseline manifest, and
  every model change that intentionally invalidates a trace is identified
  rather than absorbed as golden-file churn.
- Any intentional baseline change carries a minimal regression fixture and a
  versioned explanation.
- A synthetic template can alter dimensions, limits, and access without adding
  a named-mode branch to shared interaction code.
- The dependency-direction check still prevents UI, filesystem, and wall-clock
  types from entering the kernel.

Parent [[TAS-017-phase-2-mixed-traffic]].
