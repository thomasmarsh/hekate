---
context_rev: 1
priority: P1
updated: 2026-09-14T14:19:54Z
summary: Return a cross-facility change of lane to the source band.
---

Parent [[TAS-095-complete-lane-transitions-and-safe-aborts]].

# Outcome

A committed cross-facility change of lane can return from the destination band to
the source band through the compiled shared boundary, steering back to the
source offset under the same compiled corridor and obstruction policy as an
ordinary return, with the world pose continuously authoritative.

# Done when

- The return crossing fires at the compiled shared boundary
  (`CompiledFacilityAdjacency::shared_boundary_offset`) and moves route and
  facility ownership in one step with no despawn, re-spawn, or snap.
- `returning` and `aborted` on a cross-facility change of lane detect an
  obstructing body through the ordinary predictor and hold or re-decide per the
  documented policy, exactly as `RouteState.return_blocked` already does for a
  same-facility maneuver.
- Focused tests cover a completed pass out into an adjacent band and back, and a
  return blocked by a body in the destination corridor.

# Context

Depends on [[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]] at context_rev 1.
Reads the sim seams in [[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]
and the compiled datum in
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the sim-side
return crossing and return-leg policy; do not change the authored schema or
redefine the maneuver lifecycle.

# Result

Complete. A committed cross-facility change of lane performs exactly one
crossing out and one crossing back over the same compiled adjacency, and its
return leg holds or re-decides on the ordinary compiled crossing-corridor
predictor. Files: `crates/tangle-sim/src/agent.rs`,
`crates/tangle-sim/src/sim.rs`, `crates/tangle-sim/tests/lane_transitions.rs`. No
event or trajectory version changed, and no golden, baseline, or schema was
regenerated.

## Seams

- **Source band** (`crates/tangle-sim/src/agent.rs (RouteState.return_facility)`):
  the band a crossed change of lane returns to, preserved across the outbound
  handoff that moves ownership to the destination band. `None` for a
  within-facility maneuver, for the outbound leg while the agent still rides the
  band it was attempted in, and again once the return crossing has moved
  ownership back — so the maneuver settles on the band it started from.
- **Return target** (`crates/tangle-sim/src/sim.rs (Simulation::return_offset_m)`):
  the offset the return leg steers to and settles at, in the agent's own current
  travel frame — the compiled crossing's own steering offset
  (`CrossingLateral::target_offset_m`) while the agent rides the destination
  band, and `RouteState.pre_maneuver_offset_m` otherwise.
- **Return crossing** (`crates/tangle-sim/src/sim.rs (Simulation::handoff_lateral)`):
  fires out for `ManeuverState::Committed` (the adjacency's destination
  traversal) and back for `ManeuverState::Returning`/`Aborted` (the preserved
  source band) over that one compiled adjacency, moving route and facility
  ownership in one step with the world pose unchanged — no despawn, re-spawn, or
  snap — and clearing `return_facility` on the return move so no further crossing
  is owed.
- **Return obstruction**
  (`crates/tangle-sim/src/sim.rs (Simulation::return_leg_obstructed)`): a crossed
  maneuver reads the compiled crossing corridor (`CrossingLateral::band_bounds_m`)
  through `Simulation::predict_candidate` instead of the ridden band's own
  constant-width interval, so a body anywhere in the swept corridor leaves the
  clearance below the mode's target and holds the return exactly as
  `RouteState.return_blocked` already holds a same-facility return.
- **Crossing envelope**
  (`crates/tangle-sim/src/sim.rs (Simulation::steering_envelope)`): widened on the
  crossing side for both the committed leg and the returning/aborted leg, so the
  bounded step reaches each crossing's own target.
- **Settle edge and request**
  (`crates/tangle-sim/src/sim.rs (Simulation::resolve_maneuvers)` settle phase,
  `crates/tangle-sim/src/sim.rs (Simulation::lateral_request)`): both read
  `Simulation::return_offset_m`, so the leg steers to the crossing offset and
  completes only once that offset has held.

## Evidence

Focused tests in `crates/tangle-sim/tests/lane_transitions.rs`:
`a_cross_facility_change_of_lane_returns_over_the_shared_boundary` covers a
completed pass out into the adjacent band and back — exactly two lateral records,
both at the compiled shared boundary (1.0 m in the source band's frame and -2.0 m
in the destination band's frame, neither band's 1.5 m half-width), the whole
lifecycle through to `Following`, a bounded step across both handoffs, the rider
back on the source band's reference, and zero emergency cap steps.
`a_cross_facility_returns_obstructed_corridor_holds_and_re_decides` covers a
return blocked by a body in the destination corridor: the returning leg holds the
offset it occupies for 20+ steps while the body is inside the crossing corridor's
reach, then crosses back once the corridor clears, with bounded steps and no cap
step.

## Validation

`cargo test -p tangle-sim --test lane_transitions` (14 passed, 0 failed),
`cargo test -p tangle-sim` (all suites, 0 failed),
`cargo test --workspace` (871 passed, 0 failed),
`cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean),
`cargo fmt --all --check` (clean), and `scripts/check-dependency-direction.sh`
(`dependency direction OK`).
