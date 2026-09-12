---
context_rev: 1
priority: P1
updated: 2026-09-12T11:08:44Z
summary: Parse and validate the walking-skeleton JSON5 scenario into an immutable compiled scenario.
---

# Context

Depends on [[DEC-001-phase-1-kickoff-defaults]] at context_rev 1.

# Outcome

`tangle-model` parses a versioned JSON5 scenario containing one guide path and
a portal at each end, validates it, and exposes an immutable compiled scenario
with stable string IDs mapped to dense integer IDs.

# Done when

- The source schema is versioned and parsed with Serde; a JSON Schema is
  generated and checked in.
- Semantic validation rejects duplicate IDs, non-finite coordinates,
  non-positive dimensions or durations, and unreachable portals with stable
  diagnostic codes and source object IDs.
- Compilation exports the string-to-integer ID mapping in run provenance.

# Result

Implemented in `crates/tangle-model`:

- `source.rs` — versioned `ScenarioSource` (JSON5 via `serde_json5`), one
  `PathSource` plus `PortalSource`s and a `PopulationSource`, with Serde and
  Schemars derives and `deny_unknown_fields`.
- `validate.rs` — `validate` returns `Diagnostic { code, object, message }` with
  stable codes `E_SCHEMA_VERSION`, `E_ID_EMPTY`, `E_ID_DUPLICATE`,
  `E_NON_FINITE`, `E_NON_POSITIVE`, `E_PATH_EMPTY`, `E_PATH_DEGENERATE`,
  `E_PORTAL_UNKNOWN_PATH`, `E_PORTAL_UNREACHABLE`, and `E_PORTAL_DUPLICATE_END`.
- `compiled.rs` — immutable `CompiledScenario` with dense `PathId`/`PortalId`,
  cached arc-length paths (`position_at`/`heading_at`), portals at path ends,
  and `IdMap` exposing the stable string-to-dense mapping for run provenance.
- `schema.rs` and `examples/generate-schema.rs` — checked-in
  `schemas/scenario-source.schema.json` with a drift test.
- `scenarios/walking/walking_guide_v1.json5` — the walking-skeleton scenario.

Evidence: `cargo test -p tangle-model` passes 19 unit and 1 integration tests;
`cargo clippy --workspace --all-targets --all-features` is warning-free;
`cargo fmt --all --check` passes; `scripts/check-dependency-direction.sh`
reports `dependency direction OK`. `tangle-sim` (TAS-004) consumes
`CompiledScenario`, `CompiledPath`, and `PopulationSource`.

Parent [[TAS-001-phase-1-increment-0]].
