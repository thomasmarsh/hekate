---
context_rev: 1
priority: P1
updated: 2026-09-14T01:11:12Z
summary: Compile Increment 2 policies into resolved corridors, permissions, and traversal options.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

The immutable compiled scenario exposes component-driven lateral policy,
clearance definitions, adjacency, and opposing traversal facts that simulation
can consume without source-string lookup or mode-name branching.

# Done when

- Compiled policy resolves mode template, facility, connector, permission, and
  clearance-band IDs once, preserves stable source order, and exposes focused
  accessors used by the controller stages.
- For each eligible body and route position it distinguishes the usable
  interval, nominal and permitted directions, physically connected traversal
  directions, transition targets, and applicable pass/line-crossing policy.
- A missing optional Increment 2 policy compiles to the documented
  no-free-lateral-motion Increment 1 behavior.
- Unit tests cover forward/reverse/either facilities, adjacent connectors,
  permissions and prohibitions, multiple clearance bands, and stable ordering.
- No simulation, event, metric, or presentation behavior is introduced.

# Context

Gated on [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]].
Owns crates/tangle-model/src/compiled.rs, components.rs, mode_template.rs, and
focused model compilation tests. Reuse CompiledReferencePath and
CompiledFacility geometry from TAS-074; do not add a navigation mesh.

# Result

`crates/tangle-model` compiles every Increment 2 policy shape fixed by
*Compiled semantics* in `docs/schema-v2-contract.md` into identifier-resolved
model data the controller stages read without a source-string lookup. A document
that authors no Increment 2 policy compiles to exactly the Increment 1 behaviour:
no adjacency, no permission, no band, no maneuver policy, no mode lateral
policy.

**Resolved shapes** (`crates/tangle-model/src/compiled.rs`):

- `CompiledPermission` (`PermissionId`, `name`, `kind`, `holder: ModeTemplateId`,
  `target: PermissionTarget` = `Facility`/`Movement`/`Crossing`/`Undeclared`,
  `effect`), one entry per authored statement in source order, plus
  `CompiledScenario::permission_effect(kind, holder, target)`,
  `crossing_permission(mode, crossing)`, `permission(id)`, and the `IdMap`
  permission names. `stop_service` targets a `bus_stop` (Increment 4) and any id
  no declared object supplies resolve to `Undeclared`, which matches no
  traversal and fixes no effect — the compiler stays total where validation
  ([[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]]) owns rejection.
- `CompiledClearanceBand` (`ClearanceBandId`, `name`, `threshold_m`, `violation`,
  `applies_to_modes: Option<Vec<ModeTemplateId>>`) in declaration order, with
  `applies_to(mode)` (`None` applies to every mode pair) and the `IdMap` band
  names. A mode id no template supplies is dropped from the list, so the band
  matches nothing through it.
- `CompiledFacilityAdjacency` (`FacilityAdjacencyId`, `name`, `first`, `second`,
  authored `side`, and the resolved transition of each band and direction), with
  `transition(facility, direction) -> Option<LateralTransition>`.
  `LateralTransition` names the destination `FacilityTraversal` whose reference
  tangent agrees with the agent's travel (non-negative dot product at the first
  band's reference midpoint projected onto the second) and the crossing `side`
  in the agent's own travel frame (authored side for forward travel, opposite
  for reverse, and flipped on the second band exactly when the continuation
  agrees with the first band's reference direction).
- `TraversalTransitions` per facility and direction: the connectors that leave
  that traversal's end (the Increment 1 `outgoing_connectors`, unchanged) and the
  lateral targets of the facility's adjacencies, both in authored order, read
  through `CompiledScenario::transitions(facility, direction)`.
- `DirectionSet` (ordered set over `MovementDirection`) and `FacilityTraversalPolicy`
  from `CompiledScenario::traversal_policy(mode, facility, movement)` for an
  eligible body: `nominal_directions`, `permitted_directions`,
  `physically_possible_directions`, `traversable_directions`
  (= permitted ∩ physically possible), `nominal_effect`, `usable_interval`,
  `lateral_use`, `passing_side`, `lane_use`, and `overtake`.
