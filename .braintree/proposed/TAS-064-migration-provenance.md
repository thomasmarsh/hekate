---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Record source and normalized hashes and migration version in run manifests.
next: Extend the run and baseline manifests with source and normalized hash fields plus the migration version.
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
