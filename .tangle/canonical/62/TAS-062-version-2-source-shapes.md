---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: hekate-model parses, validates, and compiles the Increment 0 version-2 subset; SUPPORTED_SCHEMA_VERSION is 2.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

`hekate-model` parses, validates, and compiles the Increment 0 subset of schema
version 2 defined by [[TAS-061-version-2-schema-contract]], and
`SUPPORTED_SCHEMA_VERSION` is 2.

# Done when

- The version-2 source structs from the contract exist with schema generation and a regenerated `schemas/scenario-source.schema.json`.
- Version negotiation accepts version 2, rejects unknown versions with a stable diagnostic, and never silently defaults a missing version-2 field.
- `cargo test -p hekate-model` passes, including the checked-in-schema drift test.

# Context

Gated on [[TAS-061-version-2-schema-contract]].

# Result

Version 2 is now readable, validatable, and compilable in `hekate-model`.
`SUPPORTED_SCHEMA_VERSION` is 2 and version 1 keeps its direct path until the
migration leaf ([[TAS-063-v1-to-v2-migration-and-cli]]) replaces it.

- **Version-2 source shapes** (`crates/hekate-model/src/source.rs`):
  `ScenarioSourceV2` reuses the version-1 sub-structs and adds
  `ModeTemplateSource` (`ModeBodySource`, `MotionKind`, `TacticKind`,
  `AccessSource`, `FacilityKind`, `OccupancyKind`, and a profile map),
  `MovementSourceV2` with the required `MovementDirection`, and mode-tagged
  `DemandSourceV2` (`DemandSpawnSource::{Rate,Population}`, `TimeIntervalSource`,
  `DemandChoiceSource::{Movements,Routes}`). Every version-2 struct derives
  `JsonSchema`.
- **Version negotiation**: `SUPPORTED_SCHEMA_VERSION = 2`,
  `MIN_SUPPORTED_SCHEMA_VERSION = 1`, `READABLE_SCHEMA_VERSIONS = [1, 2]`. New
  `parse_scenario_document` reads the declared `schema_version` and returns
  `ScenarioDocument::{V1,V2}`; any other version returns
  `DocumentReadError::UnsupportedVersion`, whose `diagnostic()` is the stable
  `E_SCHEMA_VERSION`. `parse_scenario_source`, `validate`, and
  `CompiledScenario::compile` stay unchanged for version 1.
- **Schema generation**: `scenario_schema`/`scenario_schema_json` now generate the
  supported version-2 schema into `schemas/scenario-source.schema.json`;
  `scenario_schema_v1`/`scenario_schema_v1_json` generate the legacy version-1
  schema into the new `schemas/scenario-source-v1.schema.json`. The drift test
  checks both files; the version-1 file is byte-identical to the formerly
  checked-in schema.
- **Validation** (`validate_v2`): reuses the version-1 reference and geometry
  checks through a shared collection view and adds the version-2 rules —
  undeclared mode template (`E_DEMAND_UNKNOWN_MODE`), undeclared population path
  (`E_DEMAND_UNKNOWN_PATH`), choice/motion-family mismatch
  (`E_DEMAND_CHOICE_MISMATCH`), invalid interval (`E_DEMAND_INTERVAL`), a
  population spawn mixed with other demand (`E_DEMAND_POPULATION`), mode-template
  body/motion coexistence (`E_MODE_TEMPLATE_BODY_MOTION`), profile completeness
  for the family (`E_MODE_TEMPLATE_PROFILE`), and an explicit direction that
  disagrees with the movement's portal order (`E_MOVEMENT_DIRECTION`). An absent
  required field is a parse or validation failure, never a default.
- **Compilation**: `CompiledScenario::compile_v2` validates the version-2 document
  and maps mode templates plus mode-tagged demand onto the existing compiled
  fields through the extracted `compile_validated` path, so Increment 0 compiled
  behavior is unchanged.

The focused test `crates/hekate-model/tests/version2_source.rs` shows a
version-2 document parses, validates, and compiles; its compiled fields equal the
equivalent version-1 source's; an unknown version yields `E_SCHEMA_VERSION`; and
a missing required version-2 field is rejected rather than defaulted.

Evidence: `cargo test -p hekate-model` (68 tests) passes including both
checked-in-schema drift tests; `cargo test --workspace` passes; `cargo build
--workspace` succeeds; `cargo test -p hekate-cli --test scenarios` passes;
`cargo clippy -p hekate-model --all-targets` is clean;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`;
`tangle check` passes with this node still in `proposed/`.

Notes for [[TAS-063-v1-to-v2-migration-and-cli]]: top-level version-2 collections
are optional (absent means empty, as in version 1); `direction` is required per
movement and validated against the portal order, so `compile_v2` may drop it
without silent loss; a demand `interval_s` is validated structurally but
Increment 0 compilation has no interval scheduler, so only a whole-run interval
reproduces a Phase 1 scenario exactly.

Handoff (outside this leaf's write set): once this node is `resolved/`,
[[TAS-057-schema-v2-migration-and-provenance]] still names it in `next`, which
`tangle check` reports as one expected `next-resolved-node` diagnostic. The
coordinator owns advancing that parent's `next`.
