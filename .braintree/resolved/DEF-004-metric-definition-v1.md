---
context_rev: 1
priority: P1
disposition: superseded
updated: 2026-09-13T12:14:45Z
summary: The settled versioned metric definition v1 (metric_definition_version 1) names every metric Increment 5 reports - time to collision, minimum surface separation, post-encroachment time, region occupancy, and the countable event families - with formula, unit, applicability status, tie-break, and mode and movement disaggregation keys, and defers throughput, delay, and level of service. Superseded by [[DEF-005-metric-definition-v2]].
---

# Context

Superseded by [[DEF-005-metric-definition-v2]]: metric definition v2 carries
every definition below forward unchanged and adds the operational families this
revision deferred (throughput, delay, and queue length and duration). This node
stays the authority for artifacts already reported at
`metric_definition_version: 1`; DEF-005 is the definition new work cites.

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Increment 5's gate requires that "every reported metric links back to
manifest(s) and a versioned metric definition". This node is that durable,
independently resumable definition: it is the authority the run summary,
aggregation, and sensitivity report cite, and it is versioned so a later metric
change is a new revision rather than a silent reinterpretation.

The definition is settled from the implemented surfaces, not invented:

- `crates/tangle-sim/src/metrics.rs` — the online interaction metrics (time to
  collision, minimum surface separation, conflict-region occupancy and
  post-encroachment time), read through `Simulation::interaction_metrics`
  (`crates/tangle-sim/src/sim.rs:671`);
- `crates/tangle-sim/src/event.rs` with `crates/tangle-sim/src/safety.rs` — the
  typed event union (`EVENT_VERSION` 2, `event.rs:66`) whose records the
  canonical event stream carries;
- `apps/tangle-cli/src/run_dir.rs` — the immutable run directory the CLI writes
  (`manifest.json`, `summary.json`, `events.jsonl.gz`, `trajectories.parquet`),
  which links every reported number to the run's manifest.

# Invariant

## Version

This document is **metric definition v1**: `metric_definition_version: 1`, fixed
by this node. Every number a run artifact or an aggregate reports as a metric is
a value of one of the definitions below at version 1.

## Reported interaction metrics

These are computed by the always-on observation pass in `metrics.rs`
(`InteractionMetrics::observe`, `metrics.rs:602`) and read through
`Simulation::interaction_metrics` (`sim.rs:671`). The candidate set is the swept
broad-phase pairs whose surfaces come within `INTERACTION_RANGE_M = 20.0 m`
(`metrics.rs:182`) at some time in the tick; both bodies are grown by half the
range for the candidate grid, so no pair inside the range is missed and the
tests read the true bodies. A pair whose surfaces stay outside the range for a
whole run has no recorded value at all (see "Quality statuses").

1. **Minimum time to collision — `minimum_ttc_s`, unit seconds**
   (`metrics.rs:550`; computed at `metrics.rs:316`).
   Derivation: for each candidate pair, `time_to_collision` extrapolates the
   tick-end poses on straight lines at the tick's own displacement (each body's
   velocity over one fixed step) and reports the first simulated time at which
   the closed surfaces are predicted to touch. `Some(0.0)` when the pair already
   touches or overlaps at the observed state; `Some(seconds)` when the pair is
   closing and the predicted contact lies inside `TTC_HORIZON_S = 5.0 s`
   (`metrics.rs:191`); the first contact is located by bisection to
   `TTC_TIME_TOLERANCE_S = 1e-6 s` (`metrics.rs:199`). The run value is the
   least over all ticks and pairs.
   Applicability: **not applicable** (`None`) when the pair is not closing
   (receding, parallel, or keeping a constant separation), when predicted
   contact is later than the horizon, or when the extrapolated closest approach
   stays clear of contact; the metric never reports a large sentinel number for
   those cases.
   Tie-break: `keep_minimum` (`metrics.rs:773`) updates on strictly-less, so a
   tie keeps the first value recorded — the earliest tick that achieved the
   least value, and within that tick the lowest `(AgentId, AgentId)` pair,
   because candidate pairs are ascending (`crates/tangle-sim/src/index.rs:361`).

