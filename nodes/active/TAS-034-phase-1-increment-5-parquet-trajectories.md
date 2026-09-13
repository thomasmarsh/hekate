---
context_rev: 1
priority: P1
updated: 2026-09-13T00:32:25Z
summary: Slice B2 of Phase 1 Increment 5 writes selectively sampled agent trajectories into the immutable run directory as Parquet, driven by the SamplingPolicy fixed in B1, so output size is bounded by the policy and full trajectories are opt-in.
next: Resolve the node with per-criterion evidence once the five gates pass on the committed tree.
---

# Outcome

Extend the immutable run directory with the sampled-trajectory artifact the
plan asks for: Parquet. The run reads the `SamplingPolicy` the manifest already
records (`stride_ticks` and `max_samples`), samples agent frames at that stride,
and writes a Parquet file holding the sampled trajectories.

- A declared sampling policy bounds output size: the default policy samples a
  bounded subset, and the written trajectory count never exceeds the declared
  bound.
- Full trajectories are opt-in: a policy that requests every tick has no
  truncation, and the manifest records the policy actually applied so a
  consumer can tell a sampled artifact from a full one.
- The sampled rows round-trip: reading the Parquet file back reproduces the
  sampled frames (agent id, tick, position, heading, speed, mode) in the
  canonical order, and the manifest links the trajectory artifact to the run.
- A completed run directory stays immutable.
- The Parquet dependency lives in the CLI/output layer (`apps/tangle-cli`)
  only; `tangle-model` and `tangle-sim` gain nothing. No scenario-schema,
  record-shape, or `EVENT_VERSION` change, and the existing canonical trace,
  goldens, and Phase 1 baseline stay byte-identical.

# Done when

- The run directory contains a Parquet sampled-trajectory artifact produced
  under the declared sampling policy.
- Tests cover: the Parquet round-trip, the sample-count bound for a default
  policy, the full-trajectory opt-in path, and the manifest/artifact link.
- Output size is bounded by the declared policy: a bounded policy writes no
  more than its declared maximum.
- A completed run directory is not mutated by a repeat run.
- The existing canonical trace format, trace golden, trace hash golden, and
  Phase 1 baseline are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice B1 landed the immutable run directory core (manifest, summary, compressed
event stream) and the declared `SamplingPolicy`, but left the trajectory
sampling as a declaration only. This child enforces that policy and writes the
Parquet artifact. It is the second half of slice B; the `batch`/`replay`,
aggregation, and convergence children consume the resulting layout.

The new Parquet dependency is output-layer only, per the increment constraint.

# Result

Slice B2 landed the sampled-trajectory artifact as an additive part of the run
directory B1 fixed. No scenario-schema change, no record-shape change,
`EVENT_VERSION` stays 2, and no new dependency reaches `tangle-model` or
`tangle-sim`.

## What landed

- `apps/tangle-cli/src/trajectories.rs` (new) owns the artifact.
  `TrajectorySample` (`:61`) is one agent observed at one tick: tick, agent,
  mode, `x_m`, `y_m`, `heading_rad`, `speed_mps`. `TrajectoryRecorder` (`:146`)
  samples the step the simulation last completed (`:165`), so a row and an event
  record with the same tick describe the same instant, and it appends live
  agents in stable spawn order, so rows come out in canonical order — ascending
  tick, then ascending agent. `write_trajectories` (`:207`) encodes one Arrow
  record batch with a fixed schema (`:273`) and fixed Snappy properties
  (`:324`), and returns the manifest descriptor (`:85`: file name, format, row
  count, SHA-256 of the exact bytes). `read_trajectories` (`:246`) reads the
  rows back, checking the schema (`TrajectoryError::Schema`) rather than trusting
  column positions.
