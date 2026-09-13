---
context_rev: 1
priority: P1
updated: 2026-09-13T12:16:08Z
summary: The settled versioned metric definition v2 (metric_definition_version 2) carries every v1 metric forward unchanged and names the operational families v1 deferred - throughput, travel time, stopped delay, control delay, queue length, and queue duration - each with formula, unit, applicability status, tie-break, mode and movement disaggregation, and its exact source location, leaving level of service deferred.
---

# Context

Parent [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]].

This revision replaces metric definition v1 (the resolved `DEF-004` node): Increment 6's gate needs
throughput, delay, and queues, which v1 explicitly deferred, so the reported
metric set changes and the version-bump rule in v1 fixes a new revision rather
than a silent reinterpretation. Two definition revisions never share a version,
so v2 is this node and v1 stays the authority for artifacts already reported at
v1.

This invariant is settled from the implemented surface by `TAS-046`, and every
source location below is that implemented code.

# Invariant

## Version

This document is **metric definition v2**: `metric_definition_version: 2`. The
single constant that fixes it is `apps/tangle-cli/src/run_metrics.rs:87`
(`METRIC_DEFINITION_VERSION`), and it is what the run summary
(`apps/tangle-cli/src/run_dir.rs:440`), the run metrics artifact
(`run_metrics.rs:494`, `:520`), the aggregation
(`apps/tangle-cli/src/aggregate.rs:976`, `:1166`), the comparison
(`apps/tangle-cli/src/compare.rs:761`, `:1404`), and the convergence report
(`apps/tangle-cli/src/converge.rs:766`, `:929`) each write. An artifact already
written at v1 is never relabelled; `DEF-004` remains its definition.

## Carried forward from v1, unchanged

Every metric of `DEF-004-metric-definition-v1` is unchanged in name, formula,
unit, applicability, tie-break, and disaggregation keys, and v1 remains the
authority for it. In brief:

- the interaction minima `minimum_ttc_s` (seconds),
  `minimum_separation_m` (metres, signed), and `minimum_post_encroachment_s`
  (seconds), with `region_occupancies(region)` as the PET derivation input;
- the per-`ModePair` separation slice keyed
  `vehicle_vehicle` / `vehicle_pedestrian` / `pedestrian_pedestrian`;
- the per-movement minima keyed by each pair's two sorted movement keys;
- the countable event families — collisions, near misses, violations (split by
  kind), region entries and exits, queue events, control transitions (split by
  kind), yields, spawns, and despawns — each `reported` with the record count,
  `0` when the family is absent;
- the three quality statuses (`reported`, `not_applicable`, `not_observed`) and
  the rule that an absent value is never a `0`; and
- the reporting constants v1 declared: `INTERACTION_RANGE_M`,
  `TTC_HORIZON_S`, `TTC_TIME_TOLERANCE_S`, `SEPARATION_RESOLUTION_M`,
  `NEAR_MISS_THRESHOLD_M`, `QUEUE_STOP_SPEED_MPS`, and `CONTACT_EPSILON_M`.

The `# Invariant` of `DEF-004-metric-definition-v1` is the full statement of
each of those, with its own source locations; it is not restated here.

## Added: the operational families

The operational metrics are computed by one always-on observation pass in
`crates/tangle-sim/src/metrics.rs`: `OperationMetrics`
(`metrics.rs:997`), fed from the tick that `InteractionMetrics::observe`
(`metrics.rs:719`) already observes and read through
`InteractionMetrics::operation` (`metrics.rs:692`). The pass reads the live
agent store and the typed event records the tick produced and adds no predicate
of its own about stopped, waiting, or served state, so it cannot disagree with
`Event::Queue`, `Event::ControlTransition`, or `Event::Despawned` about the
state it reports.

**Admission time.** A trip's start is the end of the tick that admitted the
agent, read from the agent store rather than from a second hook: an agent live
at the start of tick `t` was admitted at the end of tick `t - 1`, and the
initial population is live at tick zero (`OperationMetrics::observe`,
`metrics.rs:1055`; `OperationMetrics::ensure_trip`, `metrics.rs:1122`). Demand
admission and its `Event::Spawned` records are untouched.

