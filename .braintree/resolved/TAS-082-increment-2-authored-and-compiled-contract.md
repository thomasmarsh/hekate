---
context_rev: 1
priority: P1
updated: 2026-09-14T01:34:04Z
summary: Increment 2 authored, parsed, compiled, and validated policy contract is complete.
---

Parent [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]].

# Outcome

Version 2 has one documented, parsed, compiled, and validated Increment 2
surface for lateral tactics, permissions, clearance bands, maneuver policy, and
contextual opposing traversal. Consumers can distinguish the additive surface
without inferring semantics from mode names.

# Done when

- [[TAS-083-define-the-increment-2-schema-and-semantics]] fixes exact fields,
  units, defaults, state meanings, event evidence, and deferral boundaries.
- [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]] parses and
  regenerates the checked schema for the new authored shapes.
- [[TAS-085-compile-increment-2-policy-and-traversal-semantics]] compiles them
  into identifier-resolved policy and physically possible traversal data.
- [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] rejects every
  malformed or internally impossible combination with stable diagnostics.
- All four children are resolved or deliberately disposed, the schema drift
  gate and model tests pass, and the result maps each downstream consumer to
  the compiled field it reads.

# Context

Extends the resolved Increment 1 contract in place; it does not reinterpret an
existing version-2 field. This coordinating node owns ledger roll-up only.

# Result

All four children resolved; none disposed. The Increment 2 authored, parsed,
compiled, and validated surface is complete and lands as the dependency contract
for TAS-087 through TAS-111.

- **Contract.** [[TAS-083-define-the-increment-2-schema-and-semantics]] extended
  `docs/schema-v2-contract.md` in place with the *Increment 2 additions*
  section: lateral tactics and maneuver parameters, facility lateral-use policy,
  `facility_adjacencies[]`, `clearance_bands[]`, `maneuver_policy`, the
  `lane_use`/`overtake`/`crossing` permission semantics, compiled
  usable/predicted corridors and clearance facts, the five-state maneuver
  lifecycle with deterministic claim arbitration and commitment-loss policy,
  contextual wrong-way decision inputs and violation-interval boundaries, and
  the versioned event/metric/trajectory surfaces. The Increment 1 deferral rows
  are reconciled and 12 drifted seam line citations were refreshed.
- **Parse and schema.** [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]]
  added the source types (`ModeLateralSource`, `FacilityLateralPolicySource`,
  `FacilityAdjacencySource`, `ClearanceBandSource`, `ManeuverPolicySource`, the
  four `TacticKind` variants) and regenerated
  `schemas/scenario-source.schema.json`; the schema-drift test passes.
- **Compile.** [[TAS-085-compile-increment-2-policy-and-traversal-semantics]]
  resolves the policy into identifier-resolved data with focused accessors:
  `CompiledScenario::traversal_policy`, `transitions`, `permission_effect`,
  `crossing_permission`, `maneuver_policy`, `commit_policy`, `wrong_way_policy`;
  `FacilityTraversalPolicy` exposure of nominal/permitted/physically-possible
  direction sets, `usable_interval`, `lateral_use`, `passing_side`, `lane_use`,
  `overtake`; `CompiledFacilityAdjacency::transition`/`LateralTransition`;
  `CompiledClearanceBand`; and `CompiledModeTemplate::lateral`.
- **Validate.** [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]]
  enforces every Increment 2 coupling, geometry, ordering, and permission rule
  with stable diagnostics, extending the shared `required_profile_params` seam
  so validation and compilation require the same lateral wheeled profile set,
  and proving the contract-faithful inert no-opposing-path case.

Downstream consumer -> compiled field map: TAS-087/TAS-088 read
`CompiledScenario::traversal_policy` and `CompiledModeTemplate::lateral`;
TAS-089 reads `CompiledLateralPolicy` plus the profile's
`steering_rate_max_rad_s` and `lateral_accel_max_mps2`; TAS-090/TAS-091 read
`transitions` and `permitted_directions`; TAS-093/TAS-094/TAS-095 read
`passing_side`, `lane_use`, `overtake`, and `LateralTransition::side`;
TAS-096/TAS-097 read `nominal_directions`, `nominal_effect`,
`physically_possible_directions`, `permission_effect`, and `wrong_way_policy`;
TAS-100/TAS-101 read `ClearanceBandId`, `clearance_bands`, and `PassingSide`.

Gate evidence: `cargo test -p tangle-model` 167 passed; `cargo test --workspace`
761 passed; the schema-drift test passes; `scripts/check-dependency-direction.sh`
reports `dependency direction OK`; `braintree check` passes on the landed graph.
No Phase 1 or Increment 1 behavior changed. Concrete implementers TAS-084 through
TAS-086 own the commit trail under this roll-up.
