---
context_rev: 1
updated: 2026-09-13T19:19:51Z
summary: Add Increment 1 facility and narrow-mode validation rules with stable diagnostics.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

`validate_v2` enforces the Increment 1 rules against the compiled facility and
mode-template geometry: geometric containment of a facility in the traversable
world, usable width after the body envelope and clearance, reference-path
curvature against the mode's turning limits, connector continuity, directional
reachability, mode-to-facility access, legal versus physically possible routes,
and spawn clearance for the largest eligible body. It also fixes the narrow
wheeled family's profile-parameter set (including steering response and
lateral-clearance preference) so a `bicycle`/`scooter` template is accepted
exactly when its profiles are complete and well-formed. Every rule has a stable
diagnostic code.

# Done when

- Each new Increment 1 validation rule has a stable `DiagnosticCode`, a rejecting
  fixture that triggers it, and an accepting fixture that does not.
- A malformed facility (outside the world, too narrow for the largest eligible
  body, curvature beyond a mode's turning limit, a discontinuous connector, an
  unreachable direction, or an access violation) is rejected with the naming
  diagnostic, and a legal-but-physically-impossible route is distinguished from an
  illegal one.
- `validate_v2` remains total on every existing version-2 fixture; `cargo test -p
  tangle-model` passes with no Phase 1 regression.

# Context

Gated on [[TAS-074-compile-version-2-facility-and-connector-shapes]]. Per
`PHASE_2_PLAN.md` *Schema version 2* (validation list) and *Validation strategy*.
Read [[TAS-073-extend-the-version-2-schema-contract-with-increm]] (the exact
field sets this validates), [[TAS-062-version-2-source-shapes]], and
[[TAS-067-mode-template-compilation]]. Owns `crates/tangle-model/src/validate.rs`
(including the shared `required_profile_params` the template compiler reads), so
[[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]] gates on this leaf.

# Result

`validate_v2` now enforces the Increment 1 facility, connector, permission, and
narrow-wheeled profile rules against the version-2 source and its compiled
reference geometry, and `compile_v2` no longer indexes a malformed facility.
Every rule has a stable `DiagnosticCode` and a rejecting plus an accepting
fixture in `crates/tangle-model/tests/facility_validation.rs` (plus one compiled
curvature fixture in `validate::tests`).

## Rules, diagnostics, and fixtures

| Rule | Diagnostic | Rejecting fixture |
| --- | --- | --- |
| Facility references a region that is not declared | `E_FACILITY_UNKNOWN_REGION` | `rejects_a_facility_with_an_undeclared_reference` |
| Facility references a reference path that is not declared | `E_FACILITY_UNKNOWN_PATH` | `rejects_a_facility_with_an_undeclared_reference` |
| Facility access names a mode template that is not declared | `E_FACILITY_UNKNOWN_MODE` | `rejects_a_facility_with_an_undeclared_reference` |
| Facility permits no mode | `E_FACILITY_ACCESS_EMPTY` | `rejects_a_facility_that_permits_no_mode` |
| Directional nominal direction without a reference path | `E_FACILITY_DIRECTION_WITHOUT_PATH` | `rejects_a_directional_facility_without_a_reference_path` |
| Facility width non-finite or non-positive | `E_FACILITY_WIDTH` | `rejects_invalid_facility_width_and_speed_limit` |
| Facility speed limit non-finite or non-positive | `E_FACILITY_SPEED_LIMIT` | `rejects_invalid_facility_width_and_speed_limit` |
| Connector references a facility that is not declared | `E_FACILITY_CONNECTOR_UNKNOWN_FACILITY` | `rejects_a_connector_to_an_undeclared_facility` |
| Connector attaches a facility with no reference path | `E_FACILITY_CONNECTOR_WITHOUT_REFERENCE` | `rejects_a_connector_that_attaches_a_facility_without_a_reference` |
| Permission holder or target not declared | `E_PERMISSION_UNKNOWN_HOLDER`, `E_PERMISSION_UNKNOWN_TARGET` | `rejects_undeclared_permission_references` |
| Containment: facility region outside the world | `E_FACILITY_OUTSIDE_WORLD` | `rejects_a_facility_region_outside_the_world` |
| Usable width (also spawn clearance): body plus clearance does not fit | `E_FACILITY_TOO_NARROW` | `rejects_a_facility_too_narrow_for_the_largest_eligible_body` |
| Curvature against the mode's turning limit | `E_FACILITY_CURVATURE` | `validate::tests::rejects_a_reference_curved_beyond_a_modes_turning_limit` |
| Connector continuity | `E_FACILITY_CONNECTOR_DISCONTINUOUS` | `rejects_a_discontinuous_connector` |
| Directional reachability | `E_FACILITY_UNREACHABLE_DIRECTION` | `rejects_an_unreachable_nominal_direction` |
| Mode-to-facility access kind | `E_FACILITY_ACCESS_DENIED` | `rejects_a_mode_whose_template_does_not_serve_facilities` |
| Legal (`prohibit`) versus physically impossible | `E_PERMISSION_ROUTE_PROHIBITED` | `distinguishes_an_illegal_route_from_a_physically_impossible_one` |
| Narrow wheeled profile completeness | `E_MODE_TEMPLATE_PROFILE` | `requires_the_narrow_wheeled_steering_and_clearance_profiles` |
| Lateral clearance is non-negative | `E_PROFILE_NON_NEGATIVE` | `accepts_a_zero_lateral_clearance_and_rejects_a_negative_one` |
| Absent `limit_mps` is a parse error, explicit `null` is unlimited | parse error | `distinguishes_an_absent_speed_limit_from_an_explicit_null` |

