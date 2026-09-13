---
context_rev: 1
priority: P1
updated: 2026-09-13T00:18:20Z
summary: Slice B1 of Phase 1 Increment 5 generalizes `run` output into an immutable per-run directory holding a reproducible JSON manifest, a JSON summary, a compressed JSON Lines sparse typed event stream, and a declared sampling policy that bounds output size.
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

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`7de42a7`), verified by rerunning the gates on that tree:

1. **`run` writes an immutable run directory holding `manifest.json` (JSON),
   `summary.json` (JSON), and a compressed JSON Lines sparse typed event
   stream.** `apps/tangle-cli/src/run_dir.rs:261` (`write_run_directory`)
   writes `events.jsonl.gz`, `summary.json`, then `manifest.json` as the
   completion marker, naming them through `:54` `MANIFEST_FILE`, `:57`
   `SUMMARY_FILE`, and `:60` `EVENT_STREAM_FILE`; `:355` (`gzip`) compresses
   with a fixed header, and `:175` `EventStream` records the stream's path,
   compression, record count, and uncompressed size and hash.
   `apps/tangle-cli/src/main.rs:113` adds `--run-dir` and `:146` writes the
   directory before the trace output. The JSON forms are asserted by
   `tests/run_directory.rs:204` (`manifest_records_the_provenance_a_rerun_needs`)
   and `:302` (`summary_reports_the_run_and_traces_back_to_the_manifest`), both of
   which deserialize the on-disk file, and the artifact set by `:182`
   (`run_writes_the_three_artifacts_and_no_trajectory_stream`).
2. **The manifest is self-sufficient to reproduce the run and records the
   declared sampling policy.** `run_dir.rs:190` `RunManifest` carries the
   scenario id, source path, schema version and content hash (reusing
   `ScenarioProvenance`), seed, ticks, fidelity profile and `step_s`,
   `event_version`, `model_version`, `build_revision`, `sampling`, and the
   stream descriptor; `apps/tangle-cli/src/main.rs:125` loads the source through
   `load_scenario_hashed` so the recorded hash is the authored bytes' own hash.
   The policy is `:149` `SamplingPolicy`, defaulted at `:159` to
   `events: all` with `trajectories: { retention: off, stride_ticks: 10,
   max_samples: 100000 }` (`:70`, `:76`). `tests/run_directory.rs:204` asserts
   every field and the policy values against the loader's own content hash and
   `tangle_model::MODEL_VERSION`; `run_dir.rs:419`
   (`the_declared_policy_retains_every_event_and_no_trajectory_state`)
   asserts the declared bounds.
3. **A completed run directory is never rewritten; a repeat run against it does
   not mutate completed artifacts, covered by a test.** `run_dir.rs:325`
   (`ensure_writable`) rejects a `manifest.json`-bearing directory with
   `RunDirectoryError::Completed` and foreign content with `NotEmpty` (`:83`),
   and never truncates or rewrites. `tests/run_directory.rs:331`
   (`a_completed_run_directory_is_never_rewritten`) compares every file's
   content hash before and after the rejected second write; `:453`
   (`a_repeat_run_against_a_completed_directory_fails_without_mutating_it`) does
   the same through the CLI and additionally shows the failed rerun wrote no
   trace to its own destination; `:359`
   (`a_run_directory_holding_foreign_content_is_not_clobbered`) covers the
   non-completed case. `:382` (`run_directories_are_reproducible_byte_for_byte`)
   shows no clock or environment input leaks into the artifacts.
4. **The existing canonical trace format, trace golden, trace hash golden, and
   Phase 1 baseline are unchanged.** No file under `tests/golden/`,
   `baselines/`, `scenarios/`, or the kernel crates changed, and `EVENT_VERSION`
   stays 2 (`crates/tangle-sim/src/event.rs:66`); the only `trace.rs` change is
   the new `canonical_run` (`apps/tangle-cli/src/trace.rs:131`), which
   `canonical_trace` (`:116`) now delegates to, so the run loop has one
   implementation. The golden guards still pass (`tests/golden_trace.rs`,
   `tests/baseline.rs`), and `tests/run_directory.rs:256`
   (`event_stream_round_trips_to_the_canonical_records`) compares the
   decompressed stream byte for byte with the checked-in JSONL golden and its
   SHA-256 with the checked-in hash golden.
5. **Tests cover the manifest fields, the compressed event stream round-trip,
   the summary, and the immutability property.** `tests/run_directory.rs:204`
   (manifest fields), `:256` (decompressed round-trip against the canonical
   record order and the goldens), `:302` (summary counts, `elapsed_s`, and
   `manifest_sha256` against the manifest file bytes), `:331`/`:453`/`:359`
   (immutability), plus the CLI wiring at `:401` and `:520` and the unit tests in
   `run_dir.rs:403`, `:412`, `:419`, `:437`, and `:445`.
6. **The five gates pass on the final tree** (rerun on the committed tree,
   2026-09-13: `cargo test --workspace --all-features` passed with 0 failures;
   `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
   `cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh`
   printed `dependency direction OK`; `braintree check nodes` printed
   `graph check: passed`).

Outcome complete: the run directory, its declared sampling policy, its
immutability guarantee, and the gate evidence are on the committed tree. The
Parquet sampled-trajectory half, a source-revision stamp, and enforcement of the
declared trajectory stride and cap stay with the direct children and later
slices of the parent increment, not with this node.
