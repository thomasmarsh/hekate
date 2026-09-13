---
context_rev: 2
priority: P1
updated: 2026-09-12T15:18:44Z
summary: Increment 0 captured the versioned Phase 1 baseline, but its gate fails because the Phase 1 definition of done is unmet; behavior-changing Phase 2 work is blocked on Phase 1.
next: Decompose and execute Phase 1 increments 1-6, then re-check baselines/phase1/entry-gate.md.
---

# Blocked

Blocked by: the Phase 1 definition of done (`PHASE_1_PLAN.md`) is not satisfied.
Phase 1 is implemented only through Increment 0 plus the terminal-rendering epic;
increments 1-6 are plan text and no node owns them. `baselines/phase1/entry-gate.md`
records the checklist: five of seven items are Not met and two are Partial.

Unblocks when: Phase 1 increments 1-6 land, every Phase 1 acceptance scenario
passes, and `baselines/phase1/entry-gate.md` reports Satisfied.

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

Captured the versioned Phase 1 baseline for the implemented subset. The new
`tangle-cli baseline` command runs the Fast, Standard, and Fine presets over a
fixed simulated duration and writes a deterministic manifest plus a
non-normative wall-clock report:

- `baselines/phase1/baseline.json` records the scenario content hash, model
  version, frozen `event_version = 1`, per-preset canonical trace hashes and run
  counts, and a convergence statement.
- `baselines/phase1/performance.json` records the point-in-time measurement.
- The Standard preset hash equals `tests/golden/walking_guide_v1.trace.sha256`,
  so the baseline reuses the canonical trace contract instead of a second
  serializer.

Gate confirmation: FAILS. `baselines/phase1/entry-gate.md` maps every Phase 1
definition-of-done item to evidence. The only implemented behavior is
constant-speed cars on one guide path; there are no pedestrians, collisions,
safety metrics, design-variant comparison, or model cards. Committing Phase 2
behavior now would build on an unsupported base.

Evidence:

- `cargo test -p tangle-cli` passes, including
  `checked_in_baseline_matches_a_fresh_capture`,
  `standard_preset_hash_matches_the_golden_trace`, and
  `walking_counts_are_preset_invariant`.
- `baseline.json` records content hash
  `668c4bdc575c648133b93f3a406390579c27f3e56567d500298dbfa7c681ff21`, event
  version 1, and preset-invariant spawn/despawn/remaining counts at 12.5 s.
- `scripts/check-dependency-direction.sh` passes; the baseline command and its
  manifest types live in the `tangle-cli` application crate and add no kernel
  dependency.

Parent [[TAS-017-phase-2-mixed-traffic]].
