---
context_rev: 1
updated: 2026-09-14T22:46:00Z
summary: Define close-pass metric v3 and bump METRIC_DEFINITION_VERSION
next: Author DEF-006 metric definition v3 and increment METRIC_DEFINITION_VERSION to 3.
---

Parent [[TAS-122-close-the-close-pass-observation-and-report-clea]].

# Outcome

The close-pass metric families are a settled metric definition (v3) and the
reported `metric_definition_version` is 3.

# Done when

- A new `DEF-006-metric-definition-v3` node defines every close-pass family the
  increment reports - attempts, aborts, completions, violations, and clearance
  distributions by band - with formula, unit, applicability, tie-break, and
  mode/movement/facility disaggregation, and names
  [[TAS-124-add-disaggregated-wrong-way-metrics-and-version]] as the successor
  that adds the wrong-way families under the same version.
- `apps/tangle-cli/src/run_metrics.rs (METRIC_DEFINITION_VERSION)` is 3, and
  every consumer/test that pins 2 is updated.
- No aggregation behavior changes in this slice.

# Context

Extends [[TAS-122-close-the-close-pass-observation-and-report-clea]]; the
close-pass observation and event landed there. Definition and constant only; the
aggregation is the sibling slice.
