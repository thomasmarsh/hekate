---
context_rev: 1
priority: P1
updated: 2026-09-13T00:15:07Z
summary: Slice B1 of Phase 1 Increment 5 generalizes `run` output into an immutable per-run directory holding a reproducible JSON manifest, a JSON summary, a compressed JSON Lines sparse typed event stream, and a declared sampling policy that bounds output size.
next: Hand this slice to the TAS-031 coordinator for integration, slice F verification, and closeout.
---

# Outcome

Generalize the `run` command's output into an immutable per-run directory. A
completed run directory holds:

- `manifest.json` — JSON, self-sufficient to reproduce the run: scenario source
  path and content hash, seed, step, fidelity profile, `event_version`,
  `schema_version`, the simulator/build revision, and the declared sampling
  policy;
- `summary.json` — JSON, the run's aggregate output (counts and the metrics the
  current run already reports), with the metric values it carries traceable to
  the manifest;
- the sparse typed event stream — compressed JSON Lines, one typed record per
  line, in the canonical record order.

The directory is immutable once complete: a run never rewrites a completed run
directory, and a second invocation targeting an existing completed run
directory fails or no-ops without mutating it. The manifest declares the
sampling policy that bounds the output; full trajectories remain opt-in and are
not written by this slice.

The existing canonical trace artifact, its byte format, its golden, the hash
golden, and the Phase 1 baseline must remain unchanged; the run directory is the
enclosing container, and `run`'s existing default behavior stays byte-identical
apart from the added directory.

Constraint: no new dependency in `tangle-model` or `tangle-sim`. A compression
dependency belongs in the CLI/output layer (`apps/tangle-cli`) and must be
reported. No scenario-schema, record-shape, or `EVENT_VERSION` change.

# Done when

- `run` writes an immutable run directory containing `manifest.json` (JSON),
  `summary.json` (JSON), and a compressed JSON Lines sparse typed event stream.
- The manifest is self-sufficient to reproduce the run and records the declared
  sampling policy.
- A completed run directory is never rewritten; a repeat run against it does not
  mutate completed artifacts, covered by a test.
- The existing canonical trace format, trace golden, trace hash golden, and
  Phase 1 baseline are unchanged.
- Tests cover the manifest fields, the compressed event stream round-trip, the
  summary, and the immutability property.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice B of Increment 5 asks for an immutable run directory with manifest,
summary, event stream, and selectively sampled trajectories; JSON for
manifests/summaries, compressed JSON Lines for sparse typed events, and Parquet
for sampled trajectories; and a declared sampling policy bounding output size.

This child owns the manifest/summary/event-stream core and the sampling-policy
declaration. The Parquet sampled-trajectory half is a separate direct child, so
the Parquet dependency does not enlarge this slice. `batch`, `replay`,
aggregation, and the convergence runner consume the layout this child fixes.

# Result

Slice B1 landed the immutable run directory as an additive output of the
existing `run` command. No scenario-schema change, no record-shape change,
`EVENT_VERSION` stays 2, and no new dependency reaches `tangle-model` or
`tangle-sim`.

## What landed

- `apps/tangle-cli/src/run_dir.rs` (new) owns the directory layout.
  `write_run_directory` writes `events.jsonl.gz`, then `summary.json`, then
  `manifest.json` last as the completion marker, so a directory reads as
  complete only when every artifact the manifest names is present. It refuses a
  target that holds a completed run (`RunDirectoryError::Completed`) or any
  other content (`RunDirectoryError::NotEmpty`) and mutates neither.
  `RunManifest` records the scenario id, source path, schema version, and
  content hash; the seed, ticks, fidelity profile and `step_s`, `event_version`,
  `model_version`, and `build_revision`; the declared `SamplingPolicy`; and the
  `EventStream` descriptor (path, gzip, record count, uncompressed size and
  SHA-256). `RunSummary` records the run counts, `elapsed_s`, and the
  manifest's SHA-256 as the traceability link. Nothing reads the clock or the
  environment, so the same inputs reproduce byte-identical artifacts.
- The declared sampling policy is `events: all` (sparse typed events are never
  sampled; the stream is the canonical record set) and `trajectories:
  { retention: off, stride_ticks: 10, max_samples: 100000 }`. Trajectories stay
  opt-in and are not written here, so the directory cannot grow with agents x
  ticks; the stride and cap are the declared bound the Parquet child enforces
  when it enables them.
