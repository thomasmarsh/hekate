---
context_rev: 1
priority: P1
updated: 2026-09-13T03:24:26Z
summary: Phase 1 Increment 5 adds the validate/run/batch/replay CLI surface, immutable run directories with bounded sampled trajectories, multi-seed aggregation with common-random-number seed banks, a Fast/Standard/Fine convergence runner with a machine-readable sensitivity report, and release-mode benchmarks and profiler captures before any optimization.
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
per tick in a release build on `mixed_interaction_v1`, and the swept TOI cast is
on the tick path exactly once, as the safety monitor's per-tick contact
confirmation, with no behavioral consumer (control, admission, signalling, or
yielding) depending on a sweep. This increment measures those states (slice E)
and does not optimize or rewire them.

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
  swept-TOI tick-path state as the baseline.
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

`TAS-029-bound-red-queue-emergency-cap` (P2) and the `FBK-001`..`FBK-018`
feedback backlog stay `proposed` with their own `next`; neither gates this
increment.

# Result

Increment 5 landed as independently resumable children plus the definition
dependency, all integrated and committed:

- A — CLI surface: [[TAS-032-phase-1-increment-5-validate-cli]] (`validate`),
  [[TAS-035-phase-1-increment-5-batch-command]] (`batch`),
  [[TAS-036-phase-1-increment-5-replay-command]] (`replay`), alongside the
  existing `run`.
- B — Immutable run directory:
  [[TAS-033-phase-1-increment-5-run-directory-core]] (manifest, summary,
  compressed event stream, declared sampling policy) and
  [[TAS-034-phase-1-increment-5-parquet-trajectories]] (Parquet sampled
  trajectories).
- C — Aggregation and paired comparison:
  [[TAS-037-phase-1-increment-5-run-metrics-artifact]] (`metrics.json`),
  [[TAS-038-phase-1-increment-5-aggregation-command]] (`aggregate`),
  [[TAS-039-phase-1-increment-5-crn-seed-bank]] (CRN seed bank),
  [[TAS-040-phase-1-increment-5-paired-ab-comparison]] (`compare`).
- D — Convergence: [[DEF-004-metric-definition-v1]] (versioned metric
  definition v1) and
  [[TAS-041-phase-1-increment-5-convergence-runner]] (`converge` and the
  machine-readable sensitivity report).
- E — Baseline: [[TAS-042-phase-1-increment-5-release-benchmarks-profiling]]
  (release benchmarks, tick-phase ablation, profiler captures).
- F — Independent read-only verification (reviewer run `2ac861c5`), no merge
  blocker.
- G — Closeout: [[TAS-043-phase-1-increment-5-slice-f-closeout]] (the one P1 and
  the P2/P3 findings fixed, F6 accepted as a limitation).

The session also migrated the vault to `.braintree/` for Braintree 0.6.0
(commit `edf9463`) and aligned `AGENTS.md` to `braintree check` / `.braintree/`;
that is a tooling-layout change, not part of the increment's product outcome.

# Final verification

