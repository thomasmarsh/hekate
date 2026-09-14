---
context_rev: 1
updated: 2026-09-14T22:54:26Z
summary: Metric definition v3 adds the overtaking and close-pass families; METRIC_DEFINITION_VERSION 3.
---

Parent [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].

# Context

This revision replaces metric definition v2 (the resolved
[[DEF-005-metric-definition-v2]] node): the close-pass report needs the
overtaking and close-pass families v2 did not report, so the reported metric set
changes and v2's version-bump rule fixes a new revision rather than a silent
reinterpretation. Two definition revisions never share a version, so v3 is this
node and v2 stays the authority for artifacts already reported at v2.

The invariant is settled from the surfaces the contract
`docs/schema-v2-contract.md` *Metrics and output* fixes: the edge-triggered
overtaking records already emitted under `EVENT_VERSION` 3
(`crates/tangle-sim/src/event.rs (Event::Maneuver)`) and the close-pass
observations already closed (`crates/tangle-sim/src/close_pass.rs
(OvertakeObservation)`, read through `Simulation::close_pass_tracker`). The
accumulation and the run/aggregate/compare surface that read these definitions
are [[TAS-140-accumulate-close-pass-metric-families]] and
[[TAS-141-surface-close-pass-metric-families]].

# Invariant

## Version

This document is **metric definition v3**: `metric_definition_version: 3`. The
single constant that fixes it is
`apps/tangle-cli/src/run_metrics.rs (METRIC_DEFINITION_VERSION)`, and it is what
the run summary (`apps/tangle-cli/src/run_dir.rs`), the run metrics artifact and
the batch aggregation (`apps/tangle-cli/src/aggregate.rs`), the paired
comparison (`apps/tangle-cli/src/compare.rs`), the convergence report
(`apps/tangle-cli/src/converge.rs`), and the experiment report
(`apps/tangle-cli/src/experiment.rs`) each write. An artifact already written at
v1 or v2 is never relabelled; DEF-004 and DEF-005 remain its definition.

## Carried forward from v2 and v1, unchanged

Every metric of [[DEF-004-metric-definition-v1]] and
[[DEF-005-metric-definition-v2]] is unchanged in name, formula, unit,
applicability, tie-break, and disaggregation keys, and those nodes remain the
authority for it. In brief: the interaction minima (`minimum_ttc_s`,
`minimum_separation_m`, `minimum_post_encroachment_s`), the per-`ModePair` and
per-movement separation minima, the ten countable event families, the three
quality statuses with the rule that an absent value is never a `0`, and the
operational families v2 added (throughput, travel time, stopped delay, control
delay, queue length, queue duration) with their reporting constants. The
`# Invariant` of each node is the full statement; it is not restated here.

## Added: the overtaking families

The overtaking families are counts of the edge-triggered `Event::Maneuver`
records (`crates/tangle-sim/src/event.rs`) whose `kind` is `TacticKind::Overtake`
(`crates/tangle-model/src/source.rs`): one record per documented maneuver
transition, carrying `edge: ManeuverEdge` and `from`/`to: ManeuverState`
(`crates/tangle-sim/src/stage.rs`). All four are countable records in the bucket.

1. **`overtake_attempts`** — unit records. Formula: the records with
   `edge == Attempted` (`following -> preparing`).
2. **`overtake_commits`** — unit records. Formula: the records with
   `edge == Committed` (`preparing -> committed`).
3. **`overtake_completions`** — unit records. Formula: the records with
   `edge == Completed`, `from == Committed`, `to == Returning` — the
   `committed -> returning` edge the contract names.
4. **`overtake_aborts`** — unit records. Formula: the records with
   `edge == Aborted` (`preparing -> aborted`, `committed -> aborted`, or
   `aborted -> following`).

Applicability: always defined for a completed run; the reported value is the
count, and it is `0` when the family is absent. A countable family is never
`not_observed` or `not_applicable`: an absent count is an observed zero, never an
absent value.

