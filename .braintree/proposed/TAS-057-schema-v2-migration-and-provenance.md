---
context_rev: 1
priority: P1
updated: 2026-09-13T16:05:00Z
summary: Deliver schema version 2 with a deterministic version-1 migrate path and run-manifest provenance
next: [[TAS-063-v1-to-v2-migration-and-cli]]
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
