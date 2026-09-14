---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Parse Increment 2 lateral policy, clearance bands, and permission source shapes.
next: Implement the TAS-083 source shapes and regenerate the checked JSON schema.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

tangle-model parses and serializes the exact Increment 2 additive version-2
source shape and the checked JSON schema describes the same shape.

# Done when

- Source types cover every TAS-083 field without accepting aliases, implicit
  units, or mode-specific top-level variants.
- change_lane, overtake, pass, and reverse_direction capabilities plus
  lane_use, overtake, and crossing permission kinds use the existing
  mode-template and permission extension points.
- Clearance bands have stable IDs and deterministic declaration order; target
  clearance, horizon, and policy values preserve authored precision.
- Missing optional Increment 2 fields retain Increment 1 behavior, while
  malformed enum tags and unknown fields fail parsing where the existing
  contract requires strictness.
- generate-schema produces schemas/scenario-source.schema.json with no
  hand-edited drift, and round-trip/source tests pass.

# Context

Gated on [[TAS-083-define-the-increment-2-schema-and-semantics]].
Owns crates/tangle-model/src/source.rs, schema.rs only as needed,
crates/tangle-model/tests/version2_source.rs or a focused new source test, the
schema generator seam, and schemas/scenario-source.schema.json. Do not compile
or validate cross-reference semantics.

Gates my generated artifact enters: the repository schema-drift test that
regenerates schemas/scenario-source.schema.json and cargo test -p tangle-model.
