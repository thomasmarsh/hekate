---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: The checked-in version-2 schema contract at docs/schema-v2-contract.md fixes the Increment 0 source shapes, version negotiation, the deterministic version-1 field mapping, the provenance fields, and the explicit deferrals.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

A checked-in contract, cited by the version-2 implementation, that fixes what
schema version 2 adds beyond version 1 for Increment 0, the version-negotiation
rule, and the exact field mapping from a version-1 document to normalized
version 2.

# Done when

- The contract enumerates each added version-2 source shape, its purpose, and the increment that populates it.
- It states the deterministic version-1 field mapping: passenger-car template, path-to-facility direction, and population-to-demand.
- It fixes where the source hash, normalized version-2 hash, and migration version are recorded.
- It names explicitly what version 2 defers to later increments, including facilities, transit stops, and pedestrian groups.

# Context

No gate; first frontier action of [[TAS-057-schema-v2-migration-and-provenance]].

# Result

The version-2 schema contract is checked in at `docs/schema-v2-contract.md` and
is the authority [[TAS-062-version-2-source-shapes]] through
[[TAS-065-phase-1-migration-regression]] cite. It defines:

- the version-2 source-shape inventory, marking what Increment 0 populates now
  (`mode_templates` with `passenger_car` and `pedestrian`, `movements[].direction`,
  the mode-tagged `demand` shape) against what version 2 defers;
- the version-negotiation rule (accept 1 for migration and 2 for normalization,
  reject any other version with `E_SCHEMA_VERSION`, never silently default a
  missing version-2 field);
- the deterministic version-1 field mapping: profile/population body and speed
  fields to a passenger-car mode template, `PathSource` plus the movement's
  portal order to an explicit `direction` (the directional reference path), and
  `DemandSource`/`PedestrianDemandSource`/`PopulationSource` to mode-tagged
  version-2 demand;
- the provenance fields (`schema_version`, `content_sha256`, `normalized_sha256`,
  `migration_version`) and that they are recorded on `ScenarioProvenance` in both
  the run manifest and the baseline manifest;
- the explicit deferrals: facilities (Increment 1), transit stops and service
  (Increment 4), and pedestrian groups (Increment 5), plus other motion/body
  families and permissions/obligations.

Documentation only: no Rust code or JSON schema changed in this leaf, and
`scripts/check-dependency-direction.sh` still passes. `tangle check` passed
with the node in `proposed/`. After the move to `resolved/`, `tangle check`
reports one expected `next-resolved-node` diagnostic: the parent
[[TAS-057-schema-v2-migration-and-provenance]] still names this now-resolved
node in its `next`. Repointing that `next` is the parent's handoff action.
