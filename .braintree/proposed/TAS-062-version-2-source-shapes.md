---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Add version-2 source shapes, version negotiation, and validation to tangle-model.
next: Add the version-2 source structs and version negotiation described by the contract, keeping the generated JSON Schema checked in.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

`tangle-model` parses, validates, and compiles the Increment 0 subset of schema
version 2 defined by [[TAS-061-version-2-schema-contract]], and
`SUPPORTED_SCHEMA_VERSION` is 2.

# Done when

- The version-2 source structs from the contract exist with schema generation and a regenerated `schemas/scenario-source.schema.json`.
- Version negotiation accepts version 2, rejects unknown versions with a stable diagnostic, and never silently defaults a missing version-2 field.
- `cargo test -p tangle-model` passes, including the checked-in-schema drift test.

# Context

Gated on [[TAS-061-version-2-schema-contract]].