**Served agent.** An agent whose `Event::Despawned` completed its trip inside
the run. `DespawnReason::ExitedPath` is the only despawn reason the kernel
reports (`crates/tangle-sim/src/event.rs:70`), so every despawn is a completed
trip.

**Elapsed time.** `elapsed_s` is the end of the last observed tick
(`metrics.rs:1057`), so a run of `t` ticks of step `s` reports `t * s`.

1. **Throughput — `throughput_agents_per_s`, unit agents per second**
   (`OperationValues::throughput_agents_per_s`, `metrics.rs:961`; computed at
   `metrics.rs:1299`; written at `run_metrics.rs:389`). Formula:
   `served_agents / elapsed_s`, the agents whose trip completed inside the run
   divided by the run's elapsed simulated seconds. Disaggregated by mode and by
   movement.
   Applicability: **not applicable** before the run has elapsed any time (the
   rate has no denominator); **not observed** for a bucket that never held a
   live agent; otherwise reported, including the true `0.0` of a bucket that
   held agents and served none.
   Tie-break: no selection — a ratio of two deterministic totals. The totals
   accumulate in tick order and, within a tick, in ascending `AgentId` order.

2. **Travel time — `mean_travel_time_s` and `total_travel_time_s`, unit seconds**
   (`metrics.rs:964`, `:967`; computed at `metrics.rs:1301`; written at
   `run_metrics.rs:390`). Formula: `travel_time_s(agent) = despawn_s - spawn_s`
   per served agent, with `spawn_s` the admission time above; the bucket reports
   the mean over its served agents and their total. `despawn_s` is the end of
   the tick that reported the despawn (`OperationMetrics::serve`,
   `metrics.rs:1180`).
   Applicability: **not observed** for a bucket that served no agent; otherwise
   reported. Never not applicable: a trip time exists for every completed trip.
   Tie-break: no selection — a sum and its mean over the served agents, summed
   in tick order and within a tick in ascending `AgentId` order.

3. **Stopped delay — `mean_stopped_delay_s` and `total_stopped_delay_s`, unit
   seconds** (`metrics.rs:970`, `:973`; computed at `metrics.rs:1303`; written
   at `run_metrics.rs:391`). Formula: the sum over a served agent's stopped
   states of `depart_s - join_s`, where a stopped state opens on
   `Event::Queue { joined: true }` and closes on `Event::Queue { joined: false }`
   (`OperationMetrics::join_queue`, `metrics.rs:1149`;
   `OperationMetrics::close_stop`, `metrics.rs:1157`). The predicate is the
   kernel's own: the tick-end speed at or below `QUEUE_STOP_SPEED_MPS`
   (`crates/tangle-sim/src/safety.rs:101`). The bucket reports the mean per
   served agent and their total.
   Applicability: **not observed** for a bucket that served no agent; otherwise
   reported, including the true `0.0` of served agents that never stopped.
   Tie-break: no selection — a sum over the closed intervals in tick order and,
   within a tick, ascending `AgentId` order.

4. **Control delay — `mean_control_delay_s` and `total_control_delay_s`, unit
   seconds** (`metrics.rs:976`, `:979`; computed at `metrics.rs:1305`; written
   at `run_metrics.rs:392`). Formula: the sum over a served agent's control
   states of `end_s - start_s`, where the state opens on
   `Event::ControlTransition { active: true }` and closes on
   `Event::ControlTransition { active: false }` (the same pass,
   `metrics.rs:1070`-`:1107`), for either kind — `signal_stop` for a vehicle and
   `crossing_wait` for a pedestrian (`event.rs:178`). The state follows the
   agent's recorded signal-compliance decision, so it begins when that decision
   turns to wait, while the body may still be braking, and ends when the
   decision turns away: it is the time the control device held the agent, and it
   is not a subset of stopped delay. The bucket reports the mean per served
   agent and their total.
   Applicability: **not observed** for a bucket that served no agent; otherwise
   reported, including the true `0.0` of served agents that never waited.
   Tie-break: no selection, as for stopped delay.

