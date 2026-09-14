---
context_rev: 1
updated: 2026-09-14T23:49:42Z
summary: Aggregate and compare the close-pass families and dimensions
---

Parent [[TAS-141-surface-close-pass-metric-families]].

# Outcome

A batch aggregation and a paired comparison carry the close-pass families metric
definition v3 adds and their mode-pair, movement, and facility dimensions, so a
batch or a paired experiment reports the close-pass evidence its runs recorded.

# Done when

- The aggregation carries the close-pass families over the run and per mode
  pair, per movement, and per facility, each with its unit and its explicit
  applicability, in cells that widen no existing matrix cell or disposition.
- The comparison pairs the same families and dimensions over the seed bank, with
  the same cells and the same no-widening rule.
- Focused aggregate and compare tests cover a batch whose runs recorded a closed
  pass and a bucket that is not applicable, and refuse nothing.

# Context

Extends [[TAS-141-surface-close-pass-metric-families]], which landed the run
artifact `close_pass` block and stopped at that surface.
`apps/tangle-cli/src/aggregate.rs (run_level_readings)` reads a fixed metric
list and `Aggregation` has no close-pass cell, so an aggregation of v3 artifacts
carries no close-pass family; `apps/tangle-cli/src/compare.rs (Comparison)`
mirrors it. The whole-batch keys already follow the artifact path
(`operational.run.<name>`, `event_counts.by_family.<family>`) and the slice maps
are per-bucket. Reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. No new metric
semantics: the families, units, applicability, and disaggregation are
[[DEF-006-metric-definition-v3]].

# Result

Both the aggregation and the comparison cells landed, and the node is complete.

`apps/tangle-cli/src/aggregate.rs (close_pass_readings)` reads one artifact
`ClosePassValues` bucket into the four overtaking counts (unit `records`), the
two countable close-pass families (unit `observations`), the least clearance
(unit `metres`, through the new `Reading::Minimum`), and one cell per declared
band keyed `clearance_band_durations_s.<ClearanceBandId>` (unit `seconds`).
`run_level_readings` carries the run bucket as `close_pass.run.<family>`, so the
run-level families are whole-batch metrics under the artifact block they come
from. `Aggregation` gains `close_pass_mode_pair_slices`,
`close_pass_movement_slices`, and `close_pass_facility_slices`, each keyed
exactly as the artifact keys that dimension, and
`Accumulation::accumulate` reads all three.

`apps/tangle-cli/src/compare.rs` mirrors the same three cells through
`close_pass_slice_readings` and `pair_close_pass_buckets`, so `Comparison` gains
the three matching slice maps and the `close_pass.run.<family>` metrics pair
through `run_level_readings`; `SeedSide::status` and `pair_reading` read the
close-pass minimum's status and value beside the `MetricValue` shape.

No existing cell widened: the mode-pair, mode-event, movement, and
agent-movement cells still hold exactly the metrics they held, which both new
tests assert. A countable family is the observed `0` of a bucket that recorded
none, and a value family keeps the bucket's explicit `not_applicable` (the
bucket cannot host a pass) distinct from `not_observed`.

Evidence: `apps/tangle-cli/tests/aggregate.rs
(the_close_pass_families_aggregate_over_the_run_and_every_dimension)` builds a
two-seed batch whose first run recorded a closed pass — two closed observations,
one overtake from attempt to completion, a 0.4 m least clearance, a violation,
and 1.2 s inside declared band 7 — and whose second recorded none, with a
not-applicable pedestrian mode pair, a not-applicable centered facility, a
not-observed cross pair, and a sparse movement bucket. The batch aggregates
without a refusal: every cell reports its unit, band 7 is `reported` then
`not_observed`, band 9 stays `not_applicable` then `not_observed` rather than
`0`, the countable families report both seeds, and every close-pass cell's
statuses account for both seeds.

`apps/tangle-cli/tests/compare.rs (the_close_pass_families_pair_over_the_seed_bank)`
pairs that batch against one that closed a single observation and then none:
`close_pass.run.close_passes` pairs both seeds (mean difference 1.5), the least
clearance pairs one seed (-0.2) and counts the other unpaired, each declared band
is its own cell, and the not-applicable and not-observed buckets are counted with
both sides' statuses rather than as zero differences.

`a_real_batch_aggregates_its_mode_and_movement_slices` now restates the
close-pass run families from the real artifact and asserts
`close_pass.run.close_passes` is reported by all three seeds with mean `0.0`,
and `every_comparison_links_to_both_manifests_and_the_definition_version`
accepts the `observations` unit.

Gates: `cargo test -p tangle-cli`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo fmt --all --check`,
`scripts/check-dependency-direction.sh`, `braintree check`.