2. **Minimum surface separation — `minimum_separation_m`, unit metres (signed)**
   (`metrics.rs:524`; computed at `metrics.rs:414`).
   Derivation: `tick_minimum_clearance_m` reports a pair's least signed surface
   clearance over the tick, from the exact query the geometry layer exposes:
   positive is the disjoint distance, negative is `-penetration_depth`, zero is
   contact. Both tick endpoints are evaluated and, when the pair closes then
   separates inside the tick, the turning point is bisected to
   `SEPARATION_RESOLUTION_M = 1e-6 m` (`metrics.rs:208`). The run value is the
   least over all ticks and pairs. The same quantity is also kept per pair
   (`pair_minimum_separation_m`, `metrics.rs:539`), per mode pair
   (`mode_pair_minimum_separation_m`, `metrics.rs:530`), and per tick
   (`tick_minimum_separation_m`, `metrics.rs:555`).
   Applicability: always defined for a candidate pair; a contacting pair floors
   at contact and never reports a clear separation. A mid-tick penetration
   deeper than the contact entry is not this metric's value — the contact family
   (`Event::Collision`, `event.rs:254`) carries it.
   Tie-break: the same strictly-less rule as TTC.

3. **Minimum post-encroachment time — `minimum_post_encroachment_s`, unit
   seconds** (`metrics.rs:570`; derived at `metrics.rs:720`).
   Derivation: a body occupies a conflict region from the tick that reported its
   `Event::Entry` to the tick that reported its `Event::Exit` (a `Despawned`
   closes an open occupancy, `metrics.rs:701`); each completed occupancy's
   `entry_s`/`exit_s` are the ends of those ticks. Between two successive
   occupancies of one region that are by different bodies and do not overlap in
   time, `seconds = following_entry_s - preceding_exit_s`. The run value is the
   least; the full ordered list is `post_encroachments()` (`metrics.rs:565`).
   Applicability: **not applicable** (nothing recorded) for a succession by the
   same body, for occupancies that overlap in time (the bodies were in the
   region together, a different conflict family), and for a region with fewer
   than two completed occupancies. Both boundaries are quantized to the fixed
   step, so PET converges as the step refines.
   Tie-break: strictly-less over the recorded order; occupancies are stored in
   close order (tick order, and within one tick the ascending-`AgentId` order
   the safety pass emits a region's edges in), so a tie keeps the earliest
   succession.

4. **Region occupancy interval — `region_occupancies(region)`, unit seconds
   (two timestamps)** (`metrics.rs:575`; records at `metrics.rs:281`).
   Derivation: `RegionOccupancy { entry_s, exit_s }` per body per region; the
   occupancy duration is `exit_s - entry_s`. This record is the derivation input
   the PET metric reads, not a scored safety quantity of its own.
   Applicability: defined for every completed occupancy; an occupancy still open
   at the end of the run is not returned.
   Tie-break: the vector is in close order, so the record order is deterministic.

## Reported event-count metrics

The canonical event stream `events.jsonl.gz` in the run directory is the complete
record of these families; each variant is edge-triggered once per transition
(`event.rs:1`-`47`, `safety.rs:1`-`78`), and the stream is sorted by
`Event::order_key` (`event.rs:385`), so a count is a deterministic function of
the run. Unit: records (a count). Applicability: always defined for a completed
run; zero is the reported value when the family is absent. No minimum tie-break
applies; the stable key is the stream's documented order (ascending `AgentId`,
then `EventKind::order`, then the variant's own key, then the edge flag).

| Metric | Record | Counted edge | Source |
| --- | --- | --- | --- |
| collisions | `Event::Collision` | `contacting: true` | `event.rs:254` |
| near misses | `Event::NearMiss` | `entering: true` | `event.rs:274` |
| violations | `Event::Violation` | every record; split by `ViolationKind` | `event.rs:294`, kinds `event.rs:145` |
| region entries | `Event::Entry` | every record | `event.rs:305` |
| region exits | `Event::Exit` | every record | `event.rs:312` |
| queue events | `Event::Queue` | `joined: true` | `event.rs:325` |
| control transitions | `Event::ControlTransition` | every record; split by `ControlTransitionKind` | `event.rs:338`, kinds `event.rs:178` |
| yields | `Event::Yielded` | `yielding: true` | `event.rs:238` |
| spawns, despawns | `Event::Spawned`, `Event::Despawned` | every record | `event.rs:212`, `event.rs:224` |

A violation's kind separates the modes: `RanRedLight` is a vehicle,
`CrossedAgainstSignal` a pedestrian (`event.rs:145`). A collision or near-miss
record carries the pair (`agent` is the lower `AgentId`, `other` the higher);
contact and near miss are nested per-tick predicates, so a pair's near-miss
record ends where contact begins and a new one begins where contact ends.

The run summary the CLI writes (`apps/tangle-cli/src/run_dir.rs:314`) reports the
run's counts — `ticks`, `spawned`, `despawned`, `remaining`, `elapsed_s` — plus
`summary_version` and `manifest_sha256`; `spawned` and `despawned` are the
kernel's run totals (`crates/tangle-sim/src/sim.rs:182`, `:187`). The summary does
not yet carry the interaction-metric values or the event counts; those are
exposed through `Simulation::interaction_metrics()` and the event stream. The
manifest carries the versions the stream and model are read with:
`event_version` and `model_version` (`run_dir.rs:294`, `:296`).

## Disaggregation keys

- **Mode** — `AgentMode` (`vehicle` or `pedestrian`,
  `crates/tangle-sim/src/agent.rs:21`; labels at `agent.rs:30`), resolved per
  agent by `Simulation::agent_mode` (`sim.rs:500`). Pair metrics are sliced by
  `ModePair` (`metrics.rs:216`; labels `vehicle_vehicle`,
  `vehicle_pedestrian`, `pedestrian_pedestrian` at `metrics.rs:249`), which
  `MetricMinimum` carries (`metrics.rs:265`) and which
  `mode_pair_minimum_separation_m` (`metrics.rs:530`) is keyed on.
- **Movement** — the chosen key is the tagged union of **`MovementId` for
  vehicles and `PedestrianRouteId` for pedestrians**, resolved per agent by
  `Simulation::agent_route` (`sim.rs:505`) and
  `Simulation::agent_pedestrian_route` (`sim.rs:515`). The identities are the
  scenario's authored movement connector (`CompiledMovement`,
  `crates/tangle-model/src/compiled.rs:682`) and pedestrian route
  (`CompiledPedestrianRoute`, `compiled.rs:888`), named by
  `CompiledScenario::movement_name` (`compiled.rs:396`) and
  `pedestrian_route_name` (`compiled.rs:411`). A pairwise metric is sliced by
  the pair's two movement keys.
  Chosen over the other identities the model exposes (the guide `PathId`, or a
  region key) because the movement is the identity the scenario's control data
  is written against — conflict regions
  (`CompiledConflictRegion::movements`, `compiled.rs:985`), crossings
  (`CompiledCrossing::movements`, `compiled.rs:781`), rules and signal heads
  (`CompiledRule::movement`, `compiled.rs:1012`;
  `CompiledSignalHead::movement`, `compiled.rs:1041`), and demand route shares
  (`CompiledRouteShare::movement`, `compiled.rs:1125`) — and because `PathId` is
  not unique per movement: in `scenarios/benchmarks/mixed_interaction_v1.json5`
  the routes `west_to_north` and `west_to_south` share the path `west_walk`.
