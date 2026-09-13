---
context_rev: 1
priority: P1
updated: 2026-09-13T01:05:00Z
summary: Slice C1a of Phase 1 Increment 5 persists the versioned, disaggregated metric values into the immutable run directory as metrics.json (linked to the manifest and to metric_definition_version 1) so multi-seed aggregation can read mode- and movement-sliced metrics from artifacts.
next: Read DEF-004 and the run-directory writer, then emit metrics.json with run-level, per-mode-pair, and per-movement metric minima plus event counts, and test the values against `Simulation::interaction_metrics()`.
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
`tangle-model` or `tangle-sim`.

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
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

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

Pending.