The accepting fixture for every rule is the base `FACILITY` bikeway
(`accepts_a_well_formed_facility_document`, `compiles_a_well_formed_facility_document`).
Physical impossibility keeps its own codes
(`E_FACILITY_UNREACHABLE_DIRECTION`, `E_FACILITY_CURVATURE`,
`E_FACILITY_TOO_NARROW`) that `distinguishes_an_illegal_route_from_a_physically_impossible_one`
asserts are absent when only a `prohibit` makes the route illegal.

## Known reconciliations

1. **No malformed facility panics `compile_v2`.** `validate_v2` now resolves and
   rejects every reference `compile_v2` indexes — facility → region, reference
   path, and access mode template; connector → facility — before any indexing
   runs. `compile_v2_rejects_a_malformed_facility_without_panicking` proves a
   document with an undeclared region returns diagnostics rather than panicking.
   Residual: the pre-existing `v2_to_v1_view` assumption that a `passenger_car`
   template is box/wheeled (a `passenger_car` that is a walking circle still
   panics in `profile_param`) is unchanged; it is not a facility path and not
   introduced or touched here.
2. **`facility.speed_policy.limit_mps` is required-nullable.** The new
   `SpeedLimitMps` newtype in `source.rs` has a hand-written `Deserialize`, so an
   absent `limit_mps` is a parse error while an explicit `null` is unlimited
   (serde's `Option`-derived newtype treated absence as `null`).
   `schemas/scenario-source.schema.json` is regenerated: `limit_mps` now appears
   in `SpeedPolicySource.required` and its value is the nullable
   `SpeedLimitMps`. The whole `facility.speed_policy` object was already
   required.
3. **Narrow wheeled profile set.** `required_profile_params` is now keyed on
   `(body, motion)`; `capsule` + `single_body_wheeled` requires
   `steering_rate_max_rad_s` (strictly positive) and `lateral_clearance_m`
   (a new non-negative rule, `E_PROFILE_NON_NEGATIVE`) alongside the Increment 0
   wheeled parameters. The contract's `body.kind: 'capsule'` needed a source
   shape, so this leaf also added `ModeBodySource::Capsule` (compiling to the
   existing `AgentBody::Capsule`/`wheeled_capsule` family) and regenerated the
   schema; **TAS-075 owns the capsule source variant and the schema regeneration,
   and TAS-076 must consume them, not re-add them.** The compiled
   `AgentBehaviorProfile` does not yet carry the two narrow parameters (the
   `compiled_profile` consumer only reads the parameters it maps); TAS-076 owns
   adding those component fields and reading them.

## Scope notes

- **Spawn clearance** is enforced by the usable-width rule: with a constant
  authored `width_m` the usable lateral interval is identical at every arc
  length, so the spawn point has room exactly when the band fits every permitted
  mode's largest body plus its lateral clearance. There is no separate
  variable-width spawn geometry in Increment 1, so no separate code.
- **Curvature** is validated against the compiled reference geometry; authored
  reference paths are polylines whose segments have zero curvature (TAS-074
  note), so the source fixture cannot trigger it and the checked-in rejecting
  fixture builds the analytic `CompiledReferencePath::arc` directly. The rule is
  wired into `validate_v2` and will reject authored constant-curvature
  references when they exist.

## Files changed

`crates/tangle-model/src/validate.rs` (rules, codes, shared
`required_profile_params`), `crates/tangle-model/src/source.rs`
(`ModeBodySource::Capsule`, `SpeedLimitMps`), `crates/tangle-model/src/mode_template.rs`
(capsule body compile, speed-policy and required-profile consumers),
`crates/tangle-model/src/compiled.rs` (capsule arms in the v1-view profile
materializers), `crates/tangle-model/src/lib.rs` (`SpeedLimitMps` export),
`schemas/scenario-source.schema.json` (regenerated), and the new
`crates/tangle-model/tests/facility_validation.rs`. No `scenarios/**`,
`baselines/**`, `crates/tangle-sim/**`, `crates/tangle-present/**`, or `apps/**`
file changed; no Phase 1 golden trace or baseline changed.

## Acceptance

- `cargo test -p tangle-model` — passes (74 lib + 20 `facility_validation` + all
  suites), including `schema::tests::checked_in_v2_schema_matches_the_generated_schema`.
- `cargo build --workspace` and `cargo test --workspace` — pass.
- `cargo test -p tangle-cli --test baseline --test migration_regression --test scenarios` — pass (14 tests; the Phase 1 baseline and migrated goldens are unchanged).
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `cargo fmt -p tangle-model -- --check` and `cargo clippy -p tangle-model --all-targets` — clean.
- `braintree check` — `graph check: passed (118 nodes)` while this node was still
  in `proposed/`.

Post-move note: after this file moves to `resolved/`, `braintree check`
transiently reports `next-resolved-node` because
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]] still names TAS-075 in
its `next`. That is expected; the coordinator advances the parent's `next`. No
parent or sibling node file was edited and no child node was created.