5. **Queue length — `maximum_queue_length_agents`, unit agents**
   (`OperationValues::maximum_queue_length`, `metrics.rs:982`; snapshot at
   `Bucket::observe_length`, `metrics.rs:1264`; written at `run_metrics.rs:393`).
   Derivation: after each observed tick, the number of the bucket's agents
   holding an open stopped state at that tick end; the reported value is the
   most the bucket ever held, with `QueueLength { agents, tick }` carrying the
   tick it was observed on. Disaggregated by mode and by movement.
   Applicability: **not observed** for a bucket that never held a live agent;
   otherwise reported, including the true `0` of a bucket whose agents never
   queued.
   Tie-break: strictly-greater update, so a tie keeps the first: the earliest
   tick that held the maximum.

6. **Queue duration — `maximum_queue_duration_s` and `mean_queue_duration_s`,
   unit seconds** (`OperationValues::maximum_queue_duration`, `metrics.rs:984`;
   `Bucket::close_stop`, `metrics.rs:1278`; written at `run_metrics.rs:394`).
   Derivation: `depart_s - join_s` of one stopped state, exactly the interval
   stopped delay accumulates. The bucket reports the longest state it closed,
   with `QueueDuration { agent, seconds, tick }` carrying the agent that stood
   and the tick that closed it, and the mean of the states it closed.
   Applicability: **not observed** for a bucket that closed no stopped state;
   otherwise reported. A state still open at the end of the run is not a
   duration, exactly as an open region occupancy is not a post-encroachment
   time. A despawn closes the agent's stream, so a state still open then ends at
   the despawn and counts, whether or not the tick carried the closing record;
   the closing record the safety pass emits for a despawned agent carries the
   same tick, so the two never double-count (`OperationMetrics::serve`,
   `metrics.rs:1180`).
   Tie-break: strictly-greater update, so a tie keeps the first: the earliest
   closed state, which within one tick is the lowest `AgentId`.

## Disaggregation of the operational families

- **Run level** — `RunMetricsArtifact::operational.run`
  (`run_metrics.rs:464`, `:490`).
- **Mode** — `operational.by_mode`, keyed by `AgentMode::label()`
  (`vehicle`, `pedestrian`; `crates/tangle-sim/src/agent.rs:30`), always
  carrying both modes (`run_metrics.rs:886`). The kernel's per-mode buckets are
  indexed by `mode_slot` (`metrics.rs:1318`).
- **Movement** — `operational.by_movement`, keyed by the same spelling the v1
  movement buckets use: `movement:<name>` for a vehicle movement and
  `pedestrian_route:<name>` for a pedestrian route
  (`run_metrics.rs:876`), where the kernel's key is the tagged union
  `MovementKey` of `MovementId` and `PedestrianRouteId` (`metrics.rs:913`) and
  the output layer resolves the name through the compiled scenario
  (`run_metrics.rs:893`), exactly as v1 fixed it. An operational bucket is a
  property of one agent, so it is keyed by that agent's own movement key rather
  than by a pair of keys. An agent with no assigned movement (the initial static
  population) contributes to the run-level and mode buckets and to no movement
  bucket, matching v1.

## Where the version appears

`metric_definition_version: 2` is written beside every reported metric value in
the run summary (`run_dir.rs:440`), the run metrics artifact
(`run_metrics.rs:494`, `:520`), the batch aggregation (`aggregate.rs:1166`), the
paired comparison (`compare.rs:761`), and the convergence report
(`converge.rs:766`). Every aggregated and compared distribution repeats it
(`aggregate.rs:976`, `compare.rs:1404`, `converge.rs:929`). The aggregation and
comparison refuse an artifact from another revision rather than mixing
revisions (`aggregate.rs:588`, `compare.rs:969`).

## Version-bump rule

