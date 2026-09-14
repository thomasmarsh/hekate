---
context_rev: 1
updated: 2026-09-14T22:46:00Z
summary: Accumulate close-pass metric families in RunMetrics
next: Accumulate close-pass attempts, aborts, completions, violations, and clearance-band distributions by dimension in RunMetrics.
---

Parent [[TAS-122-close-the-close-pass-observation-and-report-clea]].

# Outcome

Closed `ClosePass` observations accumulate into `RunMetrics` as attempts,
aborts, completions, violations, and clearance-band distributions, by mode pair,
movement, facility, and applicability.

# Done when

- `apps/tangle-cli/src/run_metrics.rs (RunMetricsRecorder)` accumulates the
  close-pass families from the closed observations with explicit applicability.
- Nominal travel is not counted, and an inapplicable mode reports no close-pass
  family.
- Focused tests cover the accumulation and inapplicable modes.

# Context

Extends [[TAS-122-close-the-close-pass-observation-and-report-clea]] and depends
on the sibling metric-definition-v3 node for the family names. Owns the
accumulation only; the run-artifact and aggregate/compare surface is the next
sibling slice.
