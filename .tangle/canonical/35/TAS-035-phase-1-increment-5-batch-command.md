---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice A2 of Phase 1 Increment 5 adds a `batch` CLI command that runs a seed list into one immutable run directory per run under an output root, can be stopped and resumed without mutating completed artifacts, and produces identical per-run trace hashes under parallel and serial execution.
---

# Outcome

Add a `batch` command to the Hekate CLI. `batch` runs a list of seeds for one
scenario, each seed producing its own immutable run directory under an output
root (reusing the B1/B2 `run` directory writer, manifest, event stream, and
trajectory sampling). A batch-level manifest records the specification, the
seed list, and each run's directory and trace hash.

Two gates drive this slice:

- **Stop and resume.** Re-invoking the same batch skips runs whose directory is
  already complete and does not mutate any completed artifact. An interrupted
  run whose directory lacks the completion marker is re-run and completed from
  scratch. Resume is detected from the artifacts on disk, not from in-memory
  state.
- **Parallel equals serial.** `--jobs N` is an outer loop over independent
  single-threaded runs. A parallel batch and a serial batch produce identical
  per-run trace hashes and identical canonical event streams. The simulation
  tick is never parallelized, and each run's `Simulation` state is private to
  its worker.

Ordering stays deterministic: the batch manifest lists runs in ascending seed
order regardless of completion order, and per-run output is independent of
`--jobs` and of the order runs finish.

Constraint: no dependency change in `hekate-model` or `hekate-sim`; any
concurrency helper is a CLI-layer dependency and must be reported. No
scenario-schema, record-shape, or `EVENT_VERSION` change, and the canonical
trace, goldens, and Phase 1 baseline stay byte-identical.

# Done when

- `batch` runs a seed list into one immutable run directory per run and writes a
  batch-level manifest recording the spec, seed list, and each run's directory
  and trace hash.
- A stopped batch resumes: a repeat invocation skips completed runs and does not
  mutate their artifacts, covered by a content-hash-before/after test; an
  incomplete run directory is re-run and completed.
- Parallel (`--jobs N`) and serial (`--jobs 1`) batch execution produce
  identical per-run trace hashes, covered by a test.
- The batch manifest is ordered by ascending seed independent of completion
  order.
