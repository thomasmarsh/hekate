---
context_rev: 1
priority: P1
updated: 2026-09-12T17:22:16Z
summary: Phase 1 Increment 3 adds pedestrian bodies, demand, and waypoint/collision-avoidance control, contextual crossing-against-signal noncompliance, vehicle yielding to occupied crossings, and explicit controller interfaces so cars and pedestrians share one world, rule and event representation, and mixed benchmark.
next: Add the pedestrian waypoint controller over the slice-A routes: track waypoint/path progress and apply bounded steering with local collision avoidance, replacing the constant-speed route tracking.
---

# Outcome

Per `PHASE_1_PLAN.md` Increment 3, pedestrians become fully simulated agents in
the same continuous world as vehicles:

- Pedestrian bodies, demand, and routes through waiting areas and crossings.
- A waypoint controller with local collision avoidance and bounded steering,
  with pedestrian path/route tracking.
- Pedestrian signal compliance: a contextual choice to cross against the
  signal, drawn only from the named `compliance` stream, over a physically
  possible movement with no teleport or special trajectory.
- Vehicle yielding and stopping response to occupied crossings in the shared
  rule, spatial-index, and event representation.
- Explicit, replaceable controller interfaces and model cards for the initial
  vehicle and pedestrian models.

Any schema change stays additive to schema version 1, mirroring the
`stop_line_m`/`compliance` precedent: validate, regenerate
`schemas/scenario-source.schema.json`, and keep the drift test. No Phase 2
fields and no schema version 2.

# Done when

- Cars and pedestrians share the same continuous world, spatial index, rule
  representation, and event system, with evidence that both modes are indexed
  and emitted through the same seams.
- Pedestrian noncompliance is a choice over a physically possible movement, not
  a special trajectory or teleport, and the choice draws only from the named
  `compliance` stream.
- An automated mixed benchmark completes under nominal demand without NaNs,
  unresolved overlaps, or route deadlock, and it is reproducible for the same
  seed.
- Waypoint following, local collision avoidance, pedestrian signal-compliance
  boundaries, and vehicle yielding to an occupied crossing each have unit or
  scenario evidence.
- Explicit controller interfaces and model cards exist for the initial vehicle
  and pedestrian models.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Area [[IDX-001-tangle]].
Depends on [[TAS-027-phase-1-increment-2-vehicle-flow-controls]] at context_rev 2.

Increment 2 landed demand-driven vehicles, the IDM longitudinal controller,
signal state, and the contextual red-light compliance decision over
schema-v1 `stop_line_m`/`compliance`. This increment adds the pedestrian mode
and the first mixed interaction over those primitives. The pinned dependency is
resolved at context_rev 2, so it records the vehicle, rule, signal, and stream
interfaces this increment builds over.

Constraint: `f64`/`glam::DVec2`, single-threaded state-affecting tick, stable
ordering with explicit tie-breakers, no Bevy types in `tangle-model` or
`tangle-sim`, and no schema version 2 (`PHASE_2_PLAN.md` owns it).

# Result

## Slice A — pedestrian bodies, demand, routes, compile/validate/schema

Pedestrians are now first-class agents generated from scenario demand and
admitted into the same agent store, identifier space, clock, and event stream as
vehicles. This slice is bodies/demand/routes/compile/validate/schema only; the
waypoint controller, local collision avoidance, pedestrian signal compliance,
and vehicle yielding are later slices.

### Schema (additive to version 1)

`crates/tangle-model/src/source.rs` adds four additive schema-version-1
collections/fields on `ScenarioSource`, all serde-defaulted so every existing
document is unchanged:

- `waiting_areas: Vec<WaitingAreaSource>` — `{ id, region }`, a named reference
to an existing region where pedestrians stage.
- `pedestrian_routes: Vec<PedestrianRouteSource>` — `{ id, from, to, path,
crossings[], waiting_areas[] }`, the pedestrian routing primitive. It reuses
the existing portals and guide paths for geometry and names the crossings and
waiting areas it passes through in travel order.
- `pedestrian_demand: Vec<PedestrianDemandSource>` — `{ id, portal,
rate_pph, routes[{ route, weight }] }`, mirroring `DemandSource` for the
pedestrian mode with a pedestrians-per-hour rate.
- `pedestrian_profiles: PedestrianProfileSource` — `{ radius_m, speed_mps }`
ranges. A pedestrian body is a circle, so the physical range is a radius;
defaults are `radius 0.20–0.30 m`, `speed 1.0–1.6 m/s`.

`crates/tangle-model/src/validate.rs` adds twelve stable diagnostic codes —
`E_WAITING_AREA_UNKNOWN_REGION`, `E_PEDESTRIAN_ROUTE_UNKNOWN_PORTAL`,
`E_PEDESTRIAN_ROUTE_UNKNOWN_PATH`, `E_PEDESTRIAN_ROUTE_PORTAL_PATH_MISMATCH`,
`E_PEDESTRIAN_ROUTE_SELF_LOOP`, `E_PEDESTRIAN_ROUTE_UNKNOWN_CROSSING`,
`E_PEDESTRIAN_ROUTE_UNKNOWN_WAITING_AREA`,
`E_PEDESTRIAN_DEMAND_UNKNOWN_PORTAL`, `E_PEDESTRIAN_DEMAND_EMPTY_ROUTES`,
`E_PEDESTRIAN_DEMAND_UNKNOWN_ROUTE`,
`E_PEDESTRIAN_DEMAND_ROUTE_PORTAL_MISMATCH`,
`E_PEDESTRIAN_DEMAND_DUPLICATE_ROUTE` — and reuses `E_NON_POSITIVE` for a
non-positive rate/weight and `E_PROFILE_RANGE` for a pedestrian profile range.
The new object ids participate in the duplicate-id check.

