---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Settle the version-2 schema additions and the exact version-1 migration mapping.
next: Write the version-2 schema contract: added source shapes, validation rules, and the field-by-field version-1 migration mapping.
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
