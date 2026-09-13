---
context_rev: 1
priority: P1
updated: 2026-09-13T14:13:30Z
summary: Phase 1 Increment 6 delivers the first useful release demonstration - two scenario-only variants of one small intersection, a checked-in experiment spec and seed bank, immutable run results and a concise comparison report, golden traces with Fast/Standard/Fine convergence evidence and a known-limitations document, one-command reproduction, and a recorded replay with live-view instructions.
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

Complete. Every Done when criterion is met, all five direct children are
resolved, an independent read-only review found no P0/P1, and the final tree
passes the five gates. Slices, in execution order:

- A — [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]]: throughput,
  travel time, stopped delay, control delay, queue length, and queue duration
  reported at `metric_definition_version` 2 (commits `306b101`, `07014b7`);
  [[DEF-005-metric-definition-v2]] settled from the implemented surface and
  `DEF-004` superseded (`18c852a`).
- B — [[TAS-047-increment-6-scenario-variants-and-experiment-spec]]: two
  signal-timing variants plus the experiment spec and seed bank (`a9ee7ef`,
  `b85cf20`), an offset-junction benchmark (`7396058`), and the node result
  (`bc41c6f`, `091f164`).
- C — [[TAS-048-increment-6-run-and-comparison-report]]: F6 event-family
  aggregation (`3b34e84`), per-metric convergence tolerance (`575bca4`), the
  experiment command and checked-in comparison report (`0f6616b`), node result
  and resolution (`38d7e11`, `539fe80`).
- D — [[TAS-049-increment-6-evidence-and-reproduction]]: convergence report
  carries all five slice families (`252f885`), convergence evidence and summary
  (`68b7990`), the Fast/Fine step defect fix (`aefc096`), golden traces
  (`07543c7`), limitations and reproduction script (`5ae5338`), resolution
  (`c885421`).
- E — [[TAS-050-increment-6-replay-and-live-view]]: recorded replay capture and
  live-view instructions (`dedc4ff`), resolution (`878e4c1`).

Per-criterion evidence:

1. **Two variants, scenario-data only, spec + seed bank** —
   `scenarios/experiments/four_leg_pedestrian_{ew,ns}_priority_v1.json5` differ
   on exactly the `id` line and six `duration_s` values;
   `experiments/increment6_signal_timing_v1/{experiment.json,seed_bank.json}`
   are checked in. Asserted by `apps/tangle-cli/tests/experiment_spec.rs`.
2. **Immutable run directories and a concise report** —
   `experiments/increment6_signal_timing_v1/runs/` (gitignored, regenerable) and
   `comparison_report.json`; the run-directory immutability contract is
   `apps/tangle-cli/src/run_dir.rs:105-108` with a test.
3. **Gate metrics by mode and movement** — `comparison_report.json` carries ten
   sections (throughput, delay, queues, violations, collisions, near misses,
   TTC, PET, minimum separation, events): operational families and every event
   family per mode and per movement; minimum separation per mode pair, movement
   pair, and run; TTC and PET per movement pair and run. The missing TTC/PET
   mode-pair slice is disclosed and owned by
   [[TAS-052-ttc-and-pet-mode-pair-aggregation]].
4. **Fine-fidelity stability, material sensitivity, per-metric tolerance** —
   `convergence_evidence.json` and `convergence_summary.md`: tolerance relative
   0.05 with an absolute count part, Fast 0.1/Standard 0.05/Fine 0.02 s, three
   selected findings directionally stable at Fine, and absolute sensitivity
   reported per metric rather than hidden.
5. **Goldens, limitations, one-command reproduction** —
   `tests/golden/four_leg_pedestrian_{ew,ns}_priority_v1.trace.{jsonl,sha256}`
   (no existing golden regenerated), `docs/known_limitations.md`, and
   `scripts/reproduce-increment6.sh` (byte-identical artifacts, 60 manifests
   verified).
6. **Recorded replay and live view** —
   `experiments/increment6_signal_timing_v1/replay/ew_priority_seed-1_1200ticks/`
   and `docs/increment6_live_view.md`.
7. **Same manifest reproduces the canonical event stream** — `replay --verify`
   passes on all 60 run directories and both goldens; the Fast/Fine step defect
   that broke this is fixed in `apps/tangle-cli/src/batch.rs:438-441` with
   regression test `a_batch_runs_at_its_declared_step_and_replays_at_it`.
8. **Geometrically different benchmark without simulator change** —
   `scenarios/benchmarks/offset_junction_v1.json5` validates and runs through
   the unmodified kernel; no scenario id is special-cased in `tangle-model` or
   `tangle-sim`.
9. **Metric-to-manifest and metric-to-definition linkage** —
   `METRIC_DEFINITION_VERSION` is 2 in every emitting surface and no literal v1
   is emitted; aggregation and comparison refuse a foreign revision; the report
   carries `metric_definition_version` and manifest links per number; `DEF-004`
   stays v1 and superseded, `DEF-005` is v2.
10. **Every direct child resolved or disposed** — TAS-046, TAS-047, TAS-048,
    TAS-049, and TAS-050 are all in `.braintree/resolved/` with populated
    results.
11. **Five gates on the final tree** — `cargo test --workspace --all-features`
    (584 passed, 0 failed, 1 ignored), clippy `-D warnings`, `cargo fmt --all
    --check`, `./scripts/check-dependency-direction.sh`, and `braintree check`
    all pass at the resolution tree.

Independent verification: read-only reviewer run `71631b07` found no P0/P1 and
four P2 documentation/citation defects plus one scope note; the defects are
closed by `91750f8` (verified line citations in `DEF-005`/`TAS-046`, corrected
live-view controls and regeneration command, added limitation bullets).

# Limitations

- **TTC and PET lack mode-pair slices.** Metric definition v2 disaggregates them
  by movement pair and run only; per-mode-pair aggregation is a future
  definition revision owned by [[TAS-052-ttc-and-pet-mode-pair-aggregation]].
  Disclosed in the report and `docs/known_limitations.md`.
- **The checked-in report's per-run hashes cannot be re-derived from checked-in
  files** because `experiments/increment6_signal_timing_v1/runs/` is gitignored;
  attribution rests on construction and `scripts/reproduce-increment6.sh`.
- **Level of service remains deferred** in `DEF-005`; no gate requires it.
- **Runtime magnitudes are an order of magnitude, not a contract** (one host,
  one toolchain). Release-metrics ablation and the always-on interaction-metrics
  cost remain the Increment 5 baseline, owned by
  [[TAS-051-interaction-metrics-switchable-and-optimized]].
- **Increment 5 residuals:** the always-on interaction-metrics cost is owned by
  TAS-051; F6 event-family aggregation and the single global convergence
  tolerance are closed in TAS-048/TAS-049. None gates this increment.
- **Out of scope and left proposed or blocked:**
  [[TAS-029-bound-red-queue-emergency-cap]],
  [[TAS-044-braintree-feedback-triage]] with the `FBK-001`..`FBK-022` backlog,
  and the Phase 2 nodes from [[TAS-017-phase-2-mixed-traffic]] onward;
  [[TAS-018-phase-2-increment-0-baseline-extension-contract]] stays blocked.