- **Region** — `RegionKey` (`Crossing` or `ConflictRegion`, `event.rs:118`) for
  the region-scoped metrics (occupancy, PET, region entries and exits). A
  crossing spans several vehicle movements (`CompiledCrossing::movements`,
  `compiled.rs:781`), so a region slice and a movement slice answer different
  questions and are both kept.

## Applicability and quality statuses

Three statuses are distinct and must not be conflated in an artifact:

1. **reported** — the metric has a value for the pair, region, or run;
2. **not applicable** — the predicate that would define a value is false for
   this observation: TTC's `None` for a non-closing, beyond-horizon, or
   clear-pass pair, and no PET for a same-body or overlapping succession;
3. **not observed** — no observation was made because the pair's surfaces never
   came within `INTERACTION_RANGE_M`, so there is no value and no claim that the
   interaction was safe.

A run-level minimum with no value yet is `None`; an artifact must render it as
"no value" rather than `0`, which would be a false report (a zero TTC means
contact).

## Version-bump rule

`metric_definition_version` is `1` now. Increment it when any reported metric's
name, formula or derivation, unit, applicability or quality rule, tie-break, or
disaggregation key changes; when the reported metric set changes (including
implementing a metric listed below as deferred); or when a declared reporting
constant changes a reported value or its applicability: `INTERACTION_RANGE_M`
(`metrics.rs:182`), `TTC_HORIZON_S` (`metrics.rs:191`), `TTC_TIME_TOLERANCE_S`
(`metrics.rs:199`), `SEPARATION_RESOLUTION_M` (`metrics.rs:208`),
`NEAR_MISS_THRESHOLD_M` (`safety.rs:93`), `QUEUE_STOP_SPEED_MPS`
(`safety.rs:101`), `CONTACT_EPSILON_M` (`query.rs:73`). Editorial text that
changes no reported value does not bump the version. The rule mirrors
`EVENT_VERSION` (`event.rs:55`-`65`): the bump is deliberate, lands with the code
change, and precedes any regenerated golden or baseline that captures the changed
numbers. Two definition revisions never share a version.

