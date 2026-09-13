---
context_rev: 1
priority: P1
updated: 2026-09-13T12:15:33Z
summary: The operational gate metrics v1 deferred are implemented and reported - throughput, travel time, stopped delay, control delay, and queue length and duration, each with a unit, applicability status, tie-break, and mode and movement disaggregation - metric_definition_version is 2 in every surface that emits a metric, DEF-005-metric-definition-v2 is settled from the implemented surface, and DEF-004-metric-definition-v1 is superseded.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Increment 6's gate requires the comparison report to show throughput, delay, and
queues. `DEF-004-metric-definition-v1` explicitly deferred all three (and level
of service), so implementing any of them changes the reported metric set and
forces `metric_definition_version: 2` under the v1 version-bump rule. This slice
implements them and settles the new definition rather than editing the resolved
v1 in place: [[DEF-005-metric-definition-v2]] is the v2 authority and DEF-004 is
superseded. Already-reported v1 artifacts stay attributed to DEF-004.

Consumes the v1 surfaces `crates/tangle-sim/src/metrics.rs`,
`crates/tangle-sim/src/event.rs`, `crates/tangle-cli/src/run_dir.rs`, and the
aggregation/comparison artifacts Increment 5 landed. No new output dependency may
enter `tangle-model` or `tangle-sim`; metric computation may live in the layer
that owns the value, but serialization/output dependencies belong in the
CLI/output layer.

# Outcome

The gate metrics the v1 definition deferred are implemented and versioned, and
the metric definition advances to v2:

- Throughput (persons/agents served per unit time).
- Delay (at least travel time and stopped/control delay).
- Queues (length and duration; queue events already exist).
- Level of service only if trivial and already implied; otherwise leave named as
  deferred in v2.
- Any conflict/event family the gate or comparison report needs that is not yet
  reported.

Each new metric has a unit, an applicability/quality status (reported, not
applicable, not observed), a tie-break, and mode and movement disaggregation
matching the v1 conventions. `metric_definition_version` is 2 wherever a metric
value is emitted.

# Done when

- Throughput, delay, and queue length/duration metrics are implemented and
  tested (unit or property tests for each formula), with units, applicability
  and quality status, tie-break, and mode/movement disaggregation.
- Every unimplemented comparison metric remains explicitly named as deferred in
  v2 rather than silently omitted.
- `metric_definition_version` is 2 in every surface that emits it; a v1 artifact
  is never relabelled.
- [[DEF-005-metric-definition-v2]] is settled from the implemented surface,
  naming each new metric's formula, unit, applicability, tie-break, and exact
  source location, and is resolved.
- `DEF-004-metric-definition-v1` records `disposition: superseded` and
  `Superseded by [[DEF-005-metric-definition-v2]]`; the exact pinned-consumer
  search returns no stale consumer.
- `braintree check` passes, and the working node status, body, and any moves are
  committed coherently.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.

# Result

Implemented and settled. The gate metrics v1 deferred
([[DEF-004-metric-definition-v1]]) are reported, `metric_definition_version` is 2
in every surface that emits a metric value, and [[DEF-005-metric-definition-v2]]
is settled from the implemented surface with DEF-004 superseded.

## Metrics added

The always-on observation pass in `crates/tangle-sim/src/metrics.rs`
(`OperationMetrics`, `metrics.rs:997`, fed from the tick
`InteractionMetrics::observe` at `metrics.rs:719` already observes and read
through `InteractionMetrics::operation`, `metrics.rs:692`) now reports six
operational metrics per bucket, at the run level, per mode, and per movement —
keyed `movement:<name>` / `pedestrian_route:<name>` exactly as the v1 buckets
are (`apps/tangle-cli/src/run_metrics.rs:854`):

| Metric | Unit | Formula / derivation | Source |
| --- | --- | --- | --- |
| `throughput_agents_per_s` | agents/s | `served_agents / elapsed_s` | `metrics.rs:961`, `:1299` |
| `mean_travel_time_s`, `total_travel_time_s` | seconds | `despawn_s - spawn_s` per served agent | `metrics.rs:964`, `:967` |
| `mean_stopped_delay_s`, `total_stopped_delay_s` | seconds | sum of `depart_s - join_s` over `Event::Queue` states | `metrics.rs:970`, `:973` |
| `mean_control_delay_s`, `total_control_delay_s` | seconds | sum of the active `Event::ControlTransition` intervals | `metrics.rs:976`, `:979` |
| `maximum_queue_length_agents` | agents | most simultaneous open stopped states, with its tick | `metrics.rs:982`, `:1264` |
| `maximum_queue_duration_s`, `mean_queue_duration_s` | seconds | longest and mean closed stopped state, with agent and tick | `metrics.rs:984`, `:1278` |

