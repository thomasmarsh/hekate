---
context_rev: 1
priority: P1
updated: 2026-09-13T01:05:00Z
summary: Phase 1 Increment 5 adds the validate/run/batch/replay CLI surface, immutable run directories with bounded sampled trajectories, multi-seed aggregation with common-random-number seed banks, a Fast/Standard/Fine convergence runner with a machine-readable sensitivity report, and release-mode benchmarks and profiler captures before any optimization.
next: [[TAS-037-phase-1-increment-5-run-metrics-artifact]]
---

# Outcome

Per `PHASE_1_PLAN.md` Increment 5 ("Experiments, outputs, and convergence"), the
simulator gains the experiment and reproducibility surface it needs to compare
designs:

- CLI commands for `validate`, `run`, `batch`, and `replay`.
- Immutable run directory containing manifest, summary, event stream, and
  selectively sampled trajectories.
- JSON for manifests/summaries, compressed JSON Lines for sparse typed events,
  and Parquet for sampled trajectories.
- Aggregation across seeds with distributions and confidence intervals,
  disaggregated by mode and movement.
- Common-random-number seed banks for A/B comparisons.
- Fast/Standard/Fine convergence runner and a machine-readable sensitivity
  report.
- Release-mode end-to-end benchmarks and profiler captures before optimization.

Increment 5 records the Increment 4 residuals as its baseline but does not
absorb them: the always-on interaction-metrics pass costs roughly 22 microseconds
per tick in a release build on `mixed_interaction_v1`, and the swept TOI query is
a capability that no tick consumer yet ticks. This increment measures those
states (slice E) and does not optimize or rewire them.

Any schema change stays additive to schema version 1: validate, regenerate
`schemas/scenario-source.schema.json`, and keep the drift test. No schema
version 2. New output dependencies (Parquet, compression) live in the CLI/output
layer, never in `tangle-model` or `tangle-sim`.

Constraint: `f64`/`glam::DVec2`, single-threaded state-affecting tick, stable
ordering with explicit tie-breakers (ascending `AgentId`), no Bevy types in
`tangle-model` or `tangle-sim`. Parallel batch execution is an outer loop over
independent single-threaded runs and must produce identical per-run trace hashes
as serial execution; the tick is never parallelized. Output size is bounded by
the declared sampling policy, with full trajectories opt-in. Do not break
existing goldens or baselines without a deliberate, reported regeneration.

The versioned metric definition the gate requires is admitted as a durable `DEF`
child, not folded into the run-artifact plumbing: settling the metric names,
units, applicability, and version constant is a distinct outcome with independent
resumability that the sensitivity report and Phase 1 Increment 6 both consume.
It is created just in time as a direct child when slice D reaches it.

# Done when

- CLI commands `validate`, `run`, `batch`, and `replay` exist, each with a
  documented contract and tests.
- A run directory is immutable: a batch can be stopped and resumed without
  changing completed run artifacts.
- Manifests and summaries are JSON, sparse typed events are compressed JSON
  Lines, and sampled trajectories are Parquet; a manifest, summary, event
  stream, and selectively sampled trajectories are all present.
- A declared sampling policy bounds output size, and full trajectories are
  opt-in.
- Aggregation across seeds reports distributions and confidence intervals,
  disaggregated by mode and movement.
- Common-random-number seed banks support A/B comparisons.
- A Fast/Standard/Fine convergence runner produces a machine-readable
  sensitivity report.
- Every reported metric links back to its manifest(s) and to a versioned metric
  definition.
- Parallel and serial batch execution produce identical per-run trace hashes.
- Release-mode end-to-end benchmarks and profiler captures are recorded before
  any optimization, including the always-on interaction-metrics cost and the
  unticked swept-TOI state as the baseline.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Area [[IDX-001-tangle]].
Depends on [[TAS-030-phase-1-increment-4-geometry-queries-safety-events]] at context_rev 1.

Increment 4 landed the geometry and safety-event layer this increment's outputs
consume: the exact and swept queries, the typed event union at `EVENT_VERSION`
2, and the online TTC / minimum-separation / PET pass. Increment 5 builds the
CLI, run-artifact, aggregation, convergence, and profiling surface on top
without rewiring the tick.

The planned slices, in order, each an independently resumable direct child:

- A. CLI surface: `validate`, `batch`, and `replay` alongside `run`.
- B. Immutable run directory: manifest/summary/event half, then the Parquet
  sampled-trajectory half.
- C. Aggregation across seeds, then common-random-number seed banks.
- D. Fast/Standard/Fine convergence runner, machine-readable sensitivity report,
  and the durable versioned metric definition.
- E. Release-mode end-to-end benchmarks and profiler captures before
  optimization.
- F. Independent read-only verification of every `Done when` criterion.
- G. Resolution closeout (coordinator-owned).

`TAS-029-bound-red-queue-emergency-cap` (P2) and the `FBK-001`..`FBK-010`
feedback backlog stay `proposed` with their own `next`; neither gates this
increment.
