---
context_rev: 3
priority: P1
updated: 2026-09-13T14:25:11Z
summary: The Phase 1 entry gate is satisfied, unblocking Phase 2; Increment 0 still owes schema version 2 with migration and provenance, the compiled agent components with mode templates and a model-card template, the benchmark matrix, and arbitrary-body-kind representation.
next: Design schema version 2, the deterministic version-1 migrate command, and run-manifest provenance.
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

# Result

The Phase 1 prerequisite is cleared and this node moves from `blocked` to
`proposed`; `context_rev` is bumped because the gate verdict is
decision-relevant context for every downstream increment.

- `baselines/phase1/entry-gate.md` now reports **SATISFIED** (2026-09-13). All
  seven Phase 1 definition-of-done items map to committed evidence, and Phase 1
  increments 1-6 are resolved as TAS-026 through TAS-043 and TAS-045 through
  TAS-050 ([[TAS-026-phase-1-increment-1-general-scenario-foundation]],
  [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]],
  [[TAS-050-increment-6-replay-and-live-view]]). The earlier 2026-09-12
  verdict of NOT SATISFIED is superseded.

Increment 0 is **not** complete. Delivered so far:

- `baselines/phase1/baseline.json` and `baselines/phase1/performance.json`,
  produced by the `tangle-cli baseline` command, capture the versioned Phase 1
  trace, metric, convergence, and performance baseline for the Fast, Standard,
  and Fine presets. `baseline.json` records content hash
  `668c4bdc575c648133b93f3a406390579c27f3e56567d500298dbfa7c681ff21`, frozen
  `event_version = 1`, per-preset canonical trace hashes and run counts, and
  preset-invariant spawn/despawn/remaining counts at 12.5 s.

Still owed, each sized as its own session (§ Increment 0 of `PHASE_2_PLAN.md`):

1. Schema version 2, a deterministic version-1 `migrate` command, and run
   manifest provenance (source schema version, source content hash, normalized
   version-2 hash, migration version). `SUPPORTED_SCHEMA_VERSION` is still 1 and
   the CLI has no `migrate` subcommand.
2. Compiled agent components (body, motion, tactical capability, access,
   occupancy, social state), mode-template validation, controller-stage
   interfaces, and a model-card template. The compiled profiles, the
   `crates/tangle-sim/src/controller.rs` seam, and the vehicle and pedestrian
   model cards exist, but there is no template layer.
3. Benchmark matrix and quantitative tolerances for independent, pairwise, and
   mixed-mode validation, building on the existing fidelity presets.
4. Viewer and output representation for arbitrary body kinds and optional body
   segments; the presenters handle box and circle only.

Evidence: `cargo test -p tangle-cli` passes, including
`checked_in_baseline_matches_a_fresh_capture`,
`standard_preset_hash_matches_the_golden_trace`, and
`walking_counts_are_preset_invariant`; `scripts/check-dependency-direction.sh`
passes, and the baseline command and its manifest types remain in the
`tangle-cli` application crate and add no kernel dependency.

Parent [[TAS-017-phase-2-mixed-traffic]].
