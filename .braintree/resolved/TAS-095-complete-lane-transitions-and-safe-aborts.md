---
context_rev: 1
priority: P1
updated: 2026-09-14T15:01:12Z
summary: Complete configured lane or facility transitions with bounded abort, braking, and return.
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
Owns facility-transition integration, safe-abort behavior, and focused tests.
Do not define event payloads, close-pass aggregation, or acceptance scenarios.

# Result

Complete. Both facility handoffs, the forbidden-boundary fact, the compiled
shared-boundary crossing, the cross-facility committed hazard matrix, and the
return-obstruction policy landed with a focused suite; the cross-facility
return leg landed in
[[TAS-113-return-a-cross-facility-change-of-lane-to-the-so]] (one crossing out
and one back over the same compiled adjacency), and the current+destination
leader/follower constraints landed in
[[TAS-114-keep-destination-leader-follower-constraints-act]].

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

## This session (continued): return-leg obstruction

Files: `crates/tangle-sim/src/sim.rs`, `agent.rs`;
`crates/tangle-sim/tests/lane_transitions.rs` (11 -> 12 tests) plus two crate
unit tests. No version or artifact changed.

- **Return obstruction** (`RouteState.return_blocked`,
  `Simulation::return_leg_obstructed`): while a maneuver is `returning` or
  `aborted`, the return target (`pre_maneuver_offset_m`) is evaluated with the
  ordinary predictor over the compiled usable interval; a body whose progress
  interval overlaps the return corridor leaves the swept clearance below the
  mode's target clearance, so the agent holds the offset it occupies instead of
  steering into it, and settles to `following` only once the corridor clears.
  Unit tests `a_blocked_return_leg_holds_and_re_decides_until_the_corridor_clears`
  and `a_blocked_aborted_leg_holds_until_the_corridor_clears`, and integration
  test `a_returning_riders_obstructed_target_holds`.

## Previously landed (prior sessions)

Files: `crates/tangle-sim/src/sim.rs`, `prediction.rs`, `agent.rs`, `lib.rs`;
`crates/tangle-sim/tests/lane_transitions.rs`. No event or trajectory version
changed, and no golden, baseline, or schema was regenerated.

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
  body aborts it (`a_destination_band_body_aborts_the_outbound_leg`). Return
  obstruction holds and re-decides
  (`a_returning_riders_obstructed_target_holds`, plus the two crate unit tests).
- Forbidden boundary:
  `a_forbidden_lane_change_is_prevented_with_the_boundary_reason`.
- Focused suite: adjacent-lane change, connector handoff, forbidden crossing,
  complete-and-return (`an_eligible_maneuver_completes_and_returns_to_following`),
  no-change without authored connectivity or lateral capability, and the
  mixed-capability adversarial input.

## Final slices

- **Cross-facility return leg**: landed by
  [[TAS-113-return-a-cross-facility-change-of-lane-to-the-so]].
- **Current and destination constraints together**: landed by
  [[TAS-114-keep-destination-leader-follower-constraints-act]].

## Resolution

TAS-113 (cross-facility return crossing) and TAS-114 (destination
leader/follower constraints) complete the remaining scope. Final gate on the
integrated tree: `cargo test -p tangle-sim --test lane_transitions` (18 passed),
`cargo test --workspace` (875 passed, 0 failed), `cargo clippy --workspace
--all-targets --all-features -- -D warnings` (clean), `cargo fmt --all --check`
(clean), and `scripts/check-dependency-direction.sh` (`dependency direction OK`).
All Done-when clauses are met and every child is resolved.

## Validation

`cargo test -p tangle-sim` (192 lib + 12 `lane_transitions` + all suites, 361
total), `cargo test --workspace` (869 passed, 0 failed),
`cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean),
`cargo fmt --all --check` (clean), and `scripts/check-dependency-direction.sh`
(`dependency direction OK`) all pass.

# Slices

- [[TAS-113-return-a-cross-facility-change-of-lane-to-the-so]] Return crossing to the source band.
- [[TAS-114-keep-destination-leader-follower-constraints-act]] Destination leader/follower constraints.
