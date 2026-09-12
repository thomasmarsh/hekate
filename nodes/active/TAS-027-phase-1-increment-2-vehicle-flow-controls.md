---
context_rev: 1
priority: P1
updated: 2026-09-12T16:15:31Z
summary: Phase 1 Increment 2 turns portals into demand, gives cars an IDM longitudinal controller with stop-line, signal, leader, queue, and exit behavior, and records reproducible contextual red-light decisions.
next: Add path-distance tracking and the documented IDM longitudinal controller over the sampled profiles.
---

# Outcome

Per `PHASE_1_PLAN.md` Increment 2, cars become demand-driven agents with
documented, reproducible longitudinal control:

- Portal demand generation, route assignment, physical and behavior profile
  sampling, and safe spawn admission.
- Path-distance tracking and a documented IDM-based longitudinal controller.
- Stop-line, signal, leader, following, queue, and exit behavior.
- A contextual red-light decision over signal state, distance, speed, urgency,
  and compliance profile.
- Decision-reason records visible in the inspector.

Increment 1 compiles the general scenario primitives; this increment adds
behavior over them. Pedestrian bodies and mixed interaction are Increment 3.

# Done when

- Cars obey acceleration, braking, and speed bounds and never overlap in the
  controlled car-following benchmark.
- Saturated demand produces stable queues rather than unbounded spawn overlap.
- Profile distributions and red-light decisions are reproducible and draw only
  from their named random streams (`demand`, `profile`, `compliance`,
  `perception`).
- Unit and scenario tests cover green/yellow/red boundaries and deterministic
  tie-breaking.
- Every state-affecting decision can be traced to a recorded decision reason.

# Context

Area [[IDX-001-tangle]].
Depends on [[TAS-026-phase-1-increment-1-general-scenario-foundation]] at context_rev 1.

Builds on Increment 1's compiled portals, guide paths, movements, rule, and
signal primitives, which this increment now drives. The pinned dependency is
resolved, so the edge records the primitives the demand and control slices
build over.

Follows `PHASE_1_PLAN.md` numeric choices: `f64` in the kernel, `glam::DVec2`,
portable ChaCha streams derived from the root seed and stable agent IDs, and a
single-threaded state-affecting tick. `PHASE_2_PLAN.md` owns schema version 2, so
no Phase 2 field is added here.

# Result

Slice A (portal demand, route assignment, profile sampling, safe spawn
admission) landed; the IDM controller, stop-line/signal/leader/queue/exit
behavior, red-light decisions, and inspector records remain.

- Schema version 1 gains optional `demand` and `profiles` collections in
  `crates/tangle-model/src/source.rs`; `validate.rs` adds `E_DEMAND_UNKNOWN_PORTAL`,
  `E_DEMAND_EMPTY_ROUTES`, `E_DEMAND_UNKNOWN_MOVEMENT`,
  `E_DEMAND_ROUTE_PORTAL_MISMATCH`, `E_DEMAND_DUPLICATE_ROUTE`, and
  `E_PROFILE_RANGE`, and `compiled.rs` compiles `CompiledDemand`,
  `CompiledRouteShare`, `CompiledProfile`, and `ProfileRange` behind `DemandId`
  and new `IdMap` demand names.
- `crates/tangle-sim/src/rng.rs` derives named portable ChaCha streams from the
  root seed: one `demand` substream per demand source and one `profile` substream
  per stable agent id. No concern shares one mutable generator.
- `crates/tangle-sim/src/demand.rs` generates arrivals (`rate_vph * dt`, with a
  Bernoulli draw for the fractional part), assigns a weighted route from the
  `demand` stream, and holds a bounded FIFO pending queue that sheds load
  (`MAX_PENDING_SPAWNS`) instead of growing without limit.
- `crates/tangle-sim/src/profile.rs` samples a stable `VehicleProfile` (desired
  speed, body size, time gap, acceleration, comfortable braking) from the
  `profile` stream at admission.
- `crates/tangle-sim/src/sim.rs` admits an arrival only when its entry body
  clears every live body on the entrance path (`MIN_SPAWN_CLEARANCE_M`), records
  the route and profile on the agent, and exposes `pending_arrivals`,
  `dropped_arrivals`, `agent_route`, and `agent_profile`. Demand replaces the
  static population when present; the static path is byte-identical.
- The three benchmark scenarios now express flow as portal demand, so
  `four_leg_signal_v1` and the other benchmarks run instead of overflowing the
  Increment 0 spawn capacity.

Evidence:

- `cargo test --workspace --all-features` passes, including the new
  `crates/tangle-sim/tests/vehicle_flow.rs` (arrivals, route assignment, range
  sampling, bounded saturated queue, non-overlap, determinism) and the
  scenario-level `benchmark_demand_generates_routed_vehicles` in
  `apps/tangle-cli/tests/scenarios.rs`. The walking golden trace and Phase 1
  baseline are unchanged.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, and `./scripts/check-dependency-direction.sh` pass;
  no dependency edge changed.
- `schemas/scenario-source.schema.json` was regenerated for the new source
  types.
