---
context_rev: 1
priority: P1
updated: 2026-09-13T01:30:00Z
summary: Slice C1b of Phase 1 Increment 5 adds an `aggregate` command that reads a batch's per-seed metrics.json files and writes a machine-readable aggregation with per-metric distributions and confidence intervals, disaggregated by mode and movement and linked to each run manifest and metric_definition_version 1.
next: Read TAS-037's metrics.json shape and the batch manifest, then implement the aggregation (mean, spread, CI) with manifest and definition-version links and tests.
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

Pending.
