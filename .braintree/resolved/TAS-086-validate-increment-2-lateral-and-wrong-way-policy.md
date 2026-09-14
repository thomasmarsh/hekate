---
context_rev: 1
priority: P1
updated: 2026-09-14T01:26:26Z
summary: Validate Increment 2 lateral, clearance, transition, and wrong-way policy.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

Scenario validation rejects unsafe, dangling, contradictory, or physically
impossible Increment 2 policy before simulation and reports stable actionable
diagnostics.

# Done when

- Validation covers finite positive horizons, non-negative clearances,
  increasing/non-overlapping clearance bands, motion-limit feasibility, usable
  width, connector continuity, holder/target kinds, tactic capability, and
  physically connected reverse traversal.
- It rejects transitions across disjoint facilities, policies whose target body
  cannot fit, overtake permission with no capable holder, and contradictory
  effects at equal specificity. A contextual wrong-way `nominal_direction`
  statement with no opposing path is **not** rejected: the contract makes such a
  `permit`/`obligate` inert — the permitted set never leaves the physically
  possible set — and it is closed at runtime by the wrong-way decision's
  `no_opposing_path` reason. (Reconciled from "and contextual wrong-way
  permission with no opposing path"; see *Reconciled criterion* below.)
- Legal prohibition remains distinct from physical impossibility in diagnostic
  codes and messages; a prohibited but connected traversal remains available as
  violation context.
- Table-driven tests assert stable paths/codes for each failure and show that
  malformed inputs never panic.
- All Increment 1 valid fixtures continue to validate unchanged.

# Context

Gated on [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]] and
[[TAS-085-compile-increment-2-policy-and-traversal-semantics]]. Owns
crates/tangle-model/src/validate.rs and focused facility/policy validation tests.
Do not implement tactical runtime decisions.

# Result

`validate_v2` now enforces the Increment 2 lateral, clearance, transition, and
wrong-way rules fixed by `docs/schema-v2-contract.md` under *Increment 2
additions* → *`mode_templates[]` — lateral capabilities and maneuver
parameters* / *`facilities[]` — lateral-use policy* / *`facility_adjacencies[]`*
/ *`clearance_bands[]`* / *`maneuver_policy`* / the `permissions[]` resolution
rules. Every rule has a stable `DiagnosticCode` and a table-driven rejecting
case in the new `crates/tangle-model/tests/increment2_validation.rs` whose base
document is the accepting case. Validation stays consistent with the compiled
types TAS-085 landed (`FacilityTraversalPolicy`, `DirectionSet`,
`UsableLateralInterval`, `CompiledFacilityAdjacency`); it never re-derives
compiled policy and never falls back to a parser or a Phase 1 default.

## Rules, diagnostics, and rejecting fixtures