- `CompiledFacility::passing_side()` from `facilities[].lateral_policy`, and
  `physically_possible_directions` now also carries a direction whose lateral
  continuation is possible on the adjacent facility.
- `CompiledScenario::maneuver_policy()`, `commit_policy()`, and
  `wrong_way_policy()`: scenario-scoped policy with no id to resolve, carried
  verbatim like `population`, `None` when unauthored.

**Resolution semantics.** The nominal, permitted, and physically possible sets
stay separate. Nominal is the authored facility `nominal_direction`; permitted is
the mode's own `AgentAccess::nominal_direction` restriction intersected with the
applicable `nominal_direction` statement (`permit` adds the opposing direction,
`prohibit` and no statement leave the nominal direction — both on an `either`
facility — and `obligate` leaves only the opposing direction); physically
possible is the connector graph plus a lateral adjacency whose continuation
direction is possible on the adjacent facility, computed from a single snapshot
so the result does not depend on adjacency order. A `permit` or `obligate` whose
opposing direction the topology does not connect is inert, so a permission never
widens physical possibility, and `traversable_directions` is the set a controller
may actually route. A movement-targeted statement decides over a
facility-targeted one when the traversal carries that movement. The usable
interval is `CompiledFacility::usable_lateral_interval` at the mode's own body
envelope width and preferred lateral clearance, both read from the compiled
bundle; the interval is the same at every arc length because the authored band
width is constant. A destination traversal the mode is not permitted on stays a
transition target: the caller reads the destination's permitted set to record a
forbidden boundary rather than dropping the crossing.

**Mode lateral policy** (`crates/tangle-model/src/mode_template.rs`):
`CompiledLateralPolicy` (`target_clearance_m`, `horizon_s`) attached to
`CompiledModeTemplate` by `with_lateral` and read through `lateral()`; a template
that authors no `lateral` compiles to `None` and keeps the Increment 1 capability
set and behaviour. `CompiledModeTemplate::envelope_width_m()` and
`lateral_clearance_m()` expose the two compiled facts the usable interval reads.
The bounded-steering limits stay where the contract puts them: the profile's
`steering_rate_max_rad_s` and the new `lateral_accel_max_mps2`
(`AgentBehaviorProfile` in `crates/tangle-model/src/components.rs`, attached by
`with_lateral_accel_max_mps2`), so no limit is stored twice.

**Tests**: new `crates/tangle-model/tests/increment2_compiled.rs` (15 tests) over
three fixtures: forward, reverse, and `either` facilities; an `either` facility
no adjacency reaches (proximity is never inferred); an adjacent pair authored in
opposite world directions (the side and continuation mapping in both travel
frames); a facility whose only connected direction comes from an adjacency; a
crossing onto a direction the destination does not permit; permit, prohibit, and
obligate effects, a movement-targeted statement deciding over a facility-targeted
one, lane-use, overtake, and crossing policy; three clearance bands in declaration
order with resolved `applies_to_modes`; the maneuver policy; the resolved ids in
the `IdMap`; the Increment 1 behaviour of both an Increment 1 version-2 document
and a version-1 document; and a document whose Increment 2 references no declared
object supplies, which must resolve to a `Result` rather than a panic. One test
drives `compile_mode_template` directly to prove the `lateral_accel_max_mps2`
profile parameter compiles, since `validate_v2` does not yet accept it (see
*Remaining scope*).

**Acceptance**:

- `cargo test -p tangle-model` — 158 passed, 0 failed (75 lib, then
  agent_components 4, facility_validation 20, fuzz_scenarios 3,
  increment2_compiled 15, increment2_source 4, migration 8,
  mode_template_compilation 10, narrow_mode_templates 8,
  synthetic_mode_template 4, version2_source 6, walking_scenario 1). The 143
  pre-existing tests are unchanged.
- `cargo check --workspace --all-targets` — clean; the additive compiled shape
  breaks no consumer, so no path outside `tangle-model` needed a closure edit.