Rerun by the coordinator on the final tree (HEAD `9f781c0`):
`cargo test --workspace --all-features` exit 0 with no failed binary;
`cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0;
`cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh`
reports `dependency direction OK`; `braintree check` passes. An independent
read-only review (slice F) verified each criterion against the source and tests
and raised no merge blocker; slice G closed its findings. The coordinator
also confirmed the diff-level preservation claims: the only change under
`crates/` in the increment range is the sanctioned additive `+91`-line per-pair
TTC accessor in `crates/tangle-sim/src/metrics.rs`, and `tests/golden`,
`baselines`, `scenarios`, and `schemas` are untouched.

Per criterion:

1. **CLI commands `validate`, `run`, `batch`, `replay` with documented contracts
   and tests.** `apps/tangle-cli/src/main.rs` declares each subcommand with a
   `long_about` contract (`VALIDATE_LONG_ABOUT`, `RUN_LONG_ABOUT`,
   `BATCH_LONG_ABOUT`, `REPLAY_LONG_ABOUT`); tests
   `apps/tangle-cli/tests/{validate,batch,replay,run_directory}.rs` drive the
   built binary.
2. **Immutable run directory; batch stop/resume unchanged.**
   `apps/tangle-cli/src/run_dir.rs` writes `manifest.json` last and refuses a
   completed or foreign directory without writing;
   `apps/tangle-cli/src/batch.rs` derives resume from disk and clears a partial
   run. Tests `tests/run_directory.rs:331` and `tests/batch.rs`.
3. **JSON manifests/summaries, compressed JSON Lines events, Parquet
   trajectories.** `run_dir.rs` (`manifest.json`, `summary.json`,
   `events.jsonl.gz` gzip), `trajectories.rs` (`trajectories.parquet`); tests
   `tests/run_directory.rs`, `tests/trajectories.rs`.
4. **Declared sampling policy bounds size; full trajectories opt-in.**
   `run_dir.rs` `SamplingPolicy` (default stride 10, cap 100 000; `full()` is
   stride 1, `u64::MAX`) enforced during collection in `trajectories.rs`; CLI
   `--full-trajectories`. Tests `tests/trajectories.rs`.
5. **Aggregation distributions and CIs, disaggregated by mode and movement.**
   `apps/tangle-cli/src/aggregate.rs` (two-sided Student-t interval,
   `T_CRITICAL_975`, `mode_pair_slices`, `movement_slices`); tests
   `tests/aggregate.rs` (hand-computed mean/variance/interval and the real
   `mixed_interaction_v1` slices).
6. **CRN seed banks support A/B comparisons.** `seed_bank.rs`
   (`SEED_BANK_VERSION = 1`), `batch --seed-bank` records the bank reference in
   `batch.json`, and `compare.rs` proves one bank and one order before pairing.
   Tests `tests/seed_bank.rs`, `tests/compare.rs`.
7. **Fast/Standard/Fine convergence runner and machine-readable sensitivity
   report.** `apps/tangle-cli/src/converge.rs` (`PRESETS` order, tolerance
   0.05 on the standard→fine relative change, `verdict`); tests
   `tests/converge.rs` including a real three-fidelity run and a
   byte-identical re-run.
8. **Every reported metric links to its manifest(s) and a versioned metric
   definition.** `run_metrics.rs` fixes `METRIC_DEFINITION_VERSION = 1` and
   writes it plus `manifest_sha256` into `metrics.json`; `aggregate`, `compare`,
   and `converge` carry and verify it ([[DEF-004-metric-definition-v1]]). Tests
   `tests/run_metrics.rs`, `tests/aggregate.rs`, `tests/compare.rs`,
   `tests/converge.rs`.
9. **Parallel and serial batch produce identical per-run trace hashes.**
   `tests/batch.rs::parallel_and_serial_batches_produce_identical_trace_hashes_and_streams`
   compares `batch.json`, every `trace_sha256`, and every decompressed event
   stream between `--jobs 1` and `--jobs 4` over the same seeds.
10. **Release-mode benchmarks and profiler captures before optimization,
    including the metrics cost and the swept-TOI state.** `perf/release-bench.json`,
    `perf/tick-phases.json` (ablation with identical event-stream hash),
    `perf/profiles/*`, and `scripts/{bench-release,measure-tick-phases,capture-profile,profile-symbols}.*`;
    no test asserts a timing number. No optimization landed: the only `crates/`
    change is the additive TTC accessor. The swept cast is recorded as on the
    tick path exactly once (the safety monitor's contact confirmation) with no
    behavioral consumer.
11. **Five gates on the final tree.** Rerun by the coordinator; all pass (see
    above).

# Limitations

Accepted scope boundaries and recorded residuals, not failures of the criteria:

- **The interaction-metrics pass is always on and unoptimized.** Release
  ablation on `mixed_interaction_v1` measures it at about 18–33 microseconds
  per tick depending on the window (54–75% of the tick), confirming Increment
  4's ~22/7 microsecond ratio in shape. Increment 5 records this as its
  baseline and deliberately does not optimize it.
- **The swept TOI cast has no behavioral consumer.** It is on the tick path
  once, as the safety monitor's contact confirmation; control, admission,
  signalling, and yielding do not depend on a sweep.
- **Trajectories are sampled by default.** The default policy is stride 10 with
  a 100 000-sample cap; full trajectories are opt-in. `max_samples` counts rows
  (one agent at one tick), and the movement-pair buckets are keyed by the two
  movement keys.
- **Aggregation disaggregation is narrower than the run artifact's.** The
  batch-level mode slice carries only `minimum_separation_m`, and the run
  artifact's mode/movement event-family counts are not aggregated (slice-F
  finding F6, accepted).
- **Performance numbers are an order of magnitude, not a contract.** They come
  from one host, one toolchain, and one seed; the profiler capture is a macOS
  `sample` capture with no flamegraph, debug info, or frame pointers, so no
  inlined frames or line numbers.
- **Convergence uses one global 5% relative tolerance.** Count metrics commonly
  flag as materially sensitive (0 → 1 is unbounded); a per-metric tolerance is
  Increment 6 work.
- **Replay resolves the manifest's scenario path verbatim**, with no search, and
  `--verify` compares decompressed stream content rather than the gzip
  container bytes.
- **Inherited Increment 4 boundaries:** region occupancy uses a swept bounding
  circle; PET is defined only for non-overlapping successions; contacting-pair
  minimum separation is the contact entry, not the deepest penetration.
- **The residual backlog is unaffected.** `TAS-029-bound-red-queue-emergency-cap`
  (P2) and the `FBK-001`..`FBK-018` feedback backlog remain `proposed`; the new
  [[TAS-044-braintree-feedback-triage]] owns the backlog's clear next, and none
  of them gates this increment.
- **Tooling layout.** The vault now lives at `.braintree/` (Braintree 0.6.0);
  `AGENTS.md` was updated accordingly. This is not a product change.