`crates/tangle-model/src/compiled.rs` adds dense `WaitingAreaId`,
`PedestrianRouteId`, and `PedestrianDemandId` plus `CompiledWaitingArea`,
`CompiledPedestrianRoute`, `CompiledPedestrianRouteShare`,
`CompiledPedestrianDemand`, and `CompiledPedestrianProfile`, all reachable
through `CompiledScenario` and `IdMap`. A compiled route carries its portals,
path, ordered crossing/waiting-area ids, and derived entry/exit endpoints.
`schemas/scenario-source.schema.json` is regenerated and the checked-in-schema
drift test passes.

### Kernel

`crates/tangle-sim` makes pedestrians first-class over one shared store:

- `agent.rs` adds `AgentMode { Vehicle, Pedestrian }` and `mode`,
`pedestrian_route`, and `pedestrian_profile` columns to `AgentStore`, so both
modes share one contiguous, stable-order array and identifier space.
- `rng.rs` adds the named `STREAM_PEDESTRIAN_DEMAND` = `"pedestrian_demand"`
stream, one substream per pedestrian demand source. Pedestrian bodies reuse the
mode-neutral `STREAM_PROFILE`, keyed by the shared stable agent id, so vehicle
and pedestrian profiles never share a generator.
- `profile.rs` adds `PedestrianProfile { radius_m, desired_speed_mps }` and
`sample_pedestrian_profile`, which samples both fields in a fixed order.
- `demand.rs` generalises `DemandRuntime` to `DemandRuntime<T>` over the dense
route id and adds `sample_pedestrian_route`; a shared weighted-pick helper
keeps vehicle route selection byte-identical.
- `sim.rs` admits pedestrians from `pedestrian_demand` under the same
`entry_clear` portal rule, samples the body/gait from the stable agent id,
records the route on the agent, and emits the shared `Event::Spawned` /
`Event::Despawned` transitions. An admitted pedestrian tracks its route path at
its sampled constant walking speed (no controller, steering, or avoidance) and
despawns with `DespawnReason::ExitedPath` at the route exit. The static
walking-skeleton population is now used only when no demand source of either
mode is declared. `MotionSample` gains `mode`, `pedestrian_route`, and
`pedestrian_profile`, and `Simulation` exposes `agent_mode`,
`agent_pedestrian_route`, `agent_pedestrian_profile`,
`pending_pedestrian_arrivals`, and `dropped_pedestrian_arrivals`.

### Scenario

`scenarios/benchmarks/pedestrian_crossing_v1.json5` authors a road movement, a
crossing over it, a south-kerb waiting area, one pedestrian route through both,
and demand for each mode.

### Evidence

- `crates/tangle-model/src/source.rs` tests parse the new primitives and the
additive defaults.
- `crates/tangle-model/src/validate.rs` tests accept a complete pedestrian
layout and flag every new route, waiting-area, demand, and profile diagnostic;
`crates/tangle-model/src/compiled.rs` tests compile the same layout and check
dense ids, ordered crossings/waiting areas, derived endpoints, and the id map.
- `crates/tangle-model/tests/fuzz_scenarios.rs` generates the new collections so
the "arbitrary sources never panic" property covers them.
- `crates/tangle-sim/tests/pedestrian_flow.rs` (new, 12 tests) covers demand
admission without the static population, route assignment, body/profile bounds,
constant-speed route tracking with an exact one-step displacement, exit despawn,
saturated-queue load shedding, admission non-overlap, backward entry at the path
end, same-seed reproducibility, both modes sharing one snapshot and event
stream, and `pedestrian_demand` isolation from the vehicle `demand` stream (the
vehicle arrival trace is byte-identical with and without pedestrian demand).
- `crates/tangle-sim/src/rng.rs` proves the added pedestrian stream differs from
the vehicle streams and that extra pedestrian draws leave the vehicle `demand`,
`profile`, and `compliance` sequences byte-identical.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
`braintree check nodes`. The walking trace golden, scene golden, and Phase 1
baseline are unchanged.

### Deferred to later slices

- Waypoint controller, bounded steering, and local collision avoidance; a
pedestrian currently holds its sampled constant speed along the route path.
- Pedestrian signal compliance and a pedestrian signal primitive over a
crossing; no schema field carries it yet.
- Vehicle yielding and stopping at an occupied crossing, and any cross-path or
cross-mode interaction; `nearest_leader` and `entry_clear` still act only on the
same path.
- Pedestrian route geometry beyond a single guide path: crossings and waiting
areas are ordered named references, so their stop positions along the route and
multi-path routes are later work.
- `Event::Spawned` does not carry the agent mode, and the presentation layer's
`SceneBody::project` still reports every body as a vehicle, so a renderer
cannot yet style a pedestrian distinctly; both stay out of this slice's write
set.

