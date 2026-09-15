---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Compile version-2 facility and connector shapes to reference-path geometry in hekate-model.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

`hekate-model` parses the Increment 1 version-2 facility and connector source
shapes and compiles them into geometry: a continuous-width facility region with
an optional reference path exposing arc length `s`, signed lateral offset `d`,
tangent, normal, and curvature; usable lateral intervals after the current body
envelope and configured clearance; facility adjacency and connectors across
traversal directions; and separately authored nominal/permitted versus physically
possible traversal directions. `CompiledScenario` carries compiled facilities and
compiled mode templates instead of only materializing them into the version-1
view, while version-1 and existing version-2 compiled behavior stay unchanged.

# Done when

- The version-2 source structs for facilities, facility connectors, access, and
  permission/obligation shapes exist with `JsonSchema`, and
  `schemas/scenario-source.schema.json` is regenerated with the checked-in-schema
  drift test passing.
- `CompiledScenario` exposes compiled facilities (region, reference-path
  coordinates, usable intervals, connectors, and both direction properties) and
  the compiled mode-template bundles; `compile_v2` populates them.
- A checked-in analytic test proves path-to-world-to-path round-trip error
  `<= 1e-9 m` (`T-RT`) on a straight facility and a constant-curvature facility,
  using the compiled reference path, and that the usable lateral interval
  subtracts the body envelope and clearance.
- `cargo test -p hekate-model` and `cargo build --workspace` pass; no Phase 1
  golden trace or baseline changes.

# Context

Gated on [[TAS-073-extend-the-version-2-schema-contract-with-increm]]. Per
`PHASE_2_PLAN.md` *Facilities, paths, and free space* and *Schema version 2*. Read
[[TAS-061-version-2-schema-contract]], [[TAS-062-version-2-source-shapes]], and
[[TAS-066-compiled-agent-component-model]] for the compiled seams. Owns
`crates/hekate-model/src/source.rs`, `crates/hekate-model/src/compiled.rs`, and
`schemas/scenario-source.schema.json`. This leaf is the facility/coordinate
contract every controller and fixture leaf consumes.

# Result

The Increment 1 facility, connector, access, and permission source shapes and
their compiled reference-path geometry land in `hekate-model`, additive to
Increment 0 and to Phase 1. No new schema version and no validation codes (those
are [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]]).

## New source shapes (`crates/hekate-model/src/source.rs`)

- `FacilitySource` — `id`, `region`, optional `reference_path`, `width_m`,
  `nominal_direction` (`FacilityDirection`), `access` (`FacilityAccessSource`),
  `lateral_use` (`LateralUse`), `speed_policy` (`SpeedPolicySource`).
- `FacilityConnectorSource` / `FacilityConnectorEndSource` — `id`, `from`/`to`
  `{ facility, direction: MovementDirection }` (forward/reverse).
- `PermissionSource` with `PermissionKind`
  (`nominal_direction`/`lane_use`/`overtake`/`crossing`/`stop_service`) and
  `PermissionEffect` (`permit`/`prohibit`/`obligate`), plus `holder` and `target`.
- `FacilityDirection` (forward/reverse/either) and `LateralUse`
  (shared/centered); `FacilityKind::Facility` added.
- `AccessSource` gains the additive optional `nominal_direction` and
  `speed_policy`; absent keeps the Increment 0 meaning (either / unlimited).