| Rule | Diagnostic | Rejecting case |
| --- | --- | --- |
| `lateral` requires wheeled motion and a lateral tactic | `E_MODE_TEMPLATE_LATERAL` | `lateral without a lateral tactic`, `lateral on a walking template` |
| Lateral target clearance finite and non-negative | `E_MODE_LATERAL_CLEARANCE` | `lateral clearance negative`, `… NaN` |
| Lateral horizon finite and strictly positive | `E_MODE_LATERAL_HORIZON` | `lateral horizon zero`, `… NaN` |
| A lateral tactic requires `maneuver_policy` | `E_MANEUVER_POLICY_MISSING` | `lateral tactic without maneuver policy` |
| `reverse_direction` requires `maneuver_policy.wrong_way` | `E_WRONG_WAY_POLICY_MISSING` | `reverse_direction without a wrong-way policy` |
| Lateral wheeled profile set through the shared seam | `E_MODE_TEMPLATE_PROFILE` | `lateral mode missing a required lateral profile` |
| Commit floor/timeout well-formed | `E_COMMIT_POLICY` | `commit clearance floor negative`, `commit hold timeout zero` |
| Commit floor at most every lateral target clearance | `E_COMMIT_CLEARANCE` | `commit floor above the target clearance` |
| Wrong-way thresholds well-formed | `E_WRONG_WAY_POLICY` | `wrong-way urgency above one`, `wrong-way density negative` |
| Band threshold finite and strictly positive | `E_CLEARANCE_BAND_THRESHOLD` | `clearance band threshold zero`, `… NaN` |
| Bands strictly increasing, no shared threshold | `E_CLEARANCE_BAND_ORDER` | `clearance bands out of order` |
| `applies_to_modes` present and non-empty | `E_CLEARANCE_BAND_MODES_EMPTY` | `clearance band empty mode list` |
| `applies_to_modes` names declared modes | `E_CLEARANCE_BAND_UNKNOWN_MODE` | `clearance band unknown mode` |
| `lateral_policy` requires a reference path | `E_FACILITY_LATERAL_WITHOUT_REFERENCE` | `lateral policy without a reference path` |
| `lateral_policy` is incompatible with `centered` | `E_FACILITY_LATERAL_CENTERED` | `centered facility with a lateral policy` |
| A passing side an eligible body can occupy | `E_FACILITY_PASSING_SIDE_UNUSABLE` | `passing side no body can occupy` |
| Adjacency names declared facilities | `E_FACILITY_ADJACENCY_UNKNOWN_FACILITY` | `adjacency to an undeclared facility` |
| Adjacency joins distinct facilities | `E_FACILITY_ADJACENCY_SELF` | `adjacency joining a facility to itself` |
| Adjacency attaches reference bands | `E_FACILITY_ADJACENCY_WITHOUT_REFERENCE` | `adjacency without a reference path` |
| Adjacency bands touch along a positive-length shared boundary | `E_FACILITY_ADJACENCY_DISJOINT` | `adjacency across disjoint bands` |
| Adjacency bands touch on the declared side | `E_FACILITY_ADJACENCY_SIDE` | `adjacency on the wrong side` |
| Permission `kind` fixes its target object kind | `E_PERMISSION_TARGET_KIND` | `permission kind disagrees with a movement/facility target` |
| One statement per `(kind, holder, target)` | `E_PERMISSION_EFFECT_CONFLICT` | `permission repeats a specificity with another effect/identically` |
| `overtake` holder declares the tactic | `E_PERMISSION_OVERTAKE_CAPABILITY` | `overtake permission without a capable holder` |
| `lane_use` obligation needs a fixed passing side | `E_PERMISSION_LANE_USE_OBLIGATION` | `lane-use obligation without a lateral policy`, `… on most_clearance` |
| A nominal statement needs an opposite direction | `E_PERMISSION_NOMINAL_EITHER` | `nominal statement about an either facility` |
| Permission holder declared | `E_PERMISSION_UNKNOWN_HOLDER` | `permission holder undeclared` |
| Permission target declared | `E_PERMISSION_UNKNOWN_TARGET` | `permission target undeclared` |
| Illegal route vs physically impossible route | `E_PERMISSION_ROUTE_PROHIBITED` | `legal_prohibition_stays_distinct_from_physical_impossibility` |

Malformed structural inputs (a band `threshold_m` string, an adjacency object,
an unknown passing side, a non-numeric `urgency`) fail to parse rather than
panic (`malformed_structural_inputs_fail_to_parse_not_panic`); NaN/infinite
numbers set directly on a parsed source are rejected with their code rather than
panicking.

## Physically connected reverse traversal

The first bullet's "physically connected reverse traversal" is covered by the
two direction tests over `FORWARD_ONLY`/`BOTH_WAYS`:

- `an_unconnected_opposing_statement_is_inert_for_the_traversal` — `west` is
  connected only forward, so no opposing traversal exists. A `permit` and an
  `obligate` statement both validate cleanly, and the compiled policy keeps
  `permitted ⊆ physically_possible` (`permitted == FORWARD`), which is exactly
  the precondition the runtime wrong-way decision reports as `no_opposing_path`.
- `a_connected_opposing_statement_widens_the_permitted_direction` — with the
  reverse connector present the opposing traversal is physically possible, so a
  `permit` widens the permitted set to `BOTH` and an `obligate` leaves only
  `REVERSE`.

`a_connected_but_unpermitted_traversal_stays_physically_possible` proves a
permitted/physically-possible traversal is kept as violation context, and
`legal_prohibition_stays_distinct_from_physical_impossibility` proves a
contradictory `prohibit` reports `E_PERMISSION_ROUTE_PROHIBITED` while no
physical-impossibility code (`E_FACILITY_TOO_NARROW`, `E_FACILITY_CURVATURE`,
`E_FACILITY_UNREACHABLE_DIRECTION`) fires.

## Reconciled criterion

The second Done-when bullet originally read that validation "rejects …
contextual wrong-way permission with no opposing path". The authoritative
`docs/schema-v2-contract.md` contradicts that premise under *`permissions[]`
resolution rules*:

> A `nominal_direction` statement with effect `permit` or `obligate` whose target
> object has no physically possible opposing traversal cannot be realized: the
> permitted set never leaves the physically possible set, so the statement is
> **inert** for that traversal … It is therefore *not* a validation failure (an
> `obligate` on a forward-only facility validates cleanly, as Increment 1 fixed),
> and the case is instead closed by the wrong-way decision's first precondition:
> with no opposing traversal the decision's reason is `no_opposing_path`.

