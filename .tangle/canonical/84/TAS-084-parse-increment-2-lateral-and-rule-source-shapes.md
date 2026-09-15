---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Parse and serialize every Increment 2 version-2 source shape and regenerate the checked JSON schema.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

hekate-model parses and serializes the exact Increment 2 additive version-2
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
Owns crates/hekate-model/src/source.rs, schema.rs only as needed,
crates/hekate-model/tests/version2_source.rs or a focused new source test, the
schema generator seam, and schemas/scenario-source.schema.json. Do not compile
or validate cross-reference semantics.

Gates my generated artifact enters: the repository schema-drift test that
regenerates schemas/scenario-source.schema.json and cargo test -p hekate-model.

# Result

`crates/hekate-model/src/source.rs` parses and serializes every Increment 2
source shape fixed by
[[TAS-083-define-the-increment-2-schema-and-semantics]], and
`schemas/scenario-source.schema.json` is regenerated from those types. Every
new field is additive and optional-aware, so a document authoring none of them
parses, serializes, and compiles exactly as Increment 1.

**Shapes landed**:

- `TacticKind` gains `change_lane`, `overtake`, `pass`, and `reverse_direction`
  through the existing `tactics` extension point. `PermissionKind` already
  declared `lane_use`, `overtake`, and `crossing`, so no permission kind was
  re-added and `permissions[]` keeps its Increment 1 statement shape.
- `ModeTemplateSource.lateral: Option<ModeLateralSource>` (`target_clearance_m`,
  `horizon_s`), omitted from the normalized document when absent.
- `FacilitySource.lateral_policy: Option<FacilityLateralPolicySource>`
  (`passing_side: PassingSide` = `left`/`right`/`most_clearance`), declared
  between `lateral_use` and `speed_policy` in normalized order.
- `ScenarioSourceV2` gains `facility_adjacencies: Vec<FacilityAdjacencySource>`
  (`id`, `first`, `second`, `side: AdjacencySide` = `left`/`right`),
  `clearance_bands: Vec<ClearanceBandSource>` (`id`, `threshold_m`, `violation`,
  optional `applies_to_modes: Option<Vec<String>>`), and
  `maneuver_policy: Option<ManeuverPolicySource>` (`commit`, `wrong_way`, each an
  optional object whose own fields are required once present).
- No alias tags, no implicit units, no mode-specific top-level variant: the four
  tactic tags, `passing_side`, and `side` accept only their authored spellings, a
  unit-bearing string fails to parse, and every new object keeps
  `deny_unknown_fields`.
- Bands keep their authored `id` and declaration order in the `Vec` (nothing is
  re-sorted), and `target_clearance_m`, `horizon_s`, `threshold_m`,
  `max_opposing_density_per_km`, and `urgency` round trip byte-exactly.
- `applies_to_modes` is `Option<Vec<String>>` rather than a defaulted
  `Vec<String>` because the contract makes a present-but-empty list a validation
  error, so absent and empty must stay distinguishable.

**Compiler-forced closure paths** (authored under the additive/optional-field
exception; only the new tag/field sites were touched, no new compilation or
validation behavior):

- `crates/hekate-model/src/mode_template.rs` — adding `TacticKind` variants made
  the exhaustive `compiled_tactic` match non-exhaustive. It became
  `compiled_tactics`, a `flat_map` applying TAS-083's already-fixed mapping
  (`change_lane`→`ChangeLane`, `overtake`→`Overtake`, `pass`→`Pass`,
  `reverse_direction`→`ReverseNominalDirection`, plus
  `ChooseLateralPosition` for `change_lane`/`pass`). This variant is outside the
  leaf's enumerated write set; the coordinator approved it as a compiler-forced
  closure edit. Lateral policy, corridors, and traversal resolution stay with
  [[TAS-085-compile-increment-2-policy-and-traversal-semantics]].
- `crates/hekate-model/src/lib.rs` — the new source types join the existing
  `pub use source::{…}` re-export list, which is how every other source type is
  exposed.
