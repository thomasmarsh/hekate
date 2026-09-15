---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Schema version 2, the deterministic version-1 migrate path, and run-manifest provenance are delivered.
---

Parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]].

# Outcome

Per `PHASE_2_PLAN.md` Increment 0 (schema and provenance): the simulator consumes
only normalized compiled data. This node delivers the version-2 source contract,
a deterministic version-1 `migrate` path, and run-manifest provenance naming the
source schema version, source content hash, normalized version-2 hash, and
migration version.

# Done when

- `SUPPORTED_SCHEMA_VERSION` is 2 and version-2 documents parse, validate, and compile.
- The `migrate` path deterministically rewrites a version-1 document to normalized version 2.
- The run and baseline manifests record source schema version, source content hash, normalized version-2 hash, and migration version.
- Every Phase 1 acceptance scenario and the checked-in baseline pass through the explicit migration path or their original reader.

# Context

Decomposed just in time into direct children; this node stays open until every child is resolved or disposed and the criteria above hold.

# Result

The Increment 0 schema and provenance increment is complete; all four criteria
hold through resolved children, with no child disposed.

- **Version 2 parses, validates, and compiles.**
  [[TAS-061-version-2-schema-contract]] fixed the Increment 0 subset in
  `docs/schema-v2-contract.md`; [[TAS-062-version-2-source-shapes]] set
  `SUPPORTED_SCHEMA_VERSION = 2` and added the version-2 source shapes, version
  negotiation (`parse_scenario_document`, unknown versions rejected with
  `E_SCHEMA_VERSION`), `validate_v2`, and `compile_v2`, with
  `schemas/scenario-source.schema.json` regenerated and the legacy v1 schema
  checked in at `schemas/scenario-source-v1.schema.json`.
- **Deterministic migration.** [[TAS-063-v1-to-v2-migration-and-cli]] added
  `MIGRATION_VERSION = 1`, the pure `migrate_v1_to_v2` transform, the canonical
  normalized serializer, and the `hekate-cli migrate` subcommand, with demand and
  population golden fixtures and a byte-for-byte CLI test.
- **Manifest provenance.** [[TAS-064-migration-provenance]] extended
  `ScenarioProvenance` (shared by `RunManifest` and `Baseline`) with
  `normalized_sha256` and `migration_version`, populated from the actual bytes and
  the transform that ran, and bumped the manifest format versions;
  `baselines/phase1/baseline.json` was regenerated with the new fields, and every
  Phase 1 preset trace hash is unchanged.
- **Phase 1 regression.** [[TAS-065-phase-1-migration-regression]] added
  `apps/hekate-cli/tests/migration_regression.rs`: every checked-in version-1
  scenario migrates and reproduces the original reader's canonical trace body
  byte-for-byte, and the three frozen goldens reproduce through the migration
  path, with the one header `schema_version` 1->2 recorded as a declared,
  versioned correction and pinned by three migrated-trace fixtures. The original
  reader still reproduces the frozen baseline hash.

Evidence: `tangle check` passes (110 nodes); `cargo test --workspace` passes;
`cargo test -p hekate-cli` passes including
`checked_in_baseline_matches_a_fresh_capture` and
`standard_preset_hash_matches_the_golden_trace`; the version-2 behavior and
migration suites in `hekate-model` pass; `scripts/check-dependency-direction.sh`
reports `dependency direction OK`.

Limitation carried to later increments: the `run` command still compiles
version-1 sources through the direct reader, so migrated runs differ from frozen
traces only in the trace header `schema_version`; routing `run` through migration
would require a deliberate, versioned baseline regeneration.