- The canonical trace, its hash golden, the Phase 1 baseline, and all goldens
  are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `tangle check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice B1/B2 fixed the immutable run directory (`manifest.json`, `summary.json`,
compressed event stream, Parquet trajectories) and the public
`hekate_cli::{write_run_directory, RunManifest, RunSummary, SamplingPolicy}`
surface. This child composes that writer into a resumable, optionally parallel
batch. `replay` is a sibling direct child; aggregation and the convergence
runner consume the batch output this child fixes.

This slice carries two of Increment 5's four gate bullets: batch stop/resume,
and parallel/serial identical trace hashes.

# Result

Slice A2 landed the resumable, optionally parallel `batch` command on top of
the B1/B2 run-directory writer. No scenario-schema change, no record-shape
change, `EVENT_VERSION` stays 2, no new dependency, and no golden, baseline, or
scenario file changed.

## What landed

- `apps/hekate-cli/src/batch.rs` (new) owns the batch. `run_batch` (`:204`)
  normalizes the seed list (`normalize_seeds`, `:258`, sorted and deduplicated),
  inspects every seed's directory before spending any work (`inspect`, `:274`),
  runs only the pending seeds (`execute`, `:353`), and writes `batch.json` last
  as the batch's own completion marker. `BatchManifest` (`:163`) records
  `BatchSpec` (`:123`) and the ascending `seeds`, plus one `BatchRun` (`:144`)
  per seed holding `{seed, directory, trace_sha256, manifest_sha256}` in
  ascending seed order.
- Resume is detected from the artifacts on disk. `inspect` (`:274`) treats a
  directory whose `manifest.json` completion marker is present as complete,
  checks it against the specification (`verify_matches`, `:318`: scenario
  content hash, seed, ticks, step, sampling), and reads its record back from the
  files (`read_run`, `:455`) so the recorded hashes describe what is on disk. A
  directory without the marker that holds only run artifacts
  (`holds_only_run_artifacts`, `:295`) is cleared (`clear_partial_run`, `:438`)
  and re-run from scratch; foreign content is refused rather than clobbered.
- Parallel equals serial. `--jobs N` is an outer loop over whole runs: `execute`
  (`:353`) starts N workers that each pop one `PendingRun` from a shared queue
  (`drain_queue`, `:387`) and call `execute_run` (`:409`), which builds a private
  `Simulation` through `canonical_run_sampled` and writes one run directory with
  the B1/B2 writer. No tick is parallelized, and results are re-sorted by seed
  position, so the manifest is independent of worker count and completion order.
- `apps/hekate-cli/src/main.rs` adds `batch` (`:291`) with `BatchArgs` (`:266`):
  `--seeds` (comma-separated, required), `--out-root`, `--ticks`, `--jobs` (>=1
  through a ranged value parser, `:283`), and `--full-trajectories`.
  `sampling_policy` (`:256`) is now shared with `run`. The command prints one
  `batch manifest:` line on stderr; `run` and `validate` defaults are unchanged.
- `apps/hekate-cli/src/run_dir.rs:482` makes the existing `fidelity` helper
  `pub(crate)` so the batch spec labels a fixed step exactly as a run manifest
  does, rather than duplicating the preset table.
- `apps/hekate-cli/tests/batch.rs` (new) is the contract suite.

## Evidence

`cargo test --workspace --all-features` (479 passed, 0 failed) on this tree,
including the new `apps/hekate-cli/tests/batch.rs` (6 tests) and the 3 new unit
tests in `batch.rs`:

- `batch_writes_a_run_directory_per_seed_and_an_ascending_manifest`
  (`tests/batch.rs:190`) runs `batch --seeds 2,0,1` and shows one four-artifact
  run directory per seed and a manifest whose `seeds` and `runs` are `[0,1,2]`
  despite the given order, with every specification field asserted.
- `resume_skips_completed_runs_and_completes_an_incomplete_one`
  (`tests/batch.rs:266`) records each run directory's content hashes, deletes
  one run's `manifest.json`, and re-runs the batch: the completed runs' bytes are
  unchanged, `batch.json` is byte-identical, the returned manifest equals the
  first, and the interrupted run is completed with its original bytes.
- `parallel_and_serial_batches_produce_identical_trace_hashes_and_streams`
  (`tests/batch.rs:346`) runs `--jobs 1` and `--jobs 4` batches and compares the
  whole `batch.json`, every per-run `trace_sha256`, and every decompressed
  `events.jsonl.gz`.
- `the_batch_manifest_links_each_run_to_its_run_manifest` (`tests/batch.rs:229`)
  checks each run's `trace_sha256` against its manifest's
  `stream.uncompressed_sha256` and its `manifest_sha256` against the manifest
  file bytes and the summary's `manifest_sha256`.
- `a_completed_run_that_disagrees_with_the_batch_is_refused`
  (`tests/batch.rs:312`) shows a different tick count is refused (`Mismatch`)
  without mutating the completed run or `batch.json`.
- `zero_jobs_is_a_usage_error` (`tests/batch.rs:396`) pins `--jobs 0` as a usage
  error (exit 2) that writes nothing.

## Claim and write set

Base hash `b4ebee451077330dbc107725bed3b91e3b452068d73002cce2e004383c7df897`
from `tangle hash TAS-035`, claimed by `worker` for 14400 s. Write set:
`apps/hekate-cli/src/**`, `apps/hekate-cli/tests/**`,
`apps/hekate-cli/Cargo.toml`, `Cargo.lock` (root), and this node's own frontier
transition (moved `nodes/proposed/` to `nodes/active/`, then to
`nodes/resolved/`). Every changed, created, and moved path stayed inside it.

## Dependencies reported

None. The `--jobs` outer loop uses `std::thread::scope` and `std::sync::Mutex`
from the standard library, so `apps/hekate-cli/Cargo.toml` and `Cargo.lock` are
unchanged and no concurrency helper was added.

## Preservation

No file under `tests/golden/`, `baselines/`, `scenarios/`, `schemas/`, or the
kernel crates changed; `EVENT_VERSION` stays 2 (`crates/hekate-sim/src/event.rs`).
The golden, hash-golden, and Phase 1 baseline tests still pass, and the batch
only composes the existing `canonical_run_sampled` and `write_run_directory`, so
it cannot change canonical bytes.

## Deferrals and interpretation

- **Batch specification format.** `batch.json` holds one `spec` (the run
  manifest's provenance and policy without the per-run stream descriptor), the
  deduplicated ascending `seeds`, and one `runs` entry per seed. It deliberately
  records no `--jobs` value and no timing, so serial and parallel runs write
  identical bytes. A consumer reads per-run detail from each run's own
  `manifest.json`, which `runs[].trace_sha256`/`manifest_sha256` link to.
- **Resume is per run directory, not per batch.** A resumed batch that finds
  every run complete rewrites `batch.json` byte for byte; it does not verify the
  `batch.json` that was already present, so an output root is assumed batch-owned
  and its `seed-<n>` directories are the batch's own.
- **A run directory that disagrees with the batch is refused,** not silently
  reused, so resuming with a different tick count, scenario, step, or policy
  fails before any run starts rather than recording mismatched provenance.
- **`batch` writes no standalone `trace.jsonl`/`--hash-file`**; each run's trace
  hash lives in `batch.json` and its run manifest, matching the run directory as
  the durable artifact.
- Aggregation and the convergence runner (siblings under [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]])
  consume the batch layout this node fixes.

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`2287b68`), verified by rerunning the five gates on that tree:

1. **`batch` runs a seed list into one immutable run directory per run and
   writes a batch-level manifest recording the spec, seed list, and each run's
   directory and trace hash.** `batch.rs:204` (`run_batch`) inspects, runs, and
   then writes `batch.json` (`write_batch_manifest`, `:486`) holding `BatchSpec`
   (`:123`), `seeds`, and `BatchRun` (`:144`, seed, directory, `trace_sha256`,
   `manifest_sha256`); each run writes through `execute_run` (`:409`) into the
   B1/B2 `write_run_directory`. `main.rs:291` wires the command. Asserted by
   `tests/batch.rs:190` (manifest fields and a four-artifact directory per seed)
   and `tests/batch.rs:229` (the run links).
2. **A stopped batch resumes: a repeat invocation skips completed runs and does
   not mutate their artifacts, covered by a content-hash-before/after test; an
   incomplete run directory is re-run and completed.** `inspect` (`batch.rs:274`)
   reads completion from `manifest.json` on disk, `verify_matches` (`:318`)
   checks the specification, `holds_only_run_artifacts` (`:295`) accepts a
   partial run, and `clear_partial_run` (`:438`) clears it before `execute_run`
   re-runs it. `tests/batch.rs:266` compares every run directory's content hashes
   before and after a resume that deleted one `manifest.json`, asserts the
   restored bytes, and asserts `batch.json` is unchanged; `tests/batch.rs:312`
   additionally shows a mismatched completed run is refused without mutation.
3. **Parallel (`--jobs N`) and serial (`--jobs 1`) batch execution produce
   identical per-run trace hashes, covered by a test.** `execute`
   (`batch.rs:353`) clamps the worker count, `drain_queue` (`:387`) hands each
   worker one whole run, and `execute_run` (`:409`) gives every run a private
   `Simulation` through `canonical_run_sampled`, so no tick is shared;
   `main.rs:283` enforces `--jobs >= 1`. `tests/batch.rs:346` compares the whole
   `batch.json`, every trace hash, and every decompressed event stream between
   `--jobs 1` and `--jobs 4`.
4. **The batch manifest is ordered by ascending seed independent of completion
   order.** `normalize_seeds` (`batch.rs:258`) sorts and deduplicates, and
   `execute` (`:353`) returns records re-sorted by seed position so `run_batch`
   fills `runs` in ascending order. `tests/batch.rs:190` gives the seeds as
   `2,0,1` and asserts `[0,1,2]`; `tests/batch.rs:346` asserts the two manifests
   and their run order match across `--jobs`.
5. **The canonical trace, its hash golden, the Phase 1 baseline, and all goldens
   are unchanged; `EVENT_VERSION` stays 2.** The implementation commit touches
   only `apps/hekate-cli/src/{batch.rs,lib.rs,main.rs,run_dir.rs}` and
   `apps/hekate-cli/tests/batch.rs` plus the node move; no file under
   `tests/golden/`, `baselines/`, `scenarios/`, `schemas/`, or the kernel crates
   changed, and `EVENT_VERSION` stays 2
   (`crates/hekate-sim/src/event.rs:66`). The golden and baseline guards
   (`tests/golden_trace.rs`, `tests/baseline.rs`, `tests/run_directory.rs`,
   `tests/trajectories.rs`) still pass, and the batch reuses
   `canonical_run_sampled` (`apps/hekate-cli/src/trace.rs:150`) unchanged.
6. **The five gates pass on the final tree** (rerun on `2287b68`, 2026-09-13:
   `cargo test --workspace --all-features` passed with 479 tests and 0 failures;
   `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
   `cargo fmt --all --check` clean after `cargo fmt --all`;
   `./scripts/check-dependency-direction.sh` printed `dependency direction OK`;
   `tangle check nodes` printed `graph check: passed (61 nodes)`).

Outcome complete: the batch command, its resumable disk-derived skip/re-run
rule, its ascending manifest, its parallel-equals-serial guarantee, and the gate
evidence are on the committed tree.