- `crates/hekate-model/src/migrate.rs` — the exhaustive `ScenarioSourceV2`
  literal gains `facility_adjacencies: Vec::new()`, `clearance_bands: Vec::new()`,
  and `maneuver_policy: None`, and both `ModeTemplateSource` literals gain
  `lateral: None`. Migration behavior is unchanged; both golden migration
  fixtures still match byte-for-byte.
- `crates/hekate-model/tests/mode_template_compilation.rs` — three
  `ModeTemplateSource` literals gain `lateral: None`.
- `schemas/scenario-source.schema.json` — regenerated by
  `cargo run -p hekate-model --example generate-schema`;
  `schemas/scenario-source-v1.schema.json` is byte-identical (the v1 shape is
  untouched).
- `crates/hekate-model/src/schema.rs` and
  `crates/hekate-model/examples/generate-schema.rs` needed no edit: the drift
  test and the generator already cover the new types.

**Tests**: new `crates/hekate-model/tests/increment2_source.rs` (4 tests) covers
the authored values of every new shape, a canonical round trip with preserved
precision, the Increment 1 absence rules in both parsing and serialization, and
rejection of alias tags, an unknown adjacency side, implicit units, unknown
fields at the document and policy level, and a missing required `violation`.

**Acceptance**:

- `cargo run -p hekate-model --example generate-schema` then a second run —
  `sha256sum schemas/scenario-source.schema.json` is
  `548218442da29e334538511f75027dd7bf7bb9d6c8caaa57908371633468aeb8` both times,
  and `schema::tests::checked_in_v2_schema_matches_the_generated_schema` plus its
  v1 twin pass. The checked-in schema equals the generated one; the working-tree
  `git diff` is not clean only because the coordinator owns the commit, so this
  path is not hand-edited.
- `cargo test -p hekate-model` — 143 passed, 0 failed (75 lib, then
  agent_components 4, facility_validation 20, fuzz_scenarios 3,
  increment2_source 4, migration 8, mode_template_compilation 10,
  narrow_mode_templates 8, synthetic_mode_template 4, version2_source 6,
  walking_scenario 1).
- `cargo clippy -p hekate-model --all-targets` — clean, no warnings.
- `cargo check --workspace --all-targets` — clean; no other crate constructs the
  changed types or matches the changed enum.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `tangle check` — `graph check: passed (154 nodes)` while this node was still
  in `proposed/`.
- Post-move `tangle check --allow-pending-advance TAS-082` —
  `graph check: passed (154 nodes)`. Plain `tangle check` now reports only
  that [[TAS-082-increment-2-authored-and-compiled-contract]]'s `next` names this
  already-resolved node, which is the parent advance the coordinator owns.

**Scope kept out and remaining work**: none for this node. Validation of the new
shapes stays with [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]],
compiled policy, corridors, and the `required_profile_params` extension for the
lateral profile set stay with
[[TAS-085-compile-increment-2-policy-and-traversal-semantics]], and every fixture,
metric, and presenter leaf keeps its own scope. No child node was created and no
node was left `active`: the whole `# Done when` was met in this session.

Friction (for the session FBK, not created here):

- Attempted: add the four `TacticKind` values while staying inside the
  enumerated write set. Friction: a new enum variant makes an exhaustive `match`
  in another leaf's file (`src/mode_template.rs::compiled_tactic`) fail to
  compile, and that file was explicitly excluded rather than named as a closure
  path, so the compile closure and the enumerated write set disagreed.
  Improvement: state a changed-enum write set as its compile closure, or have
  tasking name every exhaustive `match` over a changed enum.
- Attempted: encode the optional-mode band field as a defaulted `Vec<String>`
  with `skip_serializing_if = "Vec::is_empty"`, matching every other additive
  array in the same document. Friction: the contract makes present-but-empty
  invalid, so that encoding would erase the distinction validation needs.
  Improvement: have the contract say which absence encoding a field needs
  whenever present-and-empty is itself invalid.
