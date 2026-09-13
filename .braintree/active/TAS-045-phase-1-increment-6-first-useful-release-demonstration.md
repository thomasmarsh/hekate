---
context_rev: 1
priority: P1
updated: 2026-09-13T13:47:55Z
summary: Phase 1 Increment 6 delivers the first useful release demonstration - two scenario-only variants of one small intersection, a checked-in experiment spec and seed bank, immutable run results and a concise comparison report, golden traces with Fast/Standard/Fine convergence evidence and a known-limitations document, one-command reproduction, and a recorded replay with live-view instructions.
next: [[TAS-050-increment-6-replay-and-live-view]]
---

# Context

Area [[IDX-001-tangle]]. This coordinating task owns the Increment 6 outcome in
`PHASE_1_PLAN.md` ("Increment 6 — First useful release demonstration") and its
gate.

Depends on [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]] at context_rev 1.

Increment 6 consumes the Increment 5 experiment surface (immutable run
directories, `aggregate`, `converge`, `compare`, CRN seed banks, the run-metrics
artifact) and the v1 metric definition. The Increment 5 residuals are recorded
here as this increment's baseline but deliberately do not gate it:

- **The always-on interaction-metrics pass is unoptimized.** Release ablation on
  `mixed_interaction_v1` measures roughly 18–33 microseconds per tick (54–75% of
  the tick). Owned by
  [[TAS-051-interaction-metrics-switchable-and-optimized]], a proposed P2 node
  under `IDX-001` that carries the recorded ablation as its baseline. Not in
  scope for this increment.
- **Aggregation disaggregation is narrower than the run artifact's** (slice-F
  finding F6): the batch-level mode slice carries only `minimum_separation_m`,
  and event-family mode/movement counts are not aggregated. Folded into
  [[TAS-048-increment-6-run-and-comparison-report]] because that slice owns the
  comparison report that must show those families by mode and movement.
- **Convergence uses one global 5% relative tolerance.** Folded into
  [[TAS-048-increment-6-run-and-comparison-report]] so the per-metric tolerance
  and the "material sensitivity is reported rather than hidden" gate land
  together.

Metric-definition versioning decision: **supersede, do not edit v1 in place.**
Implementing throughput, delay, and queues changes the reported metric set, so
`metric_definition_version` becomes 2 under the v1 version-bump rule. Because
`DEF-004`'s settled invariant is v1 and two definition revisions never share a
version, this increment admits [[DEF-005-metric-definition-v2]] as the v2
authority; `DEF-004` stays resolved and records `disposition: superseded` with
`Superseded by [[DEF-005-metric-definition-v2]]`, so artifacts already reported
at v1 remain attributable. No consumer may report a metric with the wrong
definition revision; the exact pinned-consumer search
`rg -n -F 'Depends on [[DEF-004-metric-definition-v1]] at context_rev ' .braintree`
is run before and after the bump (it returns zero today). `DEF-005` is a direct
child of [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]], the slice
that implements the metrics and settles the definition from the implemented
surfaces.

Explicitly out of scope, left proposed or blocked, and never gating this
increment: [[TAS-029-bound-red-queue-emergency-cap]] (P2);
[[TAS-044-braintree-feedback-triage]] with the `FBK-001`..`FBK-018` backlog
(P3); and the Phase 2 nodes from [[TAS-017-phase-2-mixed-traffic]] onward. The
Phase 2 increment 0 gate
[[TAS-018-phase-2-increment-0-baseline-extension-contract]] stays blocked.

Constraints carried from Increments 0–5: any schema change stays additive to
schema version 1 (validate, regenerate `schemas/scenario-source.schema.json`,
keep the drift test); no schema version 2; new output dependencies live in the
CLI/output layer, never in `tangle-model` or `tangle-sim`; `f64`/`glam::DVec2`;
single-threaded state-affecting tick; stable ordering with explicit tie-breakers
(ascending `AgentId`); no Bevy types in `tangle-model` or `tangle-sim`; parallel
batch execution is an outer loop over independent single-threaded runs that must
produce identical per-run trace hashes to serial execution, and the tick is
never parallelized; output size is bounded by the declared sampling policy with
full trajectories opt-in; no existing golden or baseline (walking trace, scene
golden, Phase 1 baseline, cell/Kitty goldens) is broken without a deliberate,
reported regeneration; the two variants differ through scenario data, not code;
a geometrically different benchmark scenario must not require simulator-logic
changes; and the determinism gate is `replay --verify` on the same manifest.

# Outcome

Per `PHASE_1_PLAN.md` Increment 6 ("First useful release demonstration"), the
project gains its first end-to-end, reproducible design comparison:

- Two freely described variants of the same small intersection, differing
  through scenario data rather than code.
- A checked-in experiment spec, seed bank, results, and concise comparison
  report.
- Golden traces, convergence evidence, a known-limitations document, and
  one-command reproduction.
- A recorded replay and live-view instructions for visual review.

The increment is decomposed into five sequentially executed slice children plus
the metric-definition revision:

- [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]] — remaining gate
  metrics (throughput, delay, queues, and any unreported conflict/event family)
  and the v2 definition [[DEF-005-metric-definition-v2]].
- [[TAS-047-increment-6-scenario-variants-and-experiment-spec]] — the two
  scenario-only variants, the checked-in experiment spec and seed bank, and the
  proof that a geometrically different benchmark scenario needs no simulator
  change.
- [[TAS-048-increment-6-run-and-comparison-report]] — immutable runs and the
  comparison report, plus F6 event-family aggregation and per-metric convergence
  tolerance.
- [[TAS-049-increment-6-evidence-and-reproduction]] — golden traces, convergence
  evidence, known-limitations document, one-command reproduction.
- [[TAS-050-increment-6-replay-and-live-view]] — recorded replay and live-view
  instructions.

Slices F (independent read-only verification of every Done when criterion) and G
(resolution closeout) are executed by the orchestrator and are not separate
children: verification is read-only and closeout is this node's own resolution.

# Done when

- Two freely described variants of one small intersection exist, differ through
  scenario data rather than code, and are accompanied by a checked-in experiment
  spec and seed bank.
- Results are written into immutable run directories and summarized in a concise
  comparison report.
- The comparison reports throughput, delay, queues, violations,
  collision/contact events, TTC, PET, and minimum separation by mode and
  movement.
- Selected findings remain directionally stable at Fine fidelity; material
  sensitivity is reported rather than hidden, with per-metric convergence
  tolerance.
- Golden traces, a known-limitations document, and one-command reproduction
  exist.
- A recorded replay and live-view instructions exist for visual review.
- The same manifest reproduces the same canonical event stream in the supported
  determinism environment (`replay --verify`).
- A developer can add a geometrically different benchmark scenario without
  changing simulator logic.
- Every reported metric links to its manifest(s) and to the versioned metric
  definition at the revision that produced it (`metric_definition_version` 2 for
  the new metrics; v1 artifacts remain attributed to `DEF-004`); no consumer
  reports a metric with the wrong definition revision.
- Every direct child is resolved or disposed, with per-criterion evidence
  recorded below.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.

# Result

_(to be completed at resolution with per-criterion evidence)_