`metric_definition_version` is `2` now. Increment it when any reported metric's
name, formula or derivation, unit, applicability or quality rule, tie-break, or
disaggregation key changes; when the reported metric set changes; or when a
declared reporting constant changes a reported value or its applicability. The
v1 list of constants is carried forward, and v2 adds `QUEUE_STOP_SPEED_MPS` as a
declared constant of two reported metrics: it fixes both the stopped-delay
predicate and every queue length and duration. Editorial text that changes no
reported value does not bump the version. The bump is deliberate, lands with the
code change, and precedes any regenerated golden or baseline that captures the
changed numbers. Two definition revisions never share a version.

## What v2 does not claim (deferred, named)

- **Level of service** — no implementation anywhere in the workspace; v2 does
  not claim it, exactly as v1 did not.
- **Delay against a free-flow reference** — no free-flow or reference-speed
  baseline exists in the model, so v2 reports stopped and control delay, which
  the recorded state defines, and claims no time-lost measure.
- **Batch-level slices of the operational families beyond mode** — the run
  artifact carries the movement slice, and the aggregation and comparison
  aggregate the run-level and per-mode operational keys. Aggregating the
  operational movement slice at batch level, together with the event-family mode
  and movement slices (slice-F finding F6), belongs to
  [[TAS-048-increment-6-run-and-comparison-report]], which owns the comparison
  report.

# Done when

- The invariant is settled from the implemented surfaces, naming each new
  metric's formula, unit, applicability, tie-break, and exact source location,
  and restating the carried-forward v1 set and the still-deferred metrics.
- `metric_definition_version` is fixed at 2 and the bump rule is restated.
- The resolved v1 definition node records `disposition: superseded`, its
  canonical `Superseded by` edge points at this node, and no pinned consumer is
  stale.
- The node is resolved (the act of settling it) and is the definition the
  Increment 6 run summary, aggregation, and comparison report cite.

# Result

Settled metric definition v2 from the implemented surface and resolved the node.
The operational families v1 deferred are implemented in
`crates/tangle-sim/src/metrics.rs` (`OperationMetrics`, `metrics.rs:997`,
observed by the same tick pass at `metrics.rs:1055`) and reported through the run
metrics artifact (`apps/tangle-cli/src/run_metrics.rs:336`, `:438`, `:490`), the
batch aggregation (`apps/tangle-cli/src/aggregate.rs:652`, `:713`), the paired
comparison (`apps/tangle-cli/src/compare.rs:1007`), and the convergence report,
each at `metric_definition_version: 2`.

Six metrics were added, each with the definition above: throughput
(`throughput_agents_per_s`, agents per second), travel time
(`mean_travel_time_s`, `total_travel_time_s`), stopped delay
(`mean_stopped_delay_s`, `total_stopped_delay_s`), control delay
(`mean_control_delay_s`, `total_control_delay_s`), queue length
(`maximum_queue_length_agents`, agents, provenance `tick`), and queue duration
(`maximum_queue_duration_s`, `mean_queue_duration_s`, provenance `agent` and
`tick`). All are disaggregated by mode and by movement, and every one is checked
against the in-process observation and, for throughput and the served count,
against the canonical event stream's despawn records
(`apps/tangle-cli/tests/run_metrics.rs::metrics_json_reports_the_operational_families`).

Verified as already reported at v1, so not added: violations (by kind),
collision/contact events, near misses, time to collision, post-encroachment time,
and minimum separation. Every event variant the kernel emits is counted, so no
conflict or event family was missing.

Still deferred and named: level of service, a free-flow delay reference, and
batch-level operational movement slices with the F6 event-family slices
([[TAS-048-increment-6-run-and-comparison-report]]).

`DEF-004-metric-definition-v1` records `disposition: superseded` and
`Superseded by [[DEF-005-metric-definition-v2]]`; the exact pinned-consumer
search TAS-045 records in its context matches only lines that quote the search
text itself (TAS-045's own context and `FBK-019`), and no node holds a pinned
edge on DEF-004 at all, so no consumer is stale.
