---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice C1b of Phase 1 Increment 5 adds an `aggregate` command that reads a batch's per-seed metrics.json files and writes a machine-readable aggregation with per-metric distributions and confidence intervals, disaggregated by mode and movement and linked to each run manifest and metric_definition_version 1.
---

# Outcome

Add an `aggregate` command to the Hekate CLI that reads a completed batch (its
`batch.json` and each seed's `metrics.json`) and writes a machine-readable
aggregation artifact. For every reported metric it computes the across-seed
distribution — count, mean, spread, and a confidence interval — disaggregated by
mode (`ModePair`) and by movement (the DEF-004 movement key), and links every
reported number back to its run manifest(s) and to
`metric_definition_version: 1`.

Requirements:

- The CI method is documented, deterministic, and appropriate for small seed
  counts (a Student-t interval with a small critical-value table, or a documented
  normal approximation with the exact rule stated). It must not depend on wall
  time, randomness, or hash-map order.
- Not-applicable and not-observed values are excluded from a metric's CI and
  instead counted per seed, so a metric with few reported seeds reports its
  reported `n` rather than silently averaging over a fabricated zero.
- Every aggregated metric records the manifest hash(es) it came from and
  `metric_definition_version: 1`, satisfying the increment gate "every reported
  metric links back to manifest(s) and a versioned metric definition".
- Ordering is deterministic (metrics, slices, and seeds in stable order).
- The command writes an aggregation file (for example `aggregation.json`) under
  the batch root or a named output path, and mutates no run artifact.

If the slice outgrows one session, land the first bounded increment (for example
the aggregation engine plus the run-level metrics), commit it with `Refs
TAS-038`, leave the node `active` with a concrete `next` naming the remaining
disaggregation, and report — do not attempt the whole thing in one pass at the
cost of a coherent handoff.

# Done when

- `aggregate` reads a batch and writes a machine-readable aggregation with
  per-metric count, mean, spread, and a documented confidence interval.
- The aggregation is disaggregated by mode and by movement.
- Every aggregated metric links to its run manifest(s) and to
  `metric_definition_version: 1`.
- Not-applicable/not-observed values are counted, not treated as zero.
- The output ordering is deterministic, covered by a test.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `tangle check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

[[DEF-004-metric-definition-v1]] fixed the metric names, units, applicability,
and the movement key. TAS-037 (C1a) made each run directory self-describing:
`metrics.json` carries `metric_definition_version: 1`, the `manifest_sha256`
link, run-level minima, the per-`ModePair` separation minimum, per-movement
minima, and event counts. This child turns N such files into distributions and
confidence intervals.

The common-random-number seed bank and paired A/B comparison are the next direct
child (C2); the convergence runner and sensitivity report (D) consume this
aggregation.

# Result

Landed in one session; nothing was deferred. Claimed by `worker` for 14400 s
with base hash
`1cf27972184b3e786c502dec917caff025946a63eaac41d8316a581fe1c4c22f`
from `tangle hash TAS-038`; this node moved `nodes/proposed/` →
`nodes/active/` as part of the claimed edit. Write set: `apps/hekate-cli/src/**`,
`apps/hekate-cli/tests/**`, and this node's own frontier transition. No
`Cargo.toml` or `Cargo.lock` change, no kernel change, no new dependency, and
no other node touched.

## What landed

`apps/hekate-cli/src/aggregate.rs` (new) owns the aggregation.
`aggregate_batch(root)` reads `batch.json`, then each run it names, and for every
seed reads the run's `manifest.json` (hashing its bytes) and `metrics.json`
before it is aggregated:

- the batch manifest's `runs` are sorted by ascending seed and a repeated seed
  is refused, so the seed order is the aggregation's own property;
- each run manifest's bytes must hash to the `manifest_sha256` the batch
  recorded, and the run's metrics artifact must link to that same manifest and
  report `metric_definition_version: 1`; otherwise the aggregation fails rather
  than averaging a run that changed after the batch finished.

The artifact (`aggregation.json`, `AGGREGATION_VERSION` 1) holds the batch link
(path, SHA-256, format version), the seeds read (seed, directory,
`manifest_sha256`, `metrics_sha256`), the documented method, and three slice
families:

- `metrics` — whole-batch distributions keyed by the run artifact's own metric
  names: `minimum_ttc_s`, `minimum_separation_m`,
  `minimum_post_encroachment_s`, `event_counts.total`,
  `event_counts.by_family.<family>`, and
  `event_counts.by_family_kind.<family>.<kind>`. The metric set is read from the
  artifacts (`run_level_readings`), so a later definition revision that adds a
  metric is aggregated without a change here;
- `mode_pair_slices` — keyed by the `ModePair` label, holding that pair's
  `minimum_separation_m`, the only metric a run reports per mode pair;
- `movement_slices` — keyed by the bucket of two movement keys, holding the
  bucket's `minimum_separation_m`, `minimum_ttc_s`, and
  `minimum_post_encroachment_s`, with the bucket's sorted `movement_keys`.

Every distribution is `MetricDistribution`: `metric_definition_version: 1`, its
unit (`seconds`, `metres`, `records`), `count`, `mean`, `spread`
(`minimum`/`maximum`/`range`/`sample_variance`/`sample_standard_deviation`), the
`confidence_interval`, the per-seed `reported_seeds` /
`not_applicable_seeds` / `not_observed_seeds`, and the `manifests` of the
reported seeds in `reported_seeds` order. So a metric with three reported seeds
of ten reads `count: 3` and never averages over seven fabricated zeros, and
every averaged number links to the manifests it came from.

**Coverage rules.** A whole-batch metric must cover every seed — a metric one
seed's artifact does not report at all is a `MissingMetric` failure, not a
silently smaller sample. A slice bucket is sparse by nature, so a bucket a run's
artifact does not carry is counted `not_observed` for that run; every slice
distribution's three status lists therefore sum to the batch's seed count.

**Interval.** Two-sided Student-t on the sample mean at 95%:
`mean ± t(0.975, n - 1) * s / sqrt(n)` with the `n - 1` sample standard
deviation, from the published table `T_CRITICAL_975` (1 to 30 degrees of
freedom) and `NORMAL_CRITICAL_975` above it; an unreported constant table, no
wall clock, no randomness, no iteration order. Fewer than two reported seeds
reports `confidence_interval: null` and `sample_variance: null`, because one
observation measures no spread. The interval is not clamped to the metric's
physical range.

`apps/hekate-cli/src/main.rs` adds the `aggregate` subcommand
(`hekate-cli aggregate <BATCH_ROOT> [--output <PATH>]`, default
`<BATCH_ROOT>/aggregation.json`, `-` for stdout) with its `--help` contract, and
`lib.rs` re-exports the new surface. The command reads only: it opens
`batch.json` and each run's manifest and metrics and writes exactly one derived
file, so no run artifact is created, changed, or removed.

## Evidence

`apps/hekate-cli/tests/aggregate.rs` (9 tests) plus the module's 4 unit tests:

- `every_reported_metric_gets_a_count_mean_spread_and_interval` pins count,
  mean, spread, and the interval against hand-computed values (`1,2,3,4` → mean
  2.5, variance 5/3, `t(3) = 3.182446305`), the event-count distributions, the
  full metric key set, and the documented method block;
- `not_applicable_and_not_observed_seeds_are_counted_not_zeroed` shows the mean
  of the reported seeds only, the per-status seed lists, and that every metric's
  three statuses cover every seed;
- `a_single_reported_seed_has_no_interval` pins the small-sample rule;
- `the_mode_pair_and_movement_slices_disaggregate_by_mode_and_movement` covers
  all three mode pairs, the bucket keys and metrics, and a bucket one seed does
  not carry being counted not observed;
- `every_aggregated_metric_links_to_its_manifests_and_definition_version` walks
  every distribution in the artifact (metrics and both slice families) and
  checks the definition version, the unit, and the manifest link of every
  reported seed, plus the batch link and the seed links against the files on
  disk;
- `a_real_batch_aggregates_its_mode_and_movement_slices` runs the mixed
  benchmark for seeds 0,1,2 and checks the metric set against an independent
  re-derivation from the run artifact, the interval rule per metric, and the
  mode and movement slices;
- `the_output_ordering_is_deterministic` aggregates two batches whose manifests
  name the same seeds in opposite orders and compares everything but the batch
  link, then checks every key list and seed list is ascending and that the
  command writes exactly the library's bytes;
- `a_run_that_disagrees_with_the_batch_is_refused` covers the manifest-link,
  definition-version, changed-manifest, missing-metric, and absent-batch cases;
- `the_aggregate_command_writes_the_aggregation_and_mutates_no_run_artifact`
  runs the binary over a real batch, shows the walking run's not-applicable time
  to collision reported as three counted seeds with no mean, and shows every run
  artifact byte-identical before and after (and unchanged by a second run and by
  `--output -`).

The module's unit tests pin the critical-value table, the Student-t interval
arithmetic, the spread, and the accumulator's status accounting.

## Gates

All five passed on the implementation tree: `cargo test --workspace
--all-features` (0 failures), clippy with `--all-targets --all-features -- -D
warnings`, `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`
(`dependency direction OK`), and `tangle check nodes` (`graph check: passed`).
No file under `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/`
changed, and `EVENT_VERSION` stays 2.

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`3e58717`), verified by rerunning the five gates on that tree.

1. **`aggregate` reads a batch and writes a machine-readable aggregation with
   per-metric count, mean, spread, and a documented confidence interval.**
   `aggregate.rs:484` reads `batch.json` and each run's `manifest.json` and
   `metrics.json`; `aggregate.rs:421` defines the distribution
   (`metric_definition_version`, `unit`, `count`, `mean`, `spread`,
   `confidence_interval`, the three seed lists, `manifests`);
   `aggregate.rs:762` computes the statistics and `aggregate.rs:399` the
   interval. The method is documented structurally in the artifact
   (`StatisticsMethod::v1`, `aggregate.rs:329`) and read from the published
   table `T_CRITICAL_975` (`aggregate.rs:125`) with `NORMAL_CRITICAL_975`
   (`aggregate.rs:159`) above 30 degrees of freedom. `main.rs:90` declares the
   subcommand, `main.rs:414` its arguments, and `main.rs:429` writes the file
   (default `<BATCH_ROOT>/aggregation.json`). Asserted by
   `tests/aggregate.rs:391` against hand-computed values (mean 2.5, variance
   5/3, `t(3) = 3.182446305`) and by `tests/aggregate.rs:1194` through the
   binary.
2. **The aggregation is disaggregated by mode and by movement.**
   `Aggregation.mode_pair_slices` (`aggregate.rs:456`) is keyed by the `ModePair`
   label and holds that pair's `minimum_separation_m`; `MovementSlice`
   (`aggregate.rs:447`) is keyed by the bucket of two movement keys and holds the
   bucket's three interaction metrics with its sorted `movement_keys`.
   `Accumulation::accumulate` (`aggregate.rs:814`) fills both from the run
   artifact's `mode_pair_minimum_separation_m` and `movement_minima`. Asserted
   by `tests/aggregate.rs:633` (all three mode pairs, the bucket keys and
   metrics, and a bucket one seed does not carry) and by
   `tests/aggregate.rs:890` on the real mixed benchmark.
3. **Every aggregated metric links to its run manifest(s) and to
   `metric_definition_version: 1`.** `MetricDistribution` carries both fields
   (`aggregate.rs:421`), stamped at `aggregate.rs:762` from
   `METRIC_DEFINITION_VERSION` and from the manifest hashes the seeds were read
   with; the aggregation also refuses a run whose metrics artifact links to a
   manifest the batch did not record (`AggregateError::ManifestLink`,
   `aggregate.rs:235`, checked at `aggregate.rs:542`) or whose bytes changed
   (`ManifestDigest`, `aggregate.rs:221`, checked at `aggregate.rs:516`).
   Asserted by `tests/aggregate.rs:795`, which walks every distribution in the
   artifact and checks each reported seed's manifest against the seed list and
   the files on disk.
4. **Not-applicable/not-observed values are counted, not treated as zero.**
   `Accumulator::reading` (`aggregate.rs:705`) records each seed behind
   `reported` / `not_applicable` / `not_observed` separately and
   `Accumulator::finish` (`aggregate.rs:762`) computes every statistic from the
   reported values alone; a movement bucket a run does not carry is counted not
   observed (`fill_not_observed`, `aggregate.rs:753`). A whole-batch metric that
   does not cover every seed is refused rather than averaged over a smaller
   sample (`MissingMetric`, `aggregate.rs:275`, checked at `aggregate.rs:879`).
   Asserted by `tests/aggregate.rs:517` (the mean of the reported seeds only,
   and every metric's three statuses covering every seed) and by
   `tests/aggregate.rs:1194` (the walking run's not-applicable time to collision
   reported as three counted seeds with `mean: null`).
5. **The output ordering is deterministic, covered by a test.** Every map is a
   `BTreeMap`, the seeds are sorted by ascending seed before they are read
   (`aggregate.rs:496`), and `Accumulator::finish` sorts the seed lists
   (`aggregate.rs:762`). Asserted by `tests/aggregate.rs:992`, which aggregates
   two batches whose manifests name the same seeds in opposite orders and
   compares everything but the batch link, checks every key and seed list is
   ascending, and checks the command writes exactly the library's bytes.
6. **The canonical trace, trace golden, trace hash golden, Phase 1 baseline,
   and all goldens are unchanged; `EVENT_VERSION` stays 2.** The implementation
   commit touches only `apps/hekate-cli/src`, `apps/hekate-cli/tests`, and this
   node; no file under `tests/golden/`, `baselines/`, `scenarios/`, or
   `schemas/` changed, `EVENT_VERSION` stays 2
   (`crates/hekate-sim/src/event.rs`), and `Cargo.toml`/`Cargo.lock` are
   unchanged, so no dependency was added. `metrics.json`'s shape is read, not
   modified.
7. **The five gates pass on the final tree** (rerun on `3e58717`): `cargo test
   --workspace --all-features` passed with 509 tests and 0 failures across 47
   test binaries; clippy with `--all-targets --all-features -- -D warnings` was
   clean; `cargo fmt --all --check` was clean;
   `./scripts/check-dependency-direction.sh` printed `dependency direction OK`;
   and `tangle check nodes` printed `graph check: passed (66 nodes)`.

Outcome complete: the across-seed aggregation, its deterministic ordering, the
mode and movement disaggregation, the manifest and definition-version links, and
the counted not-applicable/not-observed statuses are on the committed tree.

## Handoff note

`TAS-031`'s `next` names this node and its exclusive write set excludes the
parent, so this worker did not edit it; advancing that pointer to the next
direct child is the coordinator's, per
[[FBK-011-skill-md-s-mutation-rules-say-advancing-a-coordi]].
