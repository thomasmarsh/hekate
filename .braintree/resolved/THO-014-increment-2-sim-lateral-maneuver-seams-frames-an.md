---
context_rev: 1
updated: 2026-09-14T13:24:44Z
summary: Increment 2 sim lateral-maneuver seams, frames, and test gates.
---

Area [[IDX-001-tangle]].

# Scope

Shared, opt-in reconnaissance for the Increment 2 lateral-maneuver work in
`crates/tangle-sim` (TAS-095 and its children, TAS-105, TAS-106). Link it by
plain wikilink; load it on demand rather than restating it. Verified at commit
35d8030.

# Seams

- `crates/tangle-sim/src/sim.rs` (~6,300 lines; prefer `path (Symbol)`):
  - `Simulation::predict_maneuver`, `predict_outbound`, `predict_candidate`
    (~1629-1720): within-facility and cross-facility predictors.
  - `attempt_maneuver` (~1774), `attempt_facility_transition` (~1805).
  - `committed_plan` (~1875), `abort_plan` (~2052), `passed_body_cleared`.
  - `handoff_connector` (~2960), `facility_adjacency` (~3000),
    `crossing_lateral` (~3030), `handoff_lateral` (~3135).
  - `CrossingLateral` (~3369): `boundary_offset_m`, `band_bounds_m`,
    `crossing_m()`, `target_offset_m(clearance)`.
  - `lateral_request` (~3258), `committed_lateral_target`, `steering_envelope`.
- `crates/tangle-sim/src/prediction.rs`: `predict_maneuver_corridor` (~270),
  `predict_crossing_corridor`, `ManeuverInputs`, `ManeuverPrediction`,
  `PredictedClearances`, `ClearanceFact`.
- `crates/tangle-sim/src/agent.rs`: `RouteState` fields `maneuver`,
  `target_offset_m`, `target_facility`, `pre_maneuver_offset_m`,
  `bounded_steering`, `return_blocked`, `entering_facility`, `hold_since`,
  `settled_since`.
- `crates/tangle-sim/src/stage.rs`: `ManeuverState`, `ManeuverEdge`,
  `ManeuverTransition`, `FacilityTransitionRecord`, `LateralManeuverRequest`,
  `ManeuverAbortReason`, `PassSide`.

# Traps

- Heading frames (FBK-031 Finding 4): `AgentStore.heading_rad` stores the
  reference-tangent heading; the corridor and steering use the travel heading.
  A predictor that rejects every candidate for one travel direction is a frame
  bug, not a geometry bug.
- Mixed lateral capability (FBK-031 Finding 7): a v2 scenario mixing a
  lateral-capable and a lateral-incapable mode on one facility once panicked
  `resolve_maneuvers`; keep
  `a_lateral_incapable_mode_on_a_shared_facility_never_maneuvers` as an input.

# Gates

Focused: `cargo test -p tangle-sim` (358 tests at commit 35d8030). Node
resolution: `cargo test --workspace` (177 s), `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `cargo fmt --all --check`,
`scripts/check-dependency-direction.sh`.
