---
context_rev: 1
updated: 2026-09-14T23:49:42Z
summary: Surface close-pass metric families in run artifacts and comparison
---

Parent [[TAS-122-close-the-close-pass-observation-and-report-clea]].

# Outcome

The close-pass metric families survive the immutable run artifact and appear in
aggregate and compare under metric definition v3.

# Done when

- The run-directory metric artifact serializes the close-pass families and
  dimensions and they round-trip through replay.
- Aggregate and compare preserve the new families and dimensions without
  widening an existing matrix cell or disposition.
- Focused run-directory, aggregate, and compare tests pass.

# Context

Extends [[TAS-122-close-the-close-pass-observation-and-report-clea]] and depends
on the sibling accumulation node. Reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. No new metric
semantics.

# Result

Both halves landed; the node is complete.

The run-directory half: `apps/tangle-cli/src/run_metrics.rs
(RunMetricsArtifact::new)` copies the `close_pass` block into the artifact, so
`metrics.json` carries the overtaking and close-pass families over the run, per
mode pair, per movement, and per facility, and `apps/tangle-cli/src/lib.rs`
re-exports `ClosePassMetrics`, `ClosePassMinimum`, and `ClosePassValues` with the
rest of the run-metrics surface. `ClosePassMetrics::not_observed()` is the empty
shape the synthetic-batch test fixtures build into their stand-in artifacts.
Evidence: `apps/tangle-cli/tests/run_metrics.rs
(the_run_artifact_serializes_and_round_trips_the_close_pass_families)` writes a
run of the close-pass fixture to a run directory and reads it back — all three
mode-pair buckets, the pairwise movement bucket, the `facility:road` bucket, and
the three-band series are in the artifact, the parsed block equals the capture
the recorder made, re-serializing it reproduces the recorded bytes exactly, and
`replay_run_directory(.., verify)` reproduces the recorded event stream.

The batch half ([[TAS-143-aggregate-and-compare-close-pass-families]]):
`apps/tangle-cli/src/aggregate.rs (close_pass_readings)` carries the run bucket
as the `close_pass.run.<family>` whole-batch metrics and the mode-pair, movement,
and facility dimensions as `close_pass_mode_pair_slices`,
`close_pass_movement_slices`, and `close_pass_facility_slices`, each with its
unit and explicit applicability; `apps/tangle-cli/src/compare.rs` pairs the same
cells over the seed bank. No existing matrix cell widened. Evidence:
`apps/tangle-cli/tests/aggregate.rs
(the_close_pass_families_aggregate_over_the_run_and_every_dimension)` and
`apps/tangle-cli/tests/compare.rs
(the_close_pass_families_pair_over_the_seed_bank)` cover a batch whose runs
recorded a closed pass and a bucket that is not applicable, and refuse nothing.

Gates: `cargo test -p tangle-cli`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo fmt --all --check`,
`scripts/check-dependency-direction.sh`, `braintree check`.
