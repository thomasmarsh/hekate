---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Implement deterministic version-1 to version-2 migration and expose it as a CLI subcommand.
next: Implement the pure version-1 to version-2 migration transform, its golden outputs, and the migrate subcommand.
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