Each carries the v1 statuses: `reported`, `not_observed` for a bucket that made
no observation, and `not_applicable` for throughput before any elapsed time; a
reported zero is a value, never a marker for an absent one. The two maximum
statistics update on strictly-greater, so a tie keeps the first — the earliest
tick for a length, the earliest closed state (lowest `AgentId` within a tick)
for a duration; a rate, sum, or mean selects nothing and accumulates in tick
order and ascending `AgentId` order, so no tie-break applies. Admission time is
read from the agent store, so demand admission and its `Spawned` records are
untouched, and a despawn closes an open stop or control state at the same tick
the safety pass's own closing record carries, so the two never double-count.

No conflict or event family needed adding: violations (by kind),
collision/contact events, near misses, TTC, PET, minimum separation, and every
other event variant the kernel emits are already reported at v1.

## Versioning and definition

- `METRIC_DEFINITION_VERSION` is 2 (`apps/tangle-cli/src/run_metrics.rs:87`),
  and the run summary (`run_dir.rs:440`), the run metrics artifact
  (`run_metrics.rs:472`), the aggregation (`aggregate.rs:875`, `:1018`), the
  comparison (`compare.rs:704`, `:1215`), and the convergence report
  (`converge.rs:569`, `:677`) all write it. No v1 artifact was relabelled.
- The aggregation and comparison now read the operational block
  (`aggregate.rs:652`, `:713`; `compare.rs:1007`), so both aggregate and compare
  the run-level and per-mode operational keys, with the units
  `agents_per_second` and `agents`.
- [[DEF-005-metric-definition-v2]] is resolved from the implemented surface:
  every added metric's formula, unit, applicability, tie-break, disaggregation,
  and exact source location, the carried-forward v1 set, the fixed version, the
  bump rule, and the still-deferred items (level of service, a free-flow delay
  reference, and batch-level operational movement slices with the F6
  event-family slices owned by [[TAS-048-increment-6-run-and-comparison-report]]).
- `DEF-004-metric-definition-v1` records `disposition: superseded` with a
  `Superseded by [[DEF-005-metric-definition-v2]]` edge; its `context_rev` is
  unchanged because v1's invariant is unchanged and supersession adds the
  replacement pointer only. The exact pinned-consumer search matches one line,
  the search text quoted in TAS-045's own context; there is no pinned edge on
  DEF-004 (`^Depends on [[DEF-004` matches nothing), so no consumer is stale.

## Evidence and gates

[[DEF-005-metric-definition-v2]] records the formulas; the tests pin them:
`crates/tangle-sim/src/metrics.rs` unit tests
(`the_operation_pass_reports_throughput_delay_and_queues`,
`a_trip_is_measured_from_the_admission_the_store_reveals`,
`a_despawn_closes_an_open_stop_state`,
`a_despawn_and_the_closing_record_of_its_own_stop_count_once`,
`an_equal_queue_duration_keeps_the_first_closure`,
`the_delay_and_throughput_ratios_hold_over_a_scripted_run`,
`an_unobserved_bucket_reports_no_observation`) and the end-to-end contract test
`apps/tangle-cli/tests/run_metrics.rs::metrics_json_reports_the_operational_families`,
which checks every reported status and value against an independent run's
in-process observation and checks the served count and throughput against the
canonical event stream's despawn records.

All five gates are green on this tree: `cargo test --workspace --all-features`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
`braintree check`. No golden or baseline was regenerated: the pass is read-only
and no state-affecting path changed.

## Closeout

The node stayed active until TAS-045's `next` advanced to
[[TAS-047-increment-6-scenario-variants-and-experiment-spec]], because moving it
to resolved while that route still named it fails `braintree check`
(FBK-011/012). The coordinator advanced that route (commit `7abf598`), and this
node now moves to `.braintree/resolved/` with its `next` removed and a
`Closes TAS-046` commit. The `next` this node carried until then spelled no
wikilink, because a checker reads a wikilink in a `next` as the route form
(recorded as `FBK-019`).
