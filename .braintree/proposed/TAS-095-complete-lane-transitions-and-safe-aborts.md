---
context_rev: 1
priority: P1
updated: 2026-09-14T11:19:57Z
summary: Complete configured lane or facility transitions with bounded abort, braking, and return.
next: Apply the committed hazard and return-obstruction policy on a change of lane's outbound and return legs.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

Eligible wheeled agents can transition through authored adjacent connectors
around a slower leader, while every infeasible or degraded maneuver produces a
bounded hold, brake, abort, return, or explicit forbidden-boundary fact.

# Done when

- A transition updates route/facility ownership only at the contract-defined
  geometric handoff, preserves stable progress, and never despawns/re-spawns or
  snaps to the destination reference.
- Current and destination leader/follower constraints remain active and the
  ordinary spatial index and collision scan see every intermediate world pose.
- Preparing loss, committed front hazard, committed rear hazard, target
  disappearance, boundary closure, and return obstruction each follow the
  documented deterministic policy within motion limits.
- Crossing a prohibited boundary is prevented when avoidable and exposes the
  violation fact TAS-100 will record when unavoidable under the policy.
- Focused tests cover adjacent-lane change, connector handoff, each abort case,
  complete-and-return, and no-change behavior without authored capability.

# Context

Gated on [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]] and
[[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]]. Owns
facility-transition integration, safe-abort behavior, and focused tests. Do not
define event payloads, close-pass aggregation, or acceptance scenarios.

# Result

Partial: both facility handoffs, the forbidden-boundary fact, and the focused
suite landed; the change of lane's full hazard matrix, its return leg, and the
return-obstruction policy do not fit this session, so TAS-095 stays `active` with
the `next` above.

## Landed

Files: `crates/tangle-sim/src/sim.rs`, `stage.rs`, `agent.rs`, `lib.rs`;
`crates/tangle-sim/tests/lane_transitions.rs` (new, 7 tests);
`crates/tangle-sim/tests/maneuver_lifecycle.rs` (compile-forced closure: the new
`LateralManeuverRequest::target_facility` field). No public event is emitted
(TAS-100 owns that).

- **Connector handoff** (`Simulation::handoff_connector`): at the compiled
  connector coincidence (`CONNECTOR_CONTINUITY_TOLERANCE_M`) route and facility
  ownership move to the connector's `to` traversal in one step. The world pose is
  unchanged — no despawn, no re-spawn, no snap — and the destination progress is
  the projection of that same pose onto the destination reference. The record's
  `permitted` is the destination traversal policy's verdict, so a destination the
  mode may not traverse is the forbidden-boundary fact with `permitted: false`.
- **Lateral handoff** (`Simulation::handoff_lateral`): a committed cross-facility
  change of lane (`LateralManeuverRequest::target_facility`) crosses the shared
  boundary between two side-by-side bands. It fires when the body centre reaches
  the source band's half-width on the crossing side, moves ownership to the
  adjacency's destination traversal, and preserves the pose and the projected
  progress. The outbound crossing is bounded by a runtime-derived crossing bound
  because `CompiledFacilityAdjacency` carries no band separation or offset (the
  TAS-090 gap); the entry transient uses the same runtime bound until the body
  centre is inside the destination's usable interval.
- **Boundary prevention** (`Simulation::attempt_facility_transition`): a
  destination traversal the applicable rule does not permit makes the claim
  infeasible with `ManeuverReason::BoundaryForbidden`, so the crossing is
  prevented and no `FacilityTransitionRecord` is produced.
- **Fact seam**: `FacilityTransitionRecord` and
  `StepOutput::facility_transitions()` expose each handoff once;
  `ManeuverAbortReason::BoundaryForbidden` and
  `FacilityTransitionRecord.permitted` carry the forbidden-boundary fact.

## Done-when coverage

- Geometric handoff, stable progress, no despawn/re-spawn/snap: met for both
  kinds (`lane_transitions.rs`
  `a_connector_hands_a_rider_off_at_its_coincidence`,
  `an_adjacent_lane_change_hands_the_rider_off_at_the_shared_boundary`).
- Constraints and the ordinary spatial index see every pose: the handoff never
  moves the world pose, the per-tick spatial rebuild and leader selection are
  unchanged, and the tests bound every step to the mode's speed.
- Abort cases: the crate unit tests cover preparing loss
  (`a_losing_claimant_aborts_with_the_rejected_reason`,
  `a_preparing_hold_timeout_aborts_the_maneuver`), committed front and rear
  hazards (`a_committed_maneuver_brakes_within_the_comfort_bound_and_holds`,
  `a_committed_maneuver_aborts_below_the_policy_minimum`), and target
  disappearance (`a_disappearing_target_aborts_the_maneuver`). Boundary closure
  appears as the infeasible destination in
  `a_destination_band_too_narrow_closes_the_crossing`.
- Forbidden boundary:
  `a_forbidden_lane_change_is_prevented_with_the_boundary_reason`.
- Focused suite: adjacent-lane change, connector handoff, forbidden crossing,
  complete-and-return (`an_eligible_maneuver_completes_and_returns_to_following`),
  and no-change without authored connectivity or lateral capability.

## Remaining scope (the `next`)

- **Cross-facility committed hazard matrix**: the outbound leg of a change of
  lane cannot read the within-facility predictor, because the destination band's
  offset from the source is not compiled. Until a compiled separation or offset
  exists, `committed_plan` cannot brake or abort an outbound leg on a predicted
  front, rear, or swept hazard, and a band-edge closure cannot abort it.
- **Return obstruction**: `returning` and `aborted` still steer only under the
  corridor bound; a body obstructing the return target is not detected, so the
  maneuver neither holds nor re-decides for it.
- **Cross-facility return leg**: the change of lane is outbound-only — the agent
  remains on the destination. A pass out into an adjacent band and back is not
  implemented.
- **Current and destination constraints together**: leader/follower selection
  reads the agent's current facility only; the destination's leaders and
  followers are not considered before the handoff.

## Validation

`cargo test -p tangle-sim` (187 lib + 7 `lane_transitions` + all suites),
`cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, and `scripts/check-dependency-direction.sh` all pass.
