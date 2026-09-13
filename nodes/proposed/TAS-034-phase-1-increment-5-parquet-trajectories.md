---
context_rev: 1
priority: P1
updated: 2026-09-13T00:20:53Z
summary: Slice B2 of Phase 1 Increment 5 writes selectively sampled agent trajectories into the immutable run directory as Parquet, driven by the SamplingPolicy fixed in B1, so output size is bounded by the policy and full trajectories are opt-in.
next: Read the B1 run-directory layout (`RunManifest`/`SamplingPolicy`) and add the Parquet trajectory writer plus its round-trip and size-bound tests.
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

Pending.
