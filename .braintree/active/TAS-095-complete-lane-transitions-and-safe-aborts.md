---
context_rev: 1
priority: P1
updated: 2026-09-14T12:23:57Z
summary: Complete configured lane or facility transitions with bounded abort, braking, and return.
next: Detect a body obstructing the return leg's target and hold or re-decide for it per the documented policy.
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

Depends on [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]] at context_rev 1.
Depends on [[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]] at context_rev 1.
Owns
facility-transition integration, safe-abort behavior, and focused tests. Do not
define event payloads, close-pass aggregation, or acceptance scenarios.

# Result

Partial: both facility handoffs, the forbidden-boundary fact, the compiled
shared-boundary crossing, and the cross-facility committed hazard matrix have
landed with a focused suite; the return leg, its obstruction policy, and the
current+destination leader/follower constraints do not fit this session, so
TAS-095 stays `active` with the `next` above.

## This session (TAS-112 unblocked the outbound hazard matrix)

Files: `crates/tangle-sim/src/sim.rs`, `prediction.rs`, `agent.rs`, `lib.rs`;
`crates/tangle-sim/tests/lane_transitions.rs` (7 -> 11 tests). No event or
trajectory version changed, and no golden, baseline, or schema was regenerated.

- **Compiled crossing** (`Simulation::facility_adjacency`,
  `Simulation::crossing_lateral`): reads
  `CompiledFacilityAdjacency::shared_boundary_offset` (TAS-112) for both bands so
  the destination band's constant-width band lands in the source band's travel
  frame. `CrossingLateral::boundary_offset_m` is the contract's lateral handoff
  point, and `band_bounds_m` is the two bands' combined edges with the shared
  boundary left open between them.
- **Committed hazard matrix** (`Simulation::predict_outbound` with
  `prediction::predict_crossing_corridor`): the outbound leg of a change of lane
  is predicted through the compiled crossing, so `committed_plan` brakes and
  aborts it on a predicted front, rear, or swept hazard and on a destination
  band-edge closure. The runtime `source.width_m() * 0.5` crossing bound is
  replaced by the compiled boundary in `attempt_facility_transition`,
  `committed_lateral_target`, and `steering_envelope`.
- **Adversarial mix**: `a_lateral_incapable_mode_on_a_shared_facility_never_maneuvers`
  runs a lateral-capable and a lateral-incapable mode on one shared facility
  (the `resolve_maneuvers` panic FBK-031 Finding 7 names).
- **Focused suite additions**: `the_handoff_fires_at_the_compiled_shared_boundary`,
  `a_closed_destination_band_edge_aborts_the_outbound_leg`,
  `a_destination_band_body_aborts_the_outbound_leg`.

## Previously landed (prior session)

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
  change of lane (`LateralManeuverRequest::target_facility`) crosses the compiled
  shared boundary between two side-by-side bands, moves ownership to the
  adjacency's destination traversal, and preserves the pose and the projected
  progress. The entry transient is bounded by the destination corridor widened to
  where the body centre is until it is inside the destination's usable interval.
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
  `the_handoff_fires_at_the_compiled_shared_boundary`).
- Constraints and the ordinary spatial index see every pose: the handoff never
  moves the world pose, the per-tick spatial rebuild and leader selection are
  unchanged, and the tests bound every step to the mode's speed.
- Abort cases: the crate unit tests cover preparing loss
  (`a_losing_claimant_aborts_with_the_rejected_reason`,
  `a_preparing_hold_timeout_aborts_the_maneuver`), committed front and rear
  hazards (`a_committed_maneuver_brakes_within_the_comfort_bound_and_holds`,
  `a_committed_maneuver_aborts_below_the_policy_minimum`), and target
  disappearance (`a_disappearing_target_aborts_the_maneuver`). Boundary closure
  now aborts the outbound leg
  (`a_closed_destination_band_edge_aborts_the_outbound_leg`) and a destination
  body aborts it (`a_destination_band_body_aborts_the_outbound_leg`).
- Forbidden boundary:
  `a_forbidden_lane_change_is_prevented_with_the_boundary_reason`.
- Focused suite: adjacent-lane change, connector handoff, forbidden crossing,
  complete-and-return (`an_eligible_maneuver_completes_and_returns_to_following`),
  no-change without authored connectivity or lateral capability, and the
  mixed-capability adversarial input.

## Remaining scope (the `next`)

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

`cargo test -p tangle-sim` (190 lib + 11 `lane_transitions` + all suites, 358
total), `cargo test --workspace` (866 passed, 0 failed),
`cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean),
`cargo fmt --all --check` (clean), and `scripts/check-dependency-direction.sh`
(`dependency direction OK`) all pass.
