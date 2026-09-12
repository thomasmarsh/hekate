---
context_rev: 1
priority: P1
updated: 2026-09-12T16:50:21Z
summary: Phase 1 Increment 2 turns portals into demand, gives cars an IDM longitudinal controller with stop-line, signal, leader, queue, and exit behavior, and records reproducible contextual red-light decisions.
next: Verify the integrated Increment 2 slices against every Done-when criterion and resolve the node.
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
admission), slice B (path-distance tracking under a documented IDM
longitudinal controller, leader/following/queue/exit behavior, authored stop
lines, and the fixed-time signal phase state machine), and slice C (the
contextual red-light decision, its recorded reasons, and inspector display)
have landed. Final verification of the integrated increment remains.

## Slice C — contextual signal-compliance decision

- `crates/tangle-model/src/source.rs` adds an additive schema version 1
  `compliance` range to `profiles`, a fraction of the driver's comfortable
  braking in `[0, 1]` with a serde default of `1.0` (fully compliant).
  `validate.rs` adds `E_PROFILE_COMPLIANCE` and `compiled.rs` compiles
  `CompiledProfile::compliance()`; `schemas/scenario-source.schema.json` is
  regenerated and the checked-in-schema drift test passes.
- `crates/tangle-sim/src/rng.rs` adds `STREAM_COMPLIANCE`. `profile.rs` samples
  the propensity from the agent's `compliance` substream while the
  physical/longitudinal fields stay on the `profile` stream, so no two concerns
  share a mutable generator.
- `crates/tangle-sim/src/compliance.rs` is the documented decision model card:
  a stop-required head is obeyed when
  `required_deceleration <= comfortable_brake * compliance`, where
  `required_deceleration = v^2 / (2 * stop_line_gap)`. Reasons are `Green`,
  `PastStopLine`, `CannotStop`, `CompliantStop`, and `NonCompliantRun`; the
  comparison is inclusive, so the boundary decision is `Stop` (the documented
  tie-breaker). The propensity is the only random input and it comes from the
  `compliance` stream, so the decision is a deterministic function of its
  context, not a per-tick coin flip.
- `sim.rs` computes and records the decision before integrating each profile
  vehicle's speed; the stop-line constraint applies only when the decision is
  `Stop`, so a noncompliant runner proceeds under ordinary IDM and is never
  teleported. `Simulation::agent_decision` and the snapshot's `MotionSample`
  expose the small record, `tangle-present` projects it onto `SceneBody` and
  formats it with `decision_summary`, and both the TUI HUD and the Bevy viewer
  show the latest reason.
- `scenarios/benchmarks/red_light_compliance_v1.json5` runs a fixed-time
  red/green cycle with a full compliance range so one run exercises both
  stopping and red-light running.

Evidence:

- `crates/tangle-sim/src/compliance.rs` unit tests cover the green, red, and
  yellow boundaries, the inclusive tie-breaker, the zero-compliance limit, and
  the urgency formula.
- `crates/tangle-sim/tests/signal_compliance.rs` covers green/yellow/red at the
  simulation level, the recorded reason in snapshots, same-seed reproducibility,
  and `compliance`/`demand`/`profile` stream isolation.
- `crates/tangle-sim/src/rng.rs` proves an added `compliance` draw leaves the
  `demand` and `profile` sequences byte-identical.
- `apps/tangle-cli/tests/scenarios.rs` runs `red_light_compliance_v1` and
  asserts reproducible decision records, both outcomes, and that a recorded
  runner crosses the stop line on a stop-required head while staying inside its
  profile speed bound, without teleporting or overlapping.
- `cargo test --workspace --all-features`, clippy with `-D warnings`,
  `cargo fmt --all --check`, and `./scripts/check-dependency-direction.sh`
  pass; the walking golden trace is unchanged and the scene golden only gains
  the new `decision: None` field.

## Slice B — longitudinal control and signals