- The declared policy is now enforced, not just declared. `TrajectoryRetention`
  gained `Sampled` (the default) and `Full` (`run_dir.rs:135`), and
  `TrajectorySampling` gained `off`/`full` (`:175`, `:187`), `writes_artifact`
  (`:196`), `samples_tick` (`:201`, a tick is sampled when it is a multiple of
  `stride_ticks`), and `below_cap` (`:214`, the `max_samples` row bound).
  `SamplingPolicy::full_trajectories` (`:254`) is the opt-in full policy:
  stride one step and `max_samples: u64::MAX`, so the declared policy is the
  policy applied and a full artifact is never truncated. A bounded policy stops
  collecting at the cap, so the in-memory set and the file are both bounded
  before any bytes are written.
- `run_dir.rs` writes the artifact before `manifest.json` and records it in the
  manifest: `RunManifest.trajectories: Option<TrajectoryArtifact>` (`:309`),
  `RunDirectoryRequest.sampling` (`:342`) and `.trajectories`, and
  `write_run_directory` (`:360`) writes `trajectories.parquet` when the applied
  policy retains trajectories, so the manifest's hash describes final bytes.
  The manifest records the policy the run applied, which is how a consumer
  tells a bounded sample from a full artifact. Immutability is unchanged: the
  completion marker is still `manifest.json`, written last.
- `trace.rs` adds `canonical_run_sampled` (`:150`) and `canonical_run` (`:134`)
  delegates to it with the off policy, so the trace and the sampled rows come
  from one run loop and sampling cannot change the canonical bytes.
- `main.rs` adds `--full-trajectories` (`:125`), which requires `--run-dir` and
  selects the full policy (`sampling_policy`, `:213`); without it the default
  bounded policy applies, and without `--run-dir` the run takes the plain path
  and captures nothing.
- `lib.rs` re-exports the artifact surface: `TRAJECTORY_FILE`,
  `TRAJECTORY_FORMAT`, `TrajectorySample`, `TrajectoryRecorder`,
  `TrajectoryArtifact`, `TrajectoryError`, `write_trajectories`,
  `read_trajectories`, and `canonical_run_sampled`.

## Evidence

`cargo test --workspace --all-features` (470 passed, 0 failed) on this tree,
including the new `apps/tangle-cli/tests/trajectories.rs` (9 tests) and the 4
new or updated unit tests in `trajectories.rs` and `run_dir.rs`:

- `sampled_trajectories_round_trip_in_canonical_order` (`tests/trajectories.rs:190`)
  writes the default-policy run directory, reads the rows back through
  `read_trajectories`, and compares their `(tick, agent)` identity with a
  reference frame set replayed straight through the kernel and filtered by the
  declared stride: rows are exactly the sampled frames, ascending by tick then
  agent, with the field values the kernel reports (mode, motion, position at
  the tick and speed the run implies).
- `the_default_policy_bounds_the_written_sample_count` (`:239`) shows the
  default artifact holds fewer rows than the run's agent frames and never more
  than `DEFAULT_MAX_TRAJECTORY_SAMPLES`.
- `a_bounded_cap_writes_exactly_the_declared_maximum` (`:260`) uses a
  `stride_ticks: 1, max_samples: 5` policy: the artifact holds exactly 5 rows,
  the first rows in canonical order, and nothing past the cap.
- `full_trajectories_are_opt_in_and_never_truncated` (`:289`) runs
  `run --run-dir … --full-trajectories` and shows the artifact holds every
  agent frame of the run, more rows than the default sample, with the manifest
  recording `retention: full, stride_ticks: 1, max_samples: u64::MAX` while the
  sampled directory records `retention: sampled`. `full_trajectories_require_a_run_directory`
  (`:337`) pins the flag contract (usage error, exit 2, no trace written).
- `the_manifest_links_the_trajectory_artifact_to_its_run` (`:363`) checks the
  descriptor's path, format, row count, and SHA-256 against the file bytes, and
  that the manifest it lives in carries the scenario, seed, ticks, and applied
  policy that reproduce those rows.