- `cargo clippy -p tangle-model --all-targets` — clean, no warnings.
- `cargo fmt --all --check` — clean.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `braintree check` — `graph check: passed (154 nodes)` while this node was still
  in `proposed/`. Post-move `braintree check --allow-pending-advance TAS-082` —
  `graph check: passed (154 nodes)`; plain `braintree check` then reports only
  that [[TAS-082-increment-2-authored-and-compiled-contract]]'s `next` names this
  already-resolved node, which is the parent advance the coordinator owns.

No simulation, event, metric, or presentation behaviour was introduced: the leaf
adds compiled data, accessors, and tests in `tangle-model` only. No new
dependency, and no filesystem, wall-clock, or UI type enters the kernel.

Accessors the Increment 2 leaves will read:
`CompiledScenario::traversal_policy` and `CompiledModeTemplate::lateral` by
[[TAS-087-continuous-lateral-motion-and-gap-machinery]] and
[[TAS-088-add-route-relative-lateral-agent-state]]; `CompiledLateralPolicy` plus
the profile's `steering_rate_max_rad_s` and `lateral_accel_max_mps2` by
[[TAS-089-integrate-bounded-single-body-steering]]; `transitions` and
`permitted_directions` by
[[TAS-090-predict-maneuver-corridors-and-clearance]] and
[[TAS-091-resolve-gap-claims-and-maneuver-transitions]]; `passing_side`,
`lane_use`, `overtake`, and `LateralTransition::side` by
[[TAS-093-enable-same-facility-narrow-user-passing]],
[[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]], and
[[TAS-095-complete-lane-transitions-and-safe-aborts]]; `nominal_directions`,
`nominal_effect`, `physically_possible_directions`, `permission_effect`, and
`wrong_way_policy` by [[TAS-096-contextual-wrong-way-travel]] and
[[TAS-097-make-contextual-wrong-way-decisions-reproducible]]; and
`ClearanceBandId`, `clearance_bands`, and `PassingSide` by
[[TAS-100-version-the-maneuver-event-and-trace-surface]] and
[[TAS-101-measure-close-passes-with-exact-clearance-evidence]].

**Remaining scope** (recorded, not deferred to a new node):

- `lateral_accel_max_mps2` is compiled into the behavior profile when authored,
  but `validate_v2` still rejects it for a family that
  `required_profile_params` (`crates/tangle-model/src/validate.rs`) does not list,
  so no validated document can author it yet. Extending that seam for
  lateral-capable modes is [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]]'s write set; no new node was created for
  it.
- Compiling a *movement* traversal's own permitted directions (the version-2
  `movements[].direction` with movement-targeted statements) belongs to the
  wrong-way leaf; this leaf resolves statements whose target is a movement and
  applies the narrower-object rule when a facility traversal carries one.
- Corridors (usable and predicted) and the four clearance facts are
  state-dependent and stay with [[TAS-090-predict-maneuver-corridors-and-clearance]]; this leaf compiles the interval and
  the policy they are computed against.

**Friction** (for the session FBK; no FBK node was created):

- Attempted: compile the mode's bounded-steering limits exactly where the
  contract lists them. Friction: the contract requires
  `mode_templates[].profiles.lateral_accel_max_mps2` "exactly when `lateral` is
  present", but that requirement lives in `required_profile_params`
  (`validate.rs`, [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]]'s file), so `validate_v2` currently *rejects* a
  template that authors it as an unused family parameter. Requiring it in the
  compiler instead would make `compile_v2` reject documents validation accepts
  today. Improvement: state, in the contract or the tasking, which leaf owns an
  additive profile parameter whose presence rule is validation-side, so the
  compiler and the validator never disagree about a required field.
- Attempted: keep the decision of whether a lateral crossing is forbidden in the
  compiled traversal policy. Friction: the contract fixes "a destination whose
  effective direction does not permit that traversal makes the crossing a
  forbidden boundary", which is a property of the *destination* traversal and
  cannot be precomputed per mode into a mode-independent adjacency record without
  duplicating the permitted-direction resolution. Improvement: the contract could
  name the compiled accessor pair (target list plus destination permitted set)
  rather than describing the rule only in terms of the runtime decision.

