---
context_rev: 1
priority: P3
updated: 2026-09-13T14:13:11Z
summary: Metric definition v2 disaggregates time to collision and post-encroachment time by movement pair and run but not by mode pair, so the Increment 6 comparison cannot report those two pair metrics per mode pair; add per-mode-pair aggregation under a new metric-definition revision.
next: Add per-mode-pair aggregation for time to collision and post-encroachment time and settle the resulting metric-definition revision, reconciling every pinned consumer.
---

# Context

Area [[IDX-001-tangle]]. Increment 6 review finding (P2), disclosed rather than
hidden by [[TAS-048-increment-6-run-and-comparison-report]] and
[[DEF-005-metric-definition-v2]] and recorded in `docs/known_limitations.md`.

`DEF-005-metric-definition-v2` carries the v1 pair metrics forward: minimum
surface separation is keyed by mode pair (`vehicle_vehicle`,
`vehicle_pedestrian`, `pedestrian_pedestrian`), while time to collision and
post-encroachment time are keyed by movement pair and over the run. The
Increment 6 gate asks the comparison to report TTC and PET "by mode and
movement", so this node owns the missing per-mode-pair disaggregation for those
two metrics.

Changing a reported metric's disaggregation keys is a definition revision under
the v2 bump rule, so the work includes the revision and the reconciliation of
every pinned consumer, not only the code.

# Outcome

Time to collision and post-encroachment time carry per-mode-pair slices
consistent with the existing mode-pair conventions, reported by the comparison
report at the new metric-definition revision, with no consumer reporting a
metric at the wrong definition revision.

# Done when

- TTC and PET per-mode-pair minima are computed and exposed, with unit,
  applicability/quality status, tie-break, and mode-pair disaggregation matching
  minimum separation.
- The metric definition is revised to the next revision, naming the new
  disaggregation with exact source locations; `DEF-005` records the change
  (superseded or in-place per the version-bump rule) and every pinned consumer
  is reconciled.
- The comparison report (and aggregation/convergence where applicable) reports
  TTC and PET by mode pair, and tests cover the new slices.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.
