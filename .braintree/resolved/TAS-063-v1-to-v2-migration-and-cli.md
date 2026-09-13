---
context_rev: 1
priority: P1
updated: 2026-09-13T15:15:13Z
summary: tangle-model migrates a version-1 source to canonical normalized version 2 and tangle-cli exposes it as `migrate`.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

A pure, deterministic version-1 to version-2 migration transform in
`tangle-model`, exposed through a `tangle-cli migrate` subcommand that emits
normalized version 2 for a version-1 source.

# Done when

- The transform is a pure function of the version-1 document with no filesystem, clock, or random input.
- Golden version-1 to version-2 fixtures cover a demand scenario and a population scenario, and the transform is idempotent on its own output.
- The `migrate` subcommand reads a version-1 document and writes normalized version 2 to a file or stdout, with a documented contract in `--help`.
- A CLI test migrates a checked-in version-1 fixture and compares byte-for-byte with the golden version 2.

# Context

Gated on [[TAS-062-version-2-source-shapes]].

# Result

A version-1 document now rewrites to canonical normalized version 2 through a
pure transform, and `tangle-cli migrate` exposes that transform.

- **Transform** (`crates/tangle-model/src/migrate.rs`): `MIGRATION_VERSION = 1`
  and `migrate_v1_to_v2(&ScenarioSource) -> ScenarioSourceV2`. It reads no
  filesystem, clock, or RNG. It implements the contract's deterministic mapping:
  the version-1 `profiles`/`pedestrian_profiles`/`population` fields become the
  fixed Increment 0 `passenger_car` and `pedestrian` `mode_templates` with the
  resolved profiles materialized (and, when the walking-skeleton population is
  in use, the population's constant body and speed replacing the car template's
  values); every movement gains the `direction` its `from` portal fixes on its
  reference path (`start` → `forward`, `end` → `reverse`); `demand` and
  `pedestrian_demand` become mode-tagged `demand` rate spawns with a whole-run
  unbounded `interval_s`; and the population becomes a `passenger_car`
  population spawn on the scenario's first guide path. Every other version-1
  field is carried forward unchanged and `schema_version` becomes 2.
- **Canonical form** (`to_canonical_v2_json`): 2-space indentation, the
  version-2 field order, and a trailing newline, the same form
  `Baseline::to_pretty_json` writes, so the bytes are exactly what a run's
  normalized hash covers. Both new items are exported from `tangle-model`.
- **Golden fixtures** (`crates/tangle-model/tests/fixtures/migration/`):
  `demand_v1.json5` (vehicle and pedestrian demand plus both movement
  directions on one path) with `demand_v2.json`, and `population_v1.json5` (the
  Phase 1 walking skeleton) with `population_v2.json`.
- **Model tests** (`crates/tangle-model/tests/migration.rs`): the transform is a
  pure function of the document; it is a fixed point of its own canonical
  output for both fixtures; each fixture migrates byte-for-byte to its checked-in
  golden version 2; the normalized documents pass `validate_v2`; movements carry
  the expected directions and demand the expected modes; and
  `compile_v2(migrate(v1))` preserves the walking skeleton's compiled scenario.
- **CLI** (`apps/tangle-cli/src/lib.rs` `migrate_scenario`, `main.rs` `migrate`):
  reads one version-1 document and writes the normalized bytes to `--output` or
  stdout, with the full input/output/exit contract in `--help`. A non-version-1
  input is rejected: version 2 reports that no migration applies, and an
  unsupported version reports the unsupported version. The version-1 direct
  `load_scenario` path is unchanged; provenance wiring stays [[TAS-064-migration-provenance]].
- **CLI test** (`apps/tangle-cli/tests/migrate.rs`): runs the built binary on the
  checked-in demand and population fixtures and compares stdout byte-for-byte
  with the golden version 2, checks `--output` writes those same bytes and no
  other artifact, checks both rejection diagnostics, and checks `--help`
  documents the contract.

Two points the contract left open, resolved in code: the population demand
entry's `id` is the constant `population` (the contract fixes the spawn but not
its id), and the demand fallback when a version-1 document names no `from`
portal is `forward` (validation, not the transform, rejects that document). The
compiled-walking-skeleton test compares every compiled field and asserts the one
expected difference: `compile_v2` records the source's schema version, so
`schema_version()` is 2 for the migrated scenario and 1 for the version-1 one.

Evidence: `cargo test -p tangle-model` (76 tests) passes; `cargo test -p
tangle-cli` passes including the new `migrate` test (7 tests); `cargo test
--workspace` passes; `cargo clippy -p tangle-model -p tangle-cli --all-targets`
is clean; `cargo fmt --all -- --check` is clean;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`;
`tangle-cli migrate --help` documents the contract; and `braintree check` passes
with this node still in `proposed/`.

Handoff (outside this leaf's write set): once this node is `resolved/`, the
parent [[TAS-057-schema-v2-migration-and-provenance]] still names it in `next`,
which `braintree check` reports as one expected `next-resolved-node` diagnostic.
The coordinator owns advancing that parent's `next`.
