---
status: resolved
context_rev: 3
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Phase 2 Increment 0 is complete: the Phase 1 baseline is captured, schema version 2 with migration and provenance is delivered, the compiled component model and controller stages are in place, the benchmark matrix is fixed, and body kinds and segments flow through output and presenters.
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

Phase 2 Increment 0 is complete; all five Done-when criteria hold through
resolved children, with no child disposed.

- **Phase 1 acceptance through the reader or migration.**
  [[TAS-057-schema-v2-migration-and-provenance]] delivered schema version 2
  (`SUPPORTED_SCHEMA_VERSION = 2`), the pure version-1 `migrate` transform and
  `hekate-cli migrate`, and manifest provenance for the source and normalized
  hashes plus the migration version. Its gate leaf
  [[TAS-065-phase-1-migration-regression]] then proved every checked-in Phase 1
  scenario migrates and reproduces the original reader's event body
  byte-for-byte.
- **Frozen versions and identified trace changes.** `EVENT_VERSION` stays 2 and
  the baseline's event and metric versions are unchanged. The only intentional
  trace change is the migrated-run header `schema_version` 1->2; it is
  identified with a versioned explanation and three pinned migrated-trace
  fixtures rather than absorbed as golden churn.
- **Minimal regression fixtures.** The three pinned migrated hashes live in
  `apps/hekate-cli/tests/golden/*.migrated.trace.sha256`, and
  `docs/body-kind-segment-output.md` records the TAS-071 format bumps
  (`TRAJECTORY_FORMAT_VERSION` 1->2, `RUN_MANIFEST_VERSION` 2->3). No frozen
  Phase 1 baseline hash was altered.
- **Synthetic template with no mode branch.**
  [[TAS-058-agent-components-and-controller-stages]] delivered the compiled
  component model, mode-template compilation and validation, the four controller
  stages, and the model-card template. Its gate leaf
  [[TAS-070-synthetic-template-gate]] proved a synthetic template alters
  dimensions, limits, and access through the shared stages, with a source-text
  guard that fails if shared code names the synthetic mode.
- **Kernel dependency direction.** `scripts/check-dependency-direction.sh`
  reports `dependency direction OK`: no UI, filesystem, or wall-clock type
  entered `hekate-model` or `hekate-sim`.

Increment 0 also delivered the benchmark matrix
([[TAS-059-benchmark-matrix-and-tolerances]]: `docs/benchmark-matrix.md` with a
machine-readable companion) and the body-kind/segment output contract
([[TAS-060-body-kinds-and-segments]] and its leaves
[[TAS-071-body-kind-and-segment-output]] and
[[TAS-072-body-kind-and-segment-presenters]]).

Evidence: `tangle check` passes (110 nodes); `cargo test --workspace` passes;
the canonical event trace hashes, the Phase 1 goldens, and
`baselines/phase1/**` are unchanged apart from the declared format bumps;
`scripts/check-dependency-direction.sh` passes.

Limitations carried to later increments: the `run` command still compiles
version-1 sources through the direct reader (routing it through migration needs
a deliberate, versioned baseline regeneration); `compile_v2`
materializes only the `passenger_car` and `pedestrian` templates into the
version-1 compiled view, so component-driven spawning is later work; and a
scene body segment carries only a pose, so presenters split the authored body
length evenly across a chain.

Parent [[TAS-017-phase-2-mixed-traffic]].
