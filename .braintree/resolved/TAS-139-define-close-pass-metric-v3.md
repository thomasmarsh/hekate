---
context_rev: 1
updated: 2026-09-14T22:52:06Z
summary: Define close-pass metric v3 and bump METRIC_DEFINITION_VERSION
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

# Result

Authored the settled [[DEF-006-metric-definition-v3]] and raised the reported
definition to v3. `DEF-006` defines the overtaking families
(`overtake_attempts`, `overtake_commits`, `overtake_completions`,
`overtake_aborts`) and the close-pass families (`close_passes`,
`close_pass_minimum_clearance_m` with its time and relative speed, one duration
series per declared band keyed by its stable `ClearanceBandId`, and
`close_pass_violations`) with formula, unit, applicability (`not_applicable` vs
`not_observed`, never a false `0`), tie-break, and mode-pair/movement/facility
disaggregation, and names
[[TAS-124-add-disaggregated-wrong-way-metrics-and-version]] as the successor that
adds the wrong-way families under this same v3.

`METRIC_DEFINITION_VERSION` (`apps/tangle-cli/src/run_metrics.rs`) is 3. The sole
hardcoded version literal (`apps/tangle-cli/tests/experiment_spec.rs`) now reads
the constant; every other consumer already used it. No aggregation behavior
changed: the accumulation is
[[TAS-140-accumulate-close-pass-metric-families]] and the run-artifact and
aggregate/compare surface is
[[TAS-141-surface-close-pass-metric-families]].

The bump invalidated tracked derived artifacts, relabelled to the new revision
(no metric value changed): the checked-in Increment 6 comparison report,
convergence evidence, and convergence summary under
`experiments/increment6_signal_timing_v1/`, which the experiment tests read
against the constant, and the `metric_definition_version` prose in the run
metrics, aggregation, comparison, convergence, and CLI help doc comments.

Validation: `cargo test -p tangle-cli` (all suites), clippy `-D warnings`, fmt
`--check`, the dependency-direction check, and `braintree check` all pass.

Parent advance: [[TAS-122-close-the-close-pass-observation-and-report-clea]]
`.next` now routes to
[[TAS-140-accumulate-close-pass-metric-families]].