## Where the version appears

The field is **`metric_definition_version`**, fixed at `1` by this node. A run
summary or aggregation output that reports metric values carries that field
alongside them, so every number can be attributed to this revision. The run
directory already ties its artifacts together by `manifest_sha256`
(`run_dir.rs:319`) and by the manifest's `event_version` and `model_version`
(`run_dir.rs:294`, `:296`). Per `PHASE_2_PLAN.md:205`, every reported metric also
carries its definition version and applicability status.

Follow-up for the aggregation slice (no code changed by this node):
`summary.json` and the multi-seed aggregation must emit
`metric_definition_version: 1` with the metric block. Today `summary.json`
(`run_dir.rs:314`) carries only `summary_version`, `manifest_sha256`, `ticks`,
`spawned`, `despawned`, `remaining`, and `elapsed_s`, and no CLI output reports a
metric value.

## Deferred (named, not claimed)

These comparison metrics named in `PHASE_1_PLAN.md` Increment 5/6 have no
implementation, so v1 does not claim them; implementing one is a version bump
under the rule above:

- **throughput** (agents or persons served per unit time): no rate metric
  exists. The kernel reports counts (`RunSummary::spawned`, `sim.rs:182`) and
  demand arrival rates (`CompiledDemand::rate_vph`, `compiled.rs:1162`), not
  throughput.
- **delay** (travel time, stopped time, control delay): not implemented.
- **level of service**: not implemented.
- **queue length, queue duration, maximum queue**: only the per-agent queue
  event exists (`Event::Queue`, `event.rs:325`); length and duration are not
  implemented.

# Done when

- The invariant is settled from the implemented surfaces, naming each metric,
  its unit, applicability, disaggregation keys, and tie-break, with the exact
  source locations that compute it.
- `metric_definition_version` is fixed at 1 and the version-bump rule is
  stated.
- The node is resolved (the act of settling it) and is the definition later
  Increment 5 artifacts cite.

# Result

Settled metric definition v1 and resolved the node. The invariant above names
every metric Increment 5 actually reports at v1 — the four interaction metrics of
`crates/tangle-sim/src/metrics.rs` (minimum TTC, minimum surface separation,
minimum post-encroachment time, region occupancy interval) and the event-count
families of `crates/tangle-sim/src/event.rs` (collisions, near misses,
violations, region entries/exits, queue events, control transitions, yields,
spawns/despawns) — with each one's formula, unit, applicability and quality
status, tie-break, and exact source location.

Disaggregation: mode (`AgentMode`, and `ModePair` for pair metrics) and movement
(the tagged union `MovementId` for vehicles and `PedestrianRouteId` for
pedestrians), plus `RegionKey` for region-scoped metrics. `MovementId` /
`PedestrianRouteId` was chosen over the alternative `PathId` because two
pedestrian routes share one path in the mixed benchmark and because the movement
identity is what the scenario's rules, signals, conflict regions, and demand
sources reference.

`metric_definition_version` is fixed at 1; the bump rule and the constants that
force a bump are stated, as is the field (`metric_definition_version`) the run
summary and aggregation must carry. Throughput, delay, level of service, and
queue length/duration are recorded as deferred, not claimed.

No code, schema, or other node changed: this settlement is a definition only.
Two follow-ups belong to the aggregation slice, not to this node:

- `summary.json` and the multi-seed aggregation must emit
  `metric_definition_version` and the metric block (no reported value yet).
- No definition-version constant or field exists in code today; the aggregation
  slice introduces it with the version value fixed here.

`TAS-031`'s `next` still names this node after its resolution; per this node's
exclusive write set (the `DEF-004` file only) that parent advance is reported to
the coordinator rather than authored here (settled precedent: `FBK-011`).