- `a_completed_run_with_trajectories_is_never_rewritten` (`:396`) repeats a
  completed run directory under a different (full) policy: the write fails with
  `RunDirectoryError::Completed` and every artifact, including the Parquet file,
  keeps its exact content hash and row count.
- `a_policy_that_retains_no_trajectories_writes_no_artifact` (`:445`) and
  `a_run_shorter_than_the_stride_writes_an_empty_artifact` (`:474`) cover the
  two edges: an off policy keeps the three-artifact directory, and a run too
  short to reach the stride still writes a readable empty artifact so the
  manifest never names a missing file.
- Unit tests: `the_declared_columns_are_the_written_schema` and
  `a_recorded_sample_carries_the_frames_the_kernel_reports` in
  `trajectories.rs`; `the_declared_policy_keeps_every_event_and_a_bounded_sample`,
  `the_full_policy_keeps_every_tick_with_no_cap`, and
  `a_policy_samples_exactly_the_ticks_it_keeps` in `run_dir.rs`.
- The updated `apps/tangle-cli/tests/run_directory.rs` shows the default run
  directory now holds `events.jsonl.gz`, `manifest.json`, `summary.json`, and
  `trajectories.parquet` (68 sampled rows for the golden walking run), and its
  reproducibility, immutability, and CLI tests now cover the new artifact.

## Claim and write set

Base hash `8e66181b94bbc1e85ba7434a4f7b1fbd7d5cf70a51990eef8b5f921309f36053`
from `braintree hash TAS-034`, claimed by `worker` for 14400 s. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`,
`apps/tangle-cli/Cargo.toml`, `Cargo.lock` (for the CLI dependency addition
only), and this node's own frontier transition. Every changed, created, and
moved path stayed inside it.

## Dependencies reported

Three dependencies added to `apps/tangle-cli/Cargo.toml`, the output layer
only, all Apache `arrow-rs` `59.3.0`: `parquet` with default features off and
`["arrow", "snap"]`, plus `arrow-array` and `arrow-schema` for the record
batch. `Cargo.lock` records the resolved tree. No workspace-level pin was added
and the root `Cargo.toml` is untouched. `cargo tree` over `tangle-model` and
`tangle-sim` holds no Parquet, Arrow, flatbuffers, or thrift crate, and
`scripts/check-dependency-direction.sh` passes.

## Deferrals and open interpretation

- **`max_samples` counts rows, not frames:** one trajectory sample is one agent
  observed at one tick, which is one Parquet row. The cap therefore bounds the
  artifact's row count and its size directly (B1's wording "most trajectory
  samples" left the unit open). A cap that lands mid-frame keeps the rows that
  fit and drops the rest of that frame; `stride_ticks: 1, max_samples: 5`
  exercises that shape. If a later slice wants frame-granular truncation, this
  is the rule to change.
- **A full trajectory declares `max_samples: u64::MAX`** rather than the policy
  carrying "uncapped" as an absent bound, so `max_samples` stays a `u64` and
  the sampling logic is uniform. The manifest therefore always records a
  numeric bound; `retention` and `stride_ticks: 1` identify a full artifact.
- **The stride selects ticks that are multiples of `stride_ticks`,** so a run
  shorter than the stride writes an empty artifact and the first step is not
  sampled. Tick indexing follows the canonical trace, where a step's events and
  its trajectory row carry the same completed tick.
- **The column schema is the artifact's contract:** `tick`, `agent`, `mode`,
  `x_m`, `y_m`, `heading_rad`, `speed_mps`, all non-nullable, Snappy pages. It
  carries no body size, route, or controller detail; a consumer that needs
  more adds columns in a later slice with its own artifact version.
- The default run directory now includes the sampled artifact (68 rows,
  2,765 bytes for the golden walking run), so B1's "three artifacts, no
  trajectory stream" test is replaced by the four-artifact contract. Without
  `--run-dir`, `run` still writes exactly what it always wrote.
- `run` still gains no stride or cap overrides; only the full opt-in is
  exposed on the command line. A caller that wants another bound uses the
  library request.
