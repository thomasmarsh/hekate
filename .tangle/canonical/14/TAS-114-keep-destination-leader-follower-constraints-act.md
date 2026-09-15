---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Keep destination leader/follower constraints active through a handoff.
---

Parent [[TAS-095-complete-lane-transitions-and-safe-aborts]].

# Outcome

A committed facility transition keeps both the current and the destination
facility leader and follower constraints active through the handoff, so no
constraint gap opens across the ownership change and the ordinary spatial index
and collision scan see every intermediate world pose.

# Done when

- Leader and follower selection reads the destination facility before the handoff
  as well as the current one, so a body on either side constrains the maneuver.
- The per-tick spatial rebuild and collision scan are unchanged and see every
  intermediate pose; no transition creates an unconstrained step.
- Focused tests cover a leader and a follower on each side of the shared boundary
  during a committed change of lane.

# Context

Depends on [[TAS-112-compile-the-adjacency-shared-boundary-lateral-co]] at context_rev 1.
Reads the sim seams in [[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]].
Owns the sim-side leader and follower constraint selection; do not change the
authored schema or the maneuver lifecycle.

# Result

Complete. While a cross-facility maneuver is active — `preparing`, `committed`,
or `returning`/`aborted` — leader selection reads bodies on the far side of the
compiled crossing as well as on the band the agent rides, so a destination
leader constrains the approach before the handoff and no constraint gap opens
across the ownership change. The per-tick spatial index and collision scan are
unchanged and still see every intermediate world pose; the crossing's own
corridor prediction keeps the rear/side hazard response. Files:
`crates/hekate-sim/src/sim.rs`; `crates/hekate-sim/tests/lane_transitions.rs`.
No event, metric, trajectory, golden, baseline, or schema changed.

## Seams

- `crates/hekate-sim/src/sim.rs (Simulation::crossing_far_traversal)`: the
  compiled destination traversal of an active cross-facility maneuver
  (`preparing`/`committed` from `target_facility`, `returning`/`aborted` from
  `return_facility`), read through `Simulation::crossing_lateral`.
- `crates/hekate-sim/src/sim.rs (Simulation::nearest_leader)`: when a far
  traversal exists, a body on it is projected onto the ridden facility's
  reference and its gap measured in the ridden travel frame; a body alongside or
  overlapping is left to the crossing corridor rather than the anti-overlap cap,
  so the cap never freezes an agent for a body no longitudinal step can touch.

## Evidence

Focused tests in `crates/hekate-sim/tests/lane_transitions.rs` via the
`crossing_constraint_scenario` fixture:
`a_body_ahead_on_the_riders_own_band_slows_the_committed_change_of_lane`,
`a_body_ahead_on_the_destination_band_slows_the_committed_change_of_lane`
(control crosses at free flow),
`a_body_closing_from_behind_on_the_destination_band_holds_the_committed_change_of_lane`
(aborts with `ManeuverAbortReason::ClearanceLost`), and
`a_body_behind_on_the_riders_own_band_stays_its_follower_through_the_crossing`.

## Validation

`cargo test -p hekate-sim --test lane_transitions` (18 passed, 0 failed),
`cargo test --workspace` (875 passed, 0 failed), `cargo clippy --workspace
--all-targets --all-features -- -D warnings` (clean), `cargo fmt --all --check`
(clean), and `scripts/check-dependency-direction.sh` (`dependency direction OK`).
