---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Run and baseline manifests record the source schema version, source hash, normalized version-2 hash, and migration version.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

The run manifest and baseline manifest record the source schema version, the
SHA-256 of the source bytes, the SHA-256 of the normalized version-2 document,
and the migration version, so a later trace change is attributable to a source,
a migration, or the kernel.

# Done when

- `RunManifest` and `Baseline` carry source schema version, source content hash, normalized version-2 hash, and migration version.
- Fields are populated from the actual bytes and the migration that produced them, never from a hardcoded default.
- Manifest format versions are bumped and checked-in baselines are regenerated with a versioned explanation.

# Context

Gated on [[TAS-062-version-2-source-shapes]].
Gated on [[TAS-063-v1-to-v2-migration-and-cli]].

# Result

`ScenarioProvenance` now carries all four inputs of a run — source schema
version, source content hash, normalized version-2 hash, and migration version —
and both the run manifest and the checked-in baseline embed it.

- **Shared provenance** (`apps/hekate-cli/src/baseline.rs`): `ScenarioProvenance`
  gained `normalized_sha256` and `migration_version`. Both are `#[serde(default)]`
  so evidence written before the format bump (the checked-in Increment 6
  comparison report and convergence evidence, which embed the struct) still
  reads; every manifest this build writes records them from the bytes it
  consumed. `BASELINE_VERSION` is 2.
- **Run manifest** (`apps/hekate-cli/src/run_dir.rs`): `RUN_MANIFEST_VERSION` is
  2; `RunManifest.scenario` and `RunDirectoryRequest.scenario` carry the shared
  struct unchanged.
- **Version-negotiated loader** (`apps/hekate-cli/src/lib.rs`):
  `load_scenario_provenance` reads the source once, negotiates `schema_version`,
  and returns `(CompiledScenario, ScenarioProvenance)`. A version-1 document
  compiles through the existing direct path (kernel behavior unchanged) and
  records `normalized_sha256` = SHA-256 of `to_canonical_v2_json(migrate_v1_to_v2)`
  and `migration_version` = `MIGRATION_VERSION`; a native version-2 document
  compiles through `compile_v2` and records `normalized_sha256` = SHA-256 of the
  canonical re-serialization and `migration_version` = 0.
  `load_scenario_hashed` is now a thin wrapper. `LoadError` gained
  `UnsupportedSchemaVersion` for a version other than 1 or 2; a v1 parse failure
  keeps the existing `LoadError::Parse` message.
- **Capture and CLI** (`baseline.rs` `CaptureRequest`, `main.rs`, `experiment.rs`):
  `CaptureRequest` gained `normalized_sha256` and `migration_version`; `run`,
  `batch`, `converge`, `baseline`, and `experiment` derive provenance from the
  loader rather than constructing it by hand, so no produced manifest can carry a
  placeholder.
- **Baseline** (`baselines/phase1/baseline.json`, `README.md`): regenerated with
  the documented command. `baseline_version` is 2; `scenario` now records
  `normalized_sha256 = a8d6de85bf24713fa2e2530514770a3c968bdc3a3e87e3b61f5dfdacec9bc522`
  (the SHA-256 of the `migrate` output) and `migration_version = 1`. The README
  gained a "Manifest versions" section explaining the bump and both fields. The
  three preset `trace_sha256` values, run counts, and model/event versions are
  unchanged.
- **Tests**: `tests/baseline.rs` adds `version_1_provenance_covers_the_migration_output`
  (normalized hash equals the SHA-256 of the `migrate` output, migration version
  is `MIGRATION_VERSION`) and `version_2_provenance_records_no_migration`
  (schema version 2, migration version 0, normalized hash equals the canonical
  fixture bytes). `tests/run_directory.rs` asserts the manifest's
  `normalized_sha256` is 64 hex characters and `migration_version` is
  `MIGRATION_VERSION`; the other suites build provenance from the loader.

Evidence: `cargo test -p hekate-cli` passes including
`checked_in_baseline_matches_a_fresh_capture` against the regenerated
`baseline.json`; `cargo test --workspace` passes; `cargo clippy -p hekate-cli
--all-targets` is clean; `cargo fmt --all -- --check` is clean;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`; and
`tangle check` passes with this node still in `proposed/`.

Handoff (outside this leaf's write set): once this node is `resolved/`, the
parent [[TAS-057-schema-v2-migration-and-provenance]] still names it in `next`,
which `tangle check` reports as one expected `next-resolved-node` diagnostic.
The coordinator owns advancing that parent's `next`.
