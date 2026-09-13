---
context_rev: 1
priority: P1
updated: 2026-09-13T00:37:03Z
summary: Slice A2 of Phase 1 Increment 5 adds a `batch` CLI command that runs a seed list into one immutable run directory per run under an output root, can be stopped and resumed without mutating completed artifacts, and produces identical per-run trace hashes under parallel and serial execution.
next: Read the B1/B2 run-directory API and implement `batch` with a batch manifest, resumable completed-run skipping, and a `--jobs` outer loop, then test serial-versus-parallel trace-hash equality.
---

# Outcome

Add a `batch` command to the Tangle CLI. `batch` runs a list of seeds for one
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

Constraint: no dependency change in `tangle-model` or `tangle-sim`; any
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
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice B1/B2 fixed the immutable run directory (`manifest.json`, `summary.json`,
compressed event stream, Parquet trajectories) and the public
`tangle_cli::{write_run_directory, RunManifest, RunSummary, SamplingPolicy}`
surface. This child composes that writer into a resumable, optionally parallel
batch. `replay` is a sibling direct child; aggregation and the convergence
runner consume the batch output this child fixes.

This slice carries two of Increment 5's four gate bullets: batch stop/resume,
and parallel/serial identical trace hashes.

# Result

## Claim and write set

Base hash `b4ebee451077330dbc107725bed3b91e3b452068d73002cce2e004383c7df897`
from `braintree hash TAS-035`, claimed by `worker` for 14400 s. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`,
`apps/tangle-cli/Cargo.toml`, `Cargo.lock` (root), and this node's own frontier
transition (moved `nodes/proposed/` to `nodes/active/`).

Pending implementation.