Tie-break: no selection — a count is a deterministic function of the event
stream, and the stream is sorted by `Event::order_key` (ascending `AgentId`, then
`EventKind::order`, then the variant's own key), so the count is stable.

Disaggregation:

- **mode pair** — `ModePair::of` (`crates/tangle-sim/src/metrics.rs`) of the
  maneuvering agent's mode and its `partner`'s mode, keyed by the v1 labels
  `vehicle_vehicle` / `vehicle_pedestrian` / `pedestrian_pedestrian`. A record
  with no `partner` (a maneuver that targets an offset rather than a body) has no
  mode pair and contributes to the run-level and facility buckets only.
- **movement** — v1's pairwise rule: the pair's two movement keys, the tagged
  union of `MovementId` for vehicles and `PedestrianRouteId` for pedestrians,
  sorted lexicographically and joined with `|`; a record with no `partner` is
  keyed by its agent's own movement key. A body with no assigned movement
  contributes to no movement bucket, matching v1.
- **facility** — the record's `source_facility`, spelled `facility:<name>` from
  `CompiledScenario::facility_name` (`crates/tangle-model/src/compiled.rs`).

## Added: the close-pass families

The close-pass families are read from the closed `ClosePass` observations
(`crates/tangle-sim/src/close_pass.rs (OvertakeObservation)`), one observation
per participant pair, closed exactly once on pass completion, abort, or a
participant despawn (and, at run end, for every interval still open). The
observation is also emitted as `Event::ClosePass`
(`crates/tangle-sim/src/event.rs`) for a pass on a compiled shared facility, but
the families are the observation records, so the metric set is the tracker's
`overtakes()` slice rather than the event stream.

1. **`close_passes`** — countable, unit observations. Formula: the number of
   closed observations in the bucket, one per participant pair. Applicability:
   always defined for a completed run; the reported value is the count, `0` when
   the bucket recorded none. Tie-break: no selection — the observations are
   recorded in close order (ascending end tick, then canonical ascending
   `(AgentId, AgentId)` pair), so the count is deterministic.

2. **`close_pass_minimum_clearance_m`** — unit metres (signed). Formula: per
   observation, `OvertakeObservation::min_clearance_m` — the least signed surface
   clearance of the tick-swept pair over the interval, from
   `crates/tangle-sim/src/metrics.rs (tick_minimum_clearance_m)`; positive is the
   disjoint distance, zero is contact, negative is penetration. The record
   carries the minimum's simulated time `min_clearance_time_s` (seconds) and the
   relative speed `relative_speed_mps` (metres per second) at that minimum, the
   passing agent's speed minus the passed body's along the shared reference. The
   bucket reports the distribution of the per-observation minima, each with its
   time and relative speed. Applicability: **not applicable** for a bucket that
   cannot host a pass — a mode pair in which neither body's compiled mode
   template declares the `overtake` or `pass` tactic, or a facility whose
   compiled `lateral_use` is `centered` (no free lateral motion); **not observed**
   for a bucket that could host a pass but recorded no observation; otherwise
   reported, including a true zero or negative clearance (contact or penetration),
   which is a reported value and never an absent one. Tie-break: no selection for
   the distribution (close order); a scalar bucket minimum uses the
   strictly-less rule, keeping the first observation on a tie.

3. **one duration series per declared clearance band, keyed by its stable
   `ClearanceBandId`** — unit seconds. Formula: for each observation, each
   participating band's `ClosePassBand::duration_s` — the seconds the pair's exact
   clearance sat at or below the band's `threshold_m` (`CompiledClearanceBand`,
   `crates/tangle-model/src/compiled.rs`; `ClearanceBandId`) while the pair was
   alongside, accumulated whole steps by
   `ClosePassTracker`'s band accumulator. A band participates when
   `CompiledClearanceBand::applies_to` holds for either body's mode template. The
   series is keyed by the band's stable compiled id and reported in the
   scenario's declaration order, so a band is never merged with another.
   Applicability: **not applicable** for a bucket where the band's
   `applies_to_modes` gate excludes the pair (a band naming modes no body
   carries); **not observed** for a bucket that could host a pass but recorded no
   observation; otherwise reported, including the true `0.0` of an observation
   whose clearance stayed above the band. Tie-break: the series is in declaration
   order; each value is the sum of the whole step over the ticks the clearance was
   inside the band, accumulated in observed-tick order.

4. **`close_pass_violations`** — countable, unit observations. Formula: the
   observations with a non-empty `violating_bands` — a participating band whose
   compiled `violation` is `true` and whose observed minimum fell strictly below
   the band's threshold. Applicability: always defined for a completed run; the
   reported value is the count, `0` when the bucket recorded none. Tie-break: no
   selection — a count over the observations in close order.

Disaggregation of the close-pass families:

- **mode pair** — `ModePair::of` the passing agent's mode and the passed body's
  mode, keyed by the v1 labels.
- **movement** — v1's pairwise rule: the pair's two movement keys, sorted and
  joined with `|`. A body with no assigned movement contributes to no movement
  bucket, matching v1.
- **facility** — the shared facility the pass happened on
  (`OvertakeObservation::facility`), spelled `facility:<name>`. An observation
  with no shared compiled facility contributes to the run-level and mode-pair
  buckets and to no facility bucket, matching v1's rule that a body with no route
  contributes to no route-keyed bucket.
- **applicability** — every bucket reports its status; the report keeps
  `not_applicable` (the pass predicate cannot apply) distinct from `not_observed`
  (the predicate applies but nothing occurred), and neither is written as a `0`.

## Where the version appears