This is a factual correction, not a reinterpretation: the rejected behaviour
would invalidate the accepted Increment 1 fixture
`distinguishes_an_illegal_route_from_a_physically_impossible_one` (its
`obligate` on a forward-only facility asserts `validate` returns no
diagnostics). The bullet now states the contract-faithful criterion, and
`an_unconnected_opposing_statement_is_inert_for_the_traversal` is the evidence.
No other Done-when wording changed and no scope was added.

## Shared `required_profile_params` seam

`required_profile_params` (`validate.rs`) gained a `lateral: bool` argument so
the contract's lateral wheeled profile set has the single definition both
`validate_mode_templates` and the template compiler read: a `lateral`-declaring
`single_body_wheeled` template requires `steering_rate_max_rad_s`,
`lateral_accel_max_mps2`, and `lateral_clearance_m` (plus the Increment 0 set);
a capsule wheeled template without `lateral` keeps its Increment 1 set. The call
site at `crates/tangle-model/src/mode_template.rs` was updated to pass
`template.lateral.is_some()`; the compiler's diagnostic code/message shape is
unchanged and no compilation semantics changed beyond the required set.

## Golden fixture correction

TAS-085's `crates/tangle-model/tests/increment2_compiled.rs` fixture predates
these rules and authored three facts the contract now rejects: a `lateral`
template without `lateral_accel_max_mps2`, a `lane_use` `obligate` on a
`most_clearance` facility, and an `overtake` statement whose holder declared
`pass` but not `overtake`. Because `compile_v2` runs `validate_v2`, the fixture
was corrected to stay valid — `lateral_accel_max_mps2` added to the `bicycle`
and `scooter` profiles, `overtake` added to the `scooter` tactics,
`bikeway_eastbound` `passing_side` changed to `left`, and its one
`passing_side` assertion updated. No assertion was weakened and no other
compiled test changed.

## Files changed

`crates/tangle-model/src/validate.rs` (codes, rules, shared
`required_profile_params`), `crates/tangle-model/src/mode_template.rs` (seam
call site), `crates/tangle-model/tests/increment2_compiled.rs` (golden fixture
correction), and the new `crates/tangle-model/tests/increment2_validation.rs`.
No `compiled.rs`/`components.rs` semantics, `docs/schema-v2-contract.md`,
`docs/benchmark-matrix.*`, `tangle-sim/**`, schema, scenario, baseline, or
golden-trace file changed; `validate.rs`'s own `#[cfg(test)]` module and every
existing validation test are unchanged.

## Residual risk

- A `lateral`-declaring `box` wheeled template now requires
  `steering_rate_max_rad_s` and `lateral_clearance_m`, but `compiled_profile`'s
  non-capsule wheeled arm builds `AgentBehaviorProfile::wheeled`, which carries
  neither. No fixture authors a box wheeled template with `lateral`, so this is
  latent; carrying those two limits for a non-capsule lateral mode is TAS-085's
  compiled-profile gap and needs a `components.rs` builder (a follow-up, not
  this leaf).
- The adjacency geometry rule compares the two facilities' region polygons for a
  collinear shared boundary of positive length and fixes the side from `first`'s
  reference frame. A pair of bands that touch along a deliberately non-collinear
  boundary would be rejected; no such shape exists in the contract's examples
  and the parent fixture passes both adjacencies.

## Acceptance

- `cargo test -p tangle-model` — passes (167 tests, including 9 new
  `increment2_validation` and the 20 unchanged `facility_validation`).
- `cargo test --workspace` — passes (0 failures).
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `cargo fmt -p tangle-model -- --check` and
  `cargo clippy -p tangle-model --all-targets` — clean.
- `braintree check` — passed (154 nodes) while this node was still `proposed/`;
  after the move, `braintree check --allow-pending-advance TAS-082` is the
  expected gate because [[TAS-082-increment-2-authored-and-compiled-contract]]
  still names TAS-086 in its `next`.

## Friction

- **Attempted**: extend the shared `required_profile_params` seam and add the
  Increment 2 rules while keeping every existing fixture valid. **Friction**: the
  seam's signature is keyed only on `(body, motion)` and cannot see the template's
  `lateral` object, so the contract's "lateral wheeled profile set … read through
  the existing `required_profile_params` seam" cannot be met without changing
  that signature; a `bool` was the least invasive extension. Additionally,
  TAS-085's compiled fixture silently predated this leaf's rules, so enforcing
  the contract forced a golden-fixture correction outside the leaf's stated
  primary file. **Improvement**: `SKILL.md` could say how a validation leaf owns
  coercing an earlier leaf's golden fixtures when the contract tightens, and the
  contract could name the seam's full argument list rather than only its
  identifier.