- `crates/tangle-model/src/source.rs` adds an optional `stop_line_m` on
  `MovementSource`, an arc length in metres from the movement entry along the
  movement's travel direction. Omitted means `0.0`. `validate.rs` adds
  `E_MOVEMENT_STOP_LINE` (finite, non-negative, no greater than the path
  length) and `compiled.rs` carries `CompiledMovement::stop_line_m`.
- `crates/tangle-sim/src/control.rs` is the documented IDM model card:
  `a = a_max[1 - (v/v0)^4 - sum_i (s*/gap_i)^2]` with
  `s* = s0_i + max(0, v*T + v*dv/(2*sqrt(a_max*b)))`. The profile supplies
  `v0`, `T`, `a_max`, and `b`; only the free-flow exponent and leader
  standstill gap are model constants. The command is clamped to
  `[-b, +a_max]` and speed to `[0, v0]`.
- `crates/tangle-sim/src/signal.rs` is the fixed-time phase state machine:
  phases are half-open `[start_s, start_s + duration_s)`, advanced from the
  authoritative clock, and `Simulation::movement_signal` exposes the authored
  color. No compliance decision is made.
- `crates/tangle-sim/src/sim.rs` tracks path distance under IDM for every
  profile vehicle (the static walking population stays constant-speed and its
  golden trace is unchanged), follows the nearest same-direction leader bumper
  to bumper with a stable lowest-id tie-break, admits demand vehicles at a
  **safe entry speed** that keeps comfortable braking sufficient, holds the
  front bumper at a red/yellow stop line, and keeps two last-resort caps that
  make overlap impossible: the next speed may not pass the leader's rear or a
  stop line in one step.
- `scenarios/benchmarks/car_following_v1.json5` is the controlled
  car-following benchmark; `four_leg_signal_v1.json5` now authors
  `stop_line_m: 34.0` just upstream of its conflict region so the signal queue
  behavior is exercised.

Evidence:

- `crates/tangle-sim/src/control.rs` and `signal.rs` unit tests cover the IDM
  bounds, free-flow convergence, stop-line braking, and green/yellow/red phase
  boundaries including cycle wrap.
- `crates/tangle-sim/tests/longitudinal_control.rs` covers the stop-line rest
  position, red queue + non-overlap, green release + exit, free-flow exit,
  profile speed bounds, and same-seed reproducibility of control and signal
  state.
- `apps/tangle-cli/tests/scenarios.rs` runs `car_following_v1` across seeds 0-2
  and asserts every command stays inside the sampled profile bounds
  (`speed in [0, v0]`, `accel in [-b, +a_max]`) with no body overlap while
  braking under real following.

## Slice A admission seam reconciliation

Slice B reconciles slice A's landed spawn admission so the Increment 2 gate
("accelerations/braking/speeds within profile bounds and no body overlap") can
hold. Slice A admitted every demand vehicle at its full sampled desired speed;
with clearance-only acceptance, a full-speed insert behind a slow leader forced
emergency braking far beyond the comfortable bound.

The reconciliation changes only the admitted speed, not admission acceptance:

```text
v_entry = min(v0, sqrt(v_leader^2 + 2 * b * max(gap - s0, 0)))
```

`v0` and `b` are the same sampled `VehicleProfile` fields, `gap` is the
bumper-to-bumper gap to the nearest live leader ahead on the entry path, and
`s0` is the model standstill gap (2.0 m). The vehicle enters no faster than the
speed from which `b` brings it to the leader's speed within the gap, then
accelerates toward `v0`. `entry_clear` acceptance and its clearance timing are
unchanged, so slice A's spawn/drop/queue tests still hold.

Exploratory probe evidence (not committed as a test): 16 seeds each at rates
600, 1200, and 1800 vph over a 400 m corridor, with both a tight and a wide
profile envelope, produced zero steps whose deceleration exceeded `-b` and a
minimum bumper-to-bumper gap of 0.66 m. Before the reconciliation the same
envelopes produced emergency decelerations above 40 m/s².

## Slice A — demand, profiles, admission

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