- `ScenarioSourceV2` gains the optional arrays `facilities`,
  `facility_connectors`, and `permissions`. All three, and the access fields, use
  `#[serde(default, skip_serializing_if = ...)]` so a document without them
  serializes byte-identically to before: the checked-in Phase 1 baseline
  `normalized_sha256` and the migration goldens are unchanged (the baseline lives
  outside this leaf's write set).
- `validate.rs` `v2_ids` adds the three new object kinds to the shared
  duplicate-id space only; the Increment 1 semantic rules stay TAS-075's.

## New compiled surface (`crates/hekate-model/src/compiled.rs`)

- Dense ids `ModeTemplateId`, `FacilityId`, `FacilityConnectorId`; `IdMap` gains
  `facilities` and `facility_connectors` (empty on the version-1 path, so
  migration's `v1.id_map() == v2.id_map()` still holds).
- `CompiledReferencePath` — the facility's compiled reference geometry: an
  ordered sequence of straight and circular-arc segments. `from_polyline` builds
  the authored polyline's straight segments; `arc(center, radius, start_angle,
  sweep)` builds the analytic constant-curvature reference the `T-RT` evidence
  needs (authored paths are polylines, so an exact arc cannot be authored yet).
  Exposes `length`, `position_at`, `heading_at`, `tangent_at`, `normal_at`,
  `curvature_at`, `point_at(s, d)`, and `project(point) -> RouteCoordinate`.
  Sign convention: `d` positive to the left, `normal = tangent` rotated +90°,
  `kappa = dtheta/ds` positive counter-clockwise; a polyline segment reports zero
  curvature and a vertex is attributed to the segment it starts.
- `CompiledFacility` — `id`, `name`, `region`, optional
  `CompiledFacilityReference` (the authored `PathId` plus its
  `CompiledReferencePath`), `width_m`, `nominal_direction`
  (`NominalDirection`), `access` (`&[ModeTemplateId]`), `lateral_use`,
  `speed_policy` (`SpeedPolicy`), `outgoing_connectors`/`incoming_connectors`,
  and `physically_possible_directions`/`is_physically_possible`. Delegating
  `length`/`position_at`/`heading_at`/`tangent_at`/`normal_at`/`curvature_at`
  return `Option` because a facility without a reference path has no `(s, d)`
  frame. `usable_lateral_interval(envelope_width_m, clearance_m) ->
  UsableLateralInterval` computes `d_min = -(W/2 - envelope/2 - clearance)` and
  `d_max = +(...)`, empty when `envelope + 2*clearance > W`.
- `CompiledFacilityConnector` / `FacilityTraversal` — the directed edge, naming
  the `(facility, direction)` it leaves and enters. `CompiledScenario` gains
  `mode_templates()`, `facilities()`, `facility_connectors()`, and their
  by-id lookups. The three direction properties stay separate: authored
  `nominal_direction`, the mode's permitted direction via `access()` (plus the
  compiled mode template's `access().nominal_direction()`), and the connector-
  derived `is_physically_possible`.
- `compile_v2` compiles the mode-template bundles
  (`compile_mode_templates`), the facilities (`compile_facilities`), the
  connectors (`compile_facility_connectors`), and attaches adjacency
  (`attach_facility_adjacency`) before running the shared `compile_validated`
  version-1 view; the version-1 path leaves the three new fields empty.
  `mode_template.rs` `compiled_access` now reads the additive access direction
  and policy (shared `compiled_nominal_direction` / `compiled_speed_policy`).

## Tests and observed `T-RT`

- `compiled::tests::facility_reference_round_trips_path_world_path_within_trt`
  — the checked-in `T-RT` evidence. Round-trip error observed:
  straight facility `0.0 m` (exact), constant-curvature facility (CCW quarter
  circle, R = 50 m) `7.105427357601002e-15 m`, clockwise control `7.105e-15 m`;
  all `<= 1e-9 m`. It also asserts `curvature_at == 1/R` on the arc and the
  usable-lateral-interval identity `W/2 - envelope/2 - clearance` (and the empty
  interval when the band is too narrow).
- `compiled::tests::compiles_facilities_with_reference_coordinates_access_and_connectors`
  — compiled facilities, access, connector adjacency, nominal vs physically
  possible directions, and the `IdMap` names.
- `source::tests::parses_facilities_connectors_access_and_permissions` and
  `source::tests::an_increment_0_access_omits_the_additive_direction_and_policy_fields`.

## Files changed

`crates/hekate-model/src/source.rs`, `crates/hekate-model/src/compiled.rs`,
`crates/hekate-model/src/lib.rs`, `crates/hekate-model/src/migrate.rs`,
`crates/hekate-model/src/mode_template.rs`, `crates/hekate-model/src/validate.rs`,
`crates/hekate-model/tests/mode_template_compilation.rs`, and
`schemas/scenario-source.schema.json` (regenerated; the version-1 schema is
unchanged). No scenario under `scenarios/**` changed: the new arrays and access
fields are optional, so no existing fixture needed a new required field, and no
Phase 1 golden trace or baseline changed.

## Acceptance

- `cargo test -p hekate-model` — passes (73 lib + all integration suites);
  includes `schema::tests::checked_in_v2_schema_matches_the_generated_schema`.
- `cargo build --workspace` — passes.
- `cargo test --workspace` — passes.
- `cargo test -p hekate-cli --test baseline --test migration_regression` —
  passes (8 tests): the checked-in Phase 1 baseline and the migrated golden
  traces are byte-identical.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `cargo fmt --all --check` and `cargo clippy -p hekate-model --all-targets` —
  clean.
- `tangle check` — `graph check: passed (118 nodes)` while this node was
  still in `proposed/`.

## Scope notes for the next leaves

- The `permissions[]` source shape exists in full; compiling permissions and the
  `.permissions[]`-derived part of the permitted direction is not part of this
  leaf's `Done when` and is left to the increment that consumes the non-nominal
  kinds (the deferred kinds have no compiled target object, e.g. `bus_stop`).
  `access.nominal_direction` is compiled.
- A facility that references an undeclared region, path, or mode template is not
  yet rejected: `compile_facilities` indexes those references exactly as the
  existing compiled path indexes movement references, assuming validation has
  run. [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]] owns those
  reference rules; until it lands, such a document would panic in `compile_v2`
  rather than return diagnostics.
- `speed_policy.limit_mps` uses `Option<f64>` like the Increment 0
  `TimeIntervalSource.end_s`, so an omitted `limit_mps` reads as `None`
  (unlimited) rather than a parse error; TAS-075 owns tightening that if the
  contract requires presence.
- `curvature` on an authored polyline is per-segment (zero on a straight
  segment); the exact constant-curvature evidence uses the compiled `arc`
  reference, which is the analytic geometry `docs/benchmark-matrix.md` names for
  `T-RT`.

Post-move note: after this file moves to `resolved/`, `tangle check`
transiently reports `next-resolved-node` because
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]] still names TAS-074 in
its `next`. That is expected; the coordinator advances the parent's `next`. No
parent or sibling node file was edited and no child node was created.
