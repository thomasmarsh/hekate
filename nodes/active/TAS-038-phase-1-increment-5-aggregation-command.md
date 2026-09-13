---
context_rev: 1
priority: P1
updated: 2026-09-13T01:33:23Z
summary: Slice C1b of Phase 1 Increment 5 adds an `aggregate` command that reads a batch's per-seed metrics.json files and writes a machine-readable aggregation with per-metric distributions and confidence intervals, disaggregated by mode and movement and linked to each run manifest and metric_definition_version 1.
next: Verify the five gates on the final tree and resolve the node with per-criterion evidence.
---

# Outcome

Add an `aggregate` command to the Tangle CLI that reads a completed batch (its
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
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

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
from `braintree hash TAS-038`; this node moved `nodes/proposed/` →
`nodes/active/` as part of the claimed edit. Write set: `apps/tangle-cli/src/**`,
`apps/tangle-cli/tests/**`, and this node's own frontier transition. No
`Cargo.toml` or `Cargo.lock` change, no kernel change, no new dependency, and
no other node touched.

## What landed

`apps/tangle-cli/src/aggregate.rs` (new) owns the aggregation.
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

`apps/tangle-cli/src/main.rs` adds the `aggregate` subcommand
(`tangle-cli aggregate <BATCH_ROOT> [--output <PATH>]`, default
`<BATCH_ROOT>/aggregation.json`, `-` for stdout) with its `--help` contract, and
`lib.rs` re-exports the new surface. The command reads only: it opens
`batch.json` and each run's manifest and metrics and writes exactly one derived
file, so no run artifact is created, changed, or removed.

## Evidence

`apps/tangle-cli/tests/aggregate.rs` (9 tests) plus the module's 4 unit tests:

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
(`dependency direction OK`), and `braintree check nodes` (`graph check: passed`).
No file under `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/`
changed, and `EVENT_VERSION` stays 2.
