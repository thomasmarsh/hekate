---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice C1a of Phase 1 Increment 5 persists the versioned, disaggregated metric values into the immutable run directory as metrics.json (linked to the manifest and to metric_definition_version 1) so multi-seed aggregation can read mode- and movement-sliced metrics from artifacts.
---

# Outcome

Today the run directory records the event stream and trajectory samples but not
the interaction-metric values themselves, which live only in the process and
differ per seed. Multi-seed aggregation therefore has nothing to read. This
slice makes the run directory self-describing for metrics: it writes a
`metrics.json` artifact alongside `manifest.json`, `summary.json`,
`events.jsonl.gz`, and the Parquet trajectories.

`metrics.json` records, at `metric_definition_version: 1` (per the settled
[[DEF-004-metric-definition-v1]]):

- the run link (`manifest_sha256`), so every number ties back to its manifest;
- the interaction-metric minima the run reports — minimum time to collision,
  minimum surface separation, minimum post-encroachment time — with their
  applicability/quality status (a not-applicable metric is emitted as "no
  value", never as `0`);
- the same separation minimum disaggregated by `ModePair` (mode);
- the same metrics disaggregated by movement, using the `MovementId` /
  `PedestrianRouteId` identity DEF-004 chose, keyed by the pair's two movement
  keys;
- the countable event families, with the mode and movement slices the records
  and the compiled scenario allow.

Immutability is preserved: `metrics.json` is one of the artifacts written once
when a run completes, and the batch resume logic treats its presence like the
other artifacts. Existing artifacts keep their formats; the canonical trace,
goldens, and Phase 1 baseline stay byte-identical. No dependency change in
`hekate-model` or `hekate-sim`.

# Done when

- A completed run directory contains `metrics.json` with
  `metric_definition_version: 1` and the `manifest_sha256` link.
- The file records the run minima, the per-`ModePair` separation minimum, the
  per-movement (pair of movement keys) metric minima, and event counts with
  their mode and movement slices.
- Not-applicable metrics are emitted as an explicit no-value status, not `0`.
- Tests compare the written metric values against the in-process
  `Simulation::interaction_metrics()` (and the event stream) for a real
  scenario, and cover the movement/mode slices.
- The canonical trace format, trace golden, trace hash golden, Phase 1 baseline,
  and all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `tangle check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

[[DEF-004-metric-definition-v1]] fixed metric definition v1 and explicitly left
the emission of `metric_definition_version` and the metric block to the
aggregation work. This child is that emission for the per-run artifact; the
aggregation command (the next direct child) reads these files across seeds and
computes distributions and confidence intervals. The `batch` writer (TAS-035)
and the run-directory writer (TAS-033/034) are the seam it extends.

This slice carries part of the Increment 5 gate bullet "every reported metric
links back to manifest(s) and a versioned metric definition".

# Result

Landed. A completed run directory now holds `metrics.json`, a versioned,
self-describing metric artifact written once alongside `manifest.json`,
`summary.json`, `events.jsonl.gz`, and `trajectories.parquet`, by `run`,
`batch`, and the library writer alike. No scenario-schema change, no
record-shape change, `EVENT_VERSION` stays 2, and no dependency was added.

## What landed

- `apps/hekate-cli/src/run_metrics.rs` (new) owns the artifact.
  `RunMetricsRecorder` counts each completed step's events and remembers every
  agent that emitted one; `RunMetricsRecorder::finish` then reads the live
  `Simulation::interaction_metrics()` minima and the `PostEncroachment` list off
the still-live simulation and disaggregates them. `RunMetricsArtifact` is the
  on-disk `metrics.json`: `metric_definition_version`
  ([`METRIC_DEFINITION_VERSION`] = 1), `manifest_sha256`, the run-level minima
  `minimum_ttc_s` / `minimum_separation_m` / `minimum_post_encroachment_s`,
  `mode_pair_minimum_separation_m`, `movement_minima`, and `event_counts`.
  `MetricValue` carries a `status` of `reported` / `not_applicable` /
  `not_observed` and omits `value` when there is none, so a no-value metric is
  never a false `0`.
- `apps/hekate-cli/src/run_dir.rs` adds `RunDirectoryRequest.metrics` and
  writes `metrics.json` (after the event stream, summary, and trajectories,
  before the `manifest.json` completion marker). All other artifact formats are
  unchanged.
- `apps/hekate-cli/src/trace.rs` adds `canonical_run_captured`, the one run
  loop that also returns the metric capture; `canonical_run_sampled` now
  delegates to it, so the trace bytes and hash cannot drift.
- `apps/hekate-cli/src/main.rs` uses the capturing loop when `--run-dir` is
  given; `apps/hekate-cli/src/batch.rs` does the same per seed and adds
  `METRICS_FILE` to the run artifacts a partial directory may hold, so resume
  treats `metrics.json` like the other artifacts. `lib.rs` re-exports the new
  surface.
- `crates/hekate-sim/src/metrics.rs` (widened, read-only and additive) adds
  `InteractionMetrics::pair_minimum_ttc_s`, mirroring the existing
  `pair_minimum_separation_m` (same swept candidate set, same strictly-less
  tie-break, symmetric arguments) but reporting `None` for a candidate pair
  that never closed. No existing metric value, definition, tie-break, candidate
  set, or event changed, and no re-export in `sim.rs`/`lib.rs` was needed
  because `InteractionMetrics` is already exported.
- `apps/hekate-cli/tests/run_metrics.rs` (new) is the contract suite.

## The artifact

At `metric_definition_version: 1` with the run's `manifest_sha256`, the
artifact holds:

- the run-level minima, each a `{status, value?, agent?, other?, mode_pair?,
  tick?}` value;
- `mode_pair_minimum_separation_m` for all three `ModePair`s;
- `movement_minima`, keyed by the pair's two movement keys;
- `event_counts.total`, `by_family` (every family, `0` when absent),
  `by_family_kind` (violations, control transitions), and the sparse
  `by_family_mode` / `by_family_movement` slices.

**Movement bucket rule.** A movement key is spelled `movement:<name>` for a
vehicle movement and `pedestrian_route:<name>` for a pedestrian route, the
tagged union of `MovementId` and `PedestrianRouteId` DEF-004 chose. A pairwise
metric belongs to the bucket named by its two bodies' movement keys sorted
lexicographically and joined with `|`, so a pair and its mirror share one
bucket; the bucket's value is the least over the per-pair accessors
(`pair_minimum_separation_m`, `pair_minimum_ttc_s`) and the recorded
`post_encroachments()` for the pairs whose two movements name it. A body with
no assigned movement contributes to the run-level and mode-pair minima but to
no movement bucket. Enumerating the pairs is a completion-time step bounded by
the square of the spawned-agent count.

## Claim and write set

Base hash
`8b6b069eac84f2633cfebe87603d37db6f0687d01f4484acfe95ee15306b6e85`
from `tangle hash TAS-037`, claimed by `worker` for 14400 s. Write set:
`apps/hekate-cli/src/**`, `apps/hekate-cli/tests/**`, and this node's own
frontier transition, plus the supervisor-sanctioned widening
`crates/hekate-sim/src/metrics.rs`. Every changed, created, and moved path
stayed inside it; no `Cargo.toml` or `Cargo.lock` changed and the
`sim.rs`/`lib.rs` re-export was not required.

## Dependencies reported

None. The per-movement enumeration is a completion-time double loop over the
spawned agents the recorder observed; no new crate is used.

## Preservation

No file under `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/`
changed, and `EVENT_VERSION` stays 2 (`crates/hekate-sim/src/event.rs`). The
golden, hash-golden, and Phase 1 baseline guards still pass. The manifest,
summary, event stream, and Parquet formats are untouched.

## Evidence

`cargo test --workspace --all-features` passed on this tree, including the new
`apps/hekate-cli/tests/run_metrics.rs` (4 tests), the new kernel test
`the_pass_records_each_observed_pairs_least_time_to_collision`, and the unit
tests in `run_metrics.rs`:

- `metrics_json_carries_the_versioned_run_minima_and_slices` writes the mixed
  benchmark run and checks every run-level minimum against the in-process
  `interaction_metrics()`, every `ModePair` separation, every movement bucket
  against a recomputation from the per-pair accessors, and the whole event
  count block against an independent count of the decompressed stream.
- `a_not_applicable_metric_is_a_status_and_never_zero` runs the walking
  scenario: the time to collision is `not_applicable` with no value, the
  post-encroachment time is `not_observed`, and the movement slice is empty.
- `an_absent_event_family_is_reported_as_zero` pins the zero-present contract.
- `run_and_batch_write_metrics_and_resume_treats_it_like_the_other_artifacts`
  shows `run --run-dir` and `batch` both write it, that a `metrics.json`-only
  directory is re-run rather than refused, and that a resume mutates no
  completed run.

## Gates

All five passed on this tree: `cargo test --workspace --all-features` (0
failures), clippy with `-D warnings`, `cargo fmt --all --check`,
`./scripts/check-dependency-direction.sh` (`dependency direction OK`), and
`tangle check nodes` (`graph check: passed`).

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`e473a30`), verified by rerunning the five gates on that tree.

1. **A completed run directory contains `metrics.json` with
   `metric_definition_version: 1` and the `manifest_sha256` link.**
   `run_dir.rs:382` builds `RunMetricsArtifact::new` and `:388` writes it before
   the `manifest.json` completion marker; `run_metrics.rs:68` fixes
   `METRICS_FILE`, `:75` fixes `METRIC_DEFINITION_VERSION` at 1, and `:324`
   stamps `manifest_sha256` from the manifest bytes. Both `run --run-dir`
   (`main.rs:235`) and `batch` (`batch.rs:416`) write it. Asserted by
   `tests/run_metrics.rs:368` (version and manifest link) and `:697` (the
   command wiring).
2. **The file records the run minima, the per-`ModePair` separation minimum,
   the per-movement (pair of movement keys) metric minima, and event counts
   with their mode and movement slices.** `run_metrics.rs:286`/`:304` define
   `RunMetrics`/`RunMetricsArtifact`; `:398` reads the live run-level minima,
   `:504` the `ModePair` slice, `:524` the movement slice (bucket rule in the
   module doc at `:44`; `:682` spells the tagged movement key, `:697` the sorted
   bucket key), and `:434` the event counts with their `by_family`,
   `by_family_kind`, `by_family_mode`, and `by_family_movement` slices. Asserted
   by `tests/run_metrics.rs:368` against the in-process accessors and the
   decompressed stream.
3. **Not-applicable metrics are emitted as an explicit no-value status, not
   `0`.** `run_metrics.rs:84` defines `reported` / `not_applicable` /
   `not_observed`, and `MetricValue` (`:104`) omits `value` for the no-value
   statuses, so it can never serialize as `0`. Asserted by
   `tests/run_metrics.rs:615` (time to collision `not_applicable` with no value,
   post-encroachment time `not_observed`) and by the round-trip unit test
   `the_artifact_round_trips_through_json` in `run_metrics.rs`.
4. **Tests compare the written metric values against the in-process
   `Simulation::interaction_metrics()` (and the event stream) for a real
   scenario, and cover the movement/mode slices.**
   `tests/run_metrics.rs:368` runs `mixed_interaction_v1` and compares every
   run-level minimum, every `ModePair`, every movement bucket, and the whole
   event-count block against an independent count of the decompressed stream;
   `:615` covers the walking scenario and the empty movement slice. The real
   per-pair time to collision the movement slice needs is the new read-only
   accessor `interaction_metrics::pair_minimum_ttc_s` (`metrics.rs:569`),
   covered by the kernel test
   `the_pass_records_each_observed_pairs_least_time_to_collision`
   (`metrics.rs:1130`).
5. **The canonical trace, trace golden, trace hash golden, Phase 1 baseline,
   and all goldens are unchanged; `EVENT_VERSION` stays 2.** The implementation
   commit touches only `apps/hekate-cli/src`, `apps/hekate-cli/tests`,
   `crates/hekate-sim/src/metrics.rs`, and this node; no file under
   `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/` changed, and
   `EVENT_VERSION` stays 2 (`crates/hekate-sim/src/event.rs`). `trace.rs:169`
   keeps one run loop, so the golden and baseline guards
   (`tests/golden_trace.rs`, `tests/baseline.rs`) and the run-directory stream
   round-trip still pass. No dependency was added, so `Cargo.toml` and
   `Cargo.lock` are unchanged and `hekate-model` and `hekate-sim` gained none.
6. **The five gates pass on the final tree** (rerun on `e473a30`): `cargo test
   --workspace --all-features` passed with 496 tests and 0 failures; clippy with
   `--all-targets --all-features -- -D warnings` was clean; `cargo fmt --all
   --check` was clean; `./scripts/check-dependency-direction.sh` printed
   `dependency direction OK`; `tangle check nodes` printed
   `graph check: passed (64 nodes)`.

Outcome complete: the versioned, disaggregated `metrics.json` artifact, its
immutable write-once placement, the batch resume treatment, and the gate
evidence are on the committed tree.

## Handoff note

`TAS-031`'s `next` still names this node and its exclusive write set excludes
the parent, so this worker did not edit it; advancing that pointer is the
coordinator's, per [[FBK-011-skill-md-s-mutation-rules-say-advancing-a-coordi]].
The widening the coordinator sanctioned was exactly
`crates/hekate-sim/src/metrics.rs`; no re-export in `sim.rs` or `lib.rs` was
required.