- `apps/tangle-cli/src/trace.rs` adds `canonical_run`, returning the canonical
  trace and the kernel's `RunSummary` from one run loop;
  `canonical_trace` now delegates to it, so the golden bytes cannot drift from
  a second implementation of the loop.
- `apps/tangle-cli/src/main.rs` adds `--run-dir <PATH>` to `run`. The run
  directory is written before the trace output, so a rejected rerun emits
  nothing to `--output`. All existing behavior is unchanged: the same
  `--output`/`--hash-file` artifacts and the same `trace hash:` line on stderr,
  byte-identical apart from the added directory.
- `apps/tangle-cli/tests/run_directory.rs` (new) is the contract suite.

## Evidence

`cargo test -p tangle-cli --test run_directory` (10 passed) and
`cargo test -p tangle-cli --lib` (19 passed, including the 5 new unit tests in
`run_dir.rs`):

- `manifest_records_the_provenance_a_rerun_needs` — every manifest field, with
  `content_sha256` equal to the loader's own hash of the authored bytes.
- `event_stream_round_trips_to_the_canonical_records` — the decompressed stream
  equals `canonical_run`'s bytes and `tests/golden/walking_guide_v1.trace.jsonl`,
  and its SHA-256 equals the checked-in hash golden and the manifest's
  `stream.uncompressed_sha256`.
- `summary_reports_the_run_and_traces_back_to_the_manifest` — counts against the
  kernel summary, `elapsed_s` 12.5, and `manifest_sha256` equal to the SHA-256 of
  the `manifest.json` file bytes.
- `a_completed_run_directory_is_never_rewritten`,
  `a_repeat_run_against_a_completed_directory_fails_without_mutating_it` — a
  second run fails (`exit 1`, `already holds a completed run`) with every
  artifact content hash unchanged and no trace written to its own destination.
- `a_run_directory_holding_foreign_content_is_not_clobbered`,
  `run_writes_the_three_artifacts_and_no_trajectory_stream` — the directory
  holds exactly `events.jsonl.gz`, `manifest.json`, and `summary.json`.
- `run_directories_are_reproducible_byte_for_byte` — two fresh runs produce
  identical bytes for all three artifacts.
- `run_writes_the_run_directory_and_keeps_the_trace_output`,
  `run_without_a_run_directory_writes_only_the_trace` — the CLI wiring, the
  unchanged trace/hash outputs, and the default run that adds nothing.

Gates on this tree: `cargo test --workspace --all-features` passed,
`cargo clippy --workspace --all-targets --all-features -- -D warnings` passed
clean, `cargo fmt --all --check` passed after `cargo fmt --all`,
`./scripts/check-dependency-direction.sh` printed `dependency direction OK`, and
`braintree check nodes` passed.

## Claim and write set

Base hash `71904cc2084cf9c354952d2f4c58d6af8e68352ce30c561f28b667b4cc2f8d93`
from `braintree hash TAS-033`, claimed by `worker` for 14400 s. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`,
`apps/tangle-cli/Cargo.toml`, `Cargo.lock` (for the CLI dependency addition
only), and this node's own frontier transition.

## Dependencies reported

One dependency added: `flate2 = "1.1.10"` in `apps/tangle-cli/Cargo.toml`, the
output layer only, with default features (pure-Rust `miniz_oxide`; no C zlib).
`Cargo.lock` gains one line (`tangle-cli` depends on `flate2`, already present
transitively). Neither `tangle-model` nor `tangle-sim` depends on it, and
`scripts/check-dependency-direction.sh` passes.

## Deferrals

- Full sampled trajectories (Parquet) remain the separate direct child; this
  node introduces no trajectory writer.
- `build_revision` records the producing crate's version
  (`tangle-cli` `CARGO_PKG_VERSION`), because the workspace stamps no source
  revision into a build and a `build.rs` lies outside this write set. A source
  revision is a follow-up, not a gap in the directory format.
- The declared trajectory stride and cap are declared but not enforced; the
  Parquet child enforces them when it enables trajectory output.
- `run` gains no `--step` option, so the fidelity profile is currently always
  `standard`; the manifest still records the numeric step so a non-preset step
  would stay reproducible.
