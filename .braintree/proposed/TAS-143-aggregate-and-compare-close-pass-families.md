---
context_rev: 1
updated: 2026-09-14T23:25:49Z
summary: Aggregate and compare the close-pass families and dimensions
next: Read the run artifact close_pass block into aggregation.json and comparison.json as their own cells, then pair them by seed.
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
