---
context_rev: 1
priority: P1
updated: 2026-09-12T17:42:30Z
summary: Phase 1 Increment 3 adds pedestrian bodies, demand, and waypoint/collision-avoidance control, contextual crossing-against-signal noncompliance, vehicle yielding to occupied crossings, and explicit controller interfaces so cars and pedestrians share one world, rule and event representation, and mixed benchmark.
next: Add pedestrian signal compliance as a contextual choice over a physically possible movement, deciding only from the named `compliance` stream whether to cross against the signal, with no teleport and no special trajectory.
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

## Slice B — pedestrian waypoint controller, bounded steering, local avoidance

Slice A's constant-speed route tracking is replaced by a documented waypoint
controller in a new `crates/tangle-sim/src/pedestrian.rs` (a public module
holding the model card, the documented constants, `PedestrianWaypoint`, and
`PedestrianZone`, re-exported from the crate root). No schema change was needed.

### Waypoints and path progress

`Simulation` derives each route's waypoints once, at construction, from the
compiled route: one waypoint per named waiting area and crossing, at the
projection of the zone region's polygon centroid onto the route path, and the
route exit last. Waypoints are ordered by route progress with an explicit total
tie-break (kind, waiting areas before crossings, then dense id). The entry is the
admission point, not a waypoint, so the first target is the first zone ahead, or
the exit for a route that names no zone. A pedestrian's cursor advances past
every waypoint it has reached and never backward, and the exit is the last
waypoint, so the target is always ahead.

A pedestrian is now steered in world space: its state is a world position, a
canonical heading in `(-pi, pi]`, and a speed, and `path_distance_m` becomes the
projection of that position onto the route path. Progress is that arc measured
from the route entry along the direction of travel, so a route entering at the
path end travels inward and despawns at the path start. The spawn heading is the
direction of the first waypoint, which corrects the previous path-tangent
heading for backward routes.

### Local avoidance

`Simulation::collect_conflicts` scans every live agent in ascending `AgentId`
order (no hash map in state-affecting logic) within the 4 m sense radius and
reports, per neighbour, the vector to the nearest surface point of its body — an
exact circle for a pedestrian, an exact oriented box for a vehicle — plus the
signed surface clearance and the neighbour's speed. Steering is a bounded
deflection toward the free side: the perpendicular component of "away" from the
body, plus a fixed right-hand term that applies only to a body strictly ahead,
so a pedestrian is never steered into a body beside it, and a body behind is
ignored so a queue cannot push itself forward.

### Bounds and the emergency spacing cap

Model-card constants: sense radius 4 m, personal space 0.5 m, maximum turn rate
2 rad/s, maximum lateral acceleration 2 m/s², speed-change bounds 1.5 m/s² up and
2.0 m/s² down, radial gain 1.0, lateral gain 0.5, turn slowdown 0.75 down to a
0.25 fraction of the desired speed, and a 0.05 m contact margin. Every step the
turn is bounded by `min(2 rad/s, 2 m/s² / max(v, 0.5 m/s)) * dt`, so the implied
lateral acceleration never exceeds its bound; the speed stays in `[0, v0]`; and
the speed change stays inside the acceleration bounds except when the hard
spacing cap binds, which is counted in the new
`Simulation::pedestrian_cap_steps`, mirroring the vehicle
`emergency_cap_steps` seam. The cap bounds the closing component of the step by
the available surface clearance less the neighbour's worst-case translation and
the contact margin, so a pedestrian never tunnels through a body, never
teleports, never initiates contact with a body ahead, and cannot deadlock on a
body (the cap never applies to motion away from one). It does not cover a
neighbour whose orientation changes within a step; exact swept queries remain a
later increment.

### New public surface

`crate::pedestrian` with the model card and the constants above, re-exported
`PedestrianWaypoint`/`PedestrianZone`, and `Simulation::route_waypoints`,
`Simulation::pedestrian_waypoint`, and `Simulation::pedestrian_cap_steps`.

### Evidence

- `crates/tangle-sim/src/pedestrian.rs` unit tests: closest-arc projection on a
bent path, free-flow seeking, the turn-rate and lateral bounds, head-on
deflection direction, bodies behind and abeam, the minimum-over-candidates cap,
the agent-id tie-break for a perfect overlap, the speed-change bounds, the
contact-margin cap, nearest surface points for a circle and a rotated box,
route-progress direction, and heading normalization.
- `crates/tangle-sim/tests/pedestrian_control.rs` (new, 11 tests): waypoint
derivation in travel order for a forward route, a reversed route, and a route
that names no zone; the monotone cursor reaching the exit; the steering,
lateral, speed, and speed-change bounds with the counted cap exception; a
pedestrian passing a shared-path vehicle while holding the contact margin (worst
observed clearance exactly 0.05 m) with lateral deviation above 1 m and no
deadlock; opposite-direction pedestrians passing without overlap or teleport; a
same-speed saturated queue holding its admission spacing; no reaction beyond the
sense radius; mixed-benchmark completion with no NaN, no pedestrian overlap, no
pedestrian-initiated contact, no route deadlock, no dropped arrivals, and the
cap engaged in fewer than a tenth of steps; and same-seed reproducibility.
- The controller draws nothing from any random stream, so the `demand`,
`pedestrian_demand`, `profile`, and `compliance` streams are untouched; slice A's
stream-isolation and same-seed tests still pass.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
`braintree check nodes`. The walking trace golden, the scene golden, and the
Phase 1 baseline are unchanged, and no checked-in scenario needed editing.

### Deferred to later slices

- Pedestrian signal compliance and the choice to cross against a signal
(slice C), and vehicle yielding to an occupied crossing (slice D). A pedestrian
therefore cannot yet avoid a vehicle that is already committed across its path:
on the checked-in `pedestrian_crossing_v1` benchmark, six seeds over 4000 ticks
each show 0 pedestrian overlaps, 0 pedestrian-initiated contacts, and 4–10
vehicle-initiated overlaps per seed with a worst penetration of 0.30 m. Closing
those is exactly slice D's vehicle-side work.
- Slice C has no pedestrian signal primitive yet: a crossing names the movements
it crosses but no signal rule for pedestrians, so it needs either an additive
schema-v1 field or a rule derived from the crossing's movements, plus a named
pedestrian decision stream draw.
- A body that parks on a route waypoint leaves that waypoint unreachable, so the
pedestrian waits beside it; with no vehicle yielding yet, a vehicle that creeps
onto a waiting pedestrian is not prevented.
- Cross-path and cross-mode vehicle logic is untouched: `nearest_leader` and
`entry_clear` still act only on the same path, and no vehicle reacts to a
pedestrian.
- A neighbour whose orientation changes within a step can sweep a corner past a
pedestrian circle without a translation; exact swept queries belong to the
broad-phase/collision-query increment.
- `Event::Spawned` still does not carry the agent mode and `SceneBody::project`
still draws every body as a vehicle (slice A's deferral, unchanged).