`metric_definition_version: 3` is written beside every reported metric value in
the run summary, the run metrics artifact, the batch aggregation, the paired
comparison, the convergence report, and the experiment report
(`apps/tangle-cli/src/run_dir.rs`, `aggregate.rs`, `compare.rs`, `converge.rs`,
`experiment.rs`, each of which writes the constant), and every aggregated and
compared distribution repeats it. The aggregation and comparison refuse an
artifact from another revision rather than mixing revisions
(`apps/tangle-cli/src/aggregate.rs`, `compare.rs`).

## Version-bump rule

`metric_definition_version` is `3` now. Increment it when any reported metric's
name, formula or derivation, unit, applicability or quality rule, tie-break, or
disaggregation key changes; when the reported metric set changes; or when a
declared reporting constant changes a reported value or its applicability. The
v1 and v2 constant lists are carried forward; v3 adds no constant — the
close-pass bands are authored scenario data (`clearance_bands[]`), not a
reporting constant. Editorial text that changes no reported value does not bump
the version. The bump is deliberate and lands with the code change; two definition
revisions never share a version.

## Added under this same version, not defined here

The wrong-way families — `wrong_way_intervals`, `wrong_way_distance_m`,
`wrong_way_duration_s`, `wrong_way_exposure_agent_s`, `wrong_way_encounters`
(unique partners), and `wrong_way_conflicts`, disaggregated by mode, movement,
facility, participant pair, and the perceived rule so a legal opposing traversal
is reported under its rule and never as a violation — are added under this
**same** v3 by [[TAS-124-add-disaggregated-wrong-way-metrics-and-version]]. They
are not defined in this node. The interaction-classification labels
(`CC-OVERTAKE`, `CC-OPPOSE`) belong to their own leaf.

## What v3 does not claim (deferred, named)

- **Level of service** — no implementation anywhere in the workspace; v3 does not
  claim it, exactly as v1 and v2 did not.
- **Delay against a free-flow reference** — no free-flow or reference-speed
  baseline exists in the model; v3 claims no time-lost measure, as v2 did not.
- **Batch-level slices of the increment-2 families beyond what v2 named** —
  aggregating the new mode-pair, movement, and facility close-pass slices at batch
  level belongs to [[TAS-141-surface-close-pass-metric-families]].

# Done when

- The invariant defines every overtaking and close-pass family this increment
  reports — the overtaking attempts, commits, completions, and aborts and the
  close-pass count, minimum clearance (with its time and relative speed), per-band
  duration series keyed by `ClearanceBandId`, and violations — with formula, unit,
  applicability, tie-break, and mode-pair, movement, and facility disaggregation.
- The wrong-way families are named as added under this same v3 by
  [[TAS-124-add-disaggregated-wrong-way-metrics-and-version]], and the deferred
  families are named.
- `metric_definition_version` is fixed at 3 and the version-bump rule is
  restated.
- The node is resolved (the act of settling it) and is the definition the
  Increment 2 run metrics artifact, aggregation, and comparison report cite.

# Result

Settled metric definition v3 and resolved the node. The invariant above names
every family this increment's close-pass report adds at v3, from the surfaces the
contract fixes:

- the overtaking families `overtake_attempts`, `overtake_commits`,
  `overtake_completions`, and `overtake_aborts`, each a count of the edge-triggered
  `Event::Maneuver` records whose `kind` is `TacticKind::Overtake`
  (`crates/tangle-sim/src/event.rs`, edges `crates/tangle-sim/src/stage.rs`);
- the close-pass families `close_passes`, `close_pass_minimum_clearance_m` (with
  its time and relative speed), one duration series per declared band keyed by its
  stable `ClearanceBandId`, and `close_pass_violations`, each read from the closed
  `ClosePass` observations (`crates/tangle-sim/src/close_pass.rs
  (OvertakeObservation)`, `tick_minimum_clearance_m` in
  `crates/tangle-sim/src/metrics.rs`).

`METRIC_DEFINITION_VERSION`
(`apps/tangle-cli/src/run_metrics.rs`) is fixed at 3; every other consumer already
reads the constant, and the sole hardcoded version literal
(`apps/tangle-cli/tests/experiment_spec.rs`) was updated to the constant. No
aggregation behavior changed: the accumulation is
[[TAS-140-accumulate-close-pass-metric-families]] and the run-artifact and
aggregate/compare surface is [[TAS-141-surface-close-pass-metric-families]].

The wrong-way families are added under this same v3 by
[[TAS-124-add-disaggregated-wrong-way-metrics-and-version]]; they are not defined
here. Level of service, a free-flow delay reference, and the batch-level slices
beyond what v2 named remain deferred.

This node carries [[DEF-005-metric-definition-v2]] forward unchanged and adds the
close-pass report's families; v2 stays the authority for artifacts already
reported at v2.
