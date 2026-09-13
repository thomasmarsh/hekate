---
context_rev: 1
priority: P1
updated: 2026-09-13T02:10:05Z
summary: Slice D of Phase 1 Increment 5 adds a Fast/Standard/Fine convergence runner that evaluates a scenario at three fidelities over one seed bank and writes a machine-readable sensitivity report, disaggregated by mode and movement and linked to every run manifest and metric_definition_version 1.
next: Read the existing fidelity presets plus the seed-bank/aggregate/compare machinery, then implement `converge` and its sensitivity report with a declared convergence tolerance and tests.
---

# Outcome

Add a convergence runner that evaluates one scenario at the Fast, Standard, and
Fine fidelities over a shared seed bank and writes a machine-readable
sensitivity report. For every reported metric the report gives the value at each
fidelity, the change as the step refines (Fast to Standard to Fine), and a
convergence verdict against a declared tolerance: a metric whose Standard-to-Fine
change exceeds the tolerance is reported as materially sensitive rather than
hidden or averaged away.

Requirements:

- Reuse the existing fidelity presets (the same Fast/Standard/Fine steps the
  baseline already uses) and the seed-bank, batch, aggregation, and paired
  machinery rather than duplicating step constants or statistics.
- The report is deterministic: declared ordering of metrics, fidelities, and
  slices; no wall time or hash-map order in the artifact (timing, if recorded,
  is clearly separated from the reproducible content).
- Disaggregated by mode (`ModePair`) and movement (the DEF-004 key) wherever the
  metric supports it.
- Every reported metric links to the run manifest(s) at each fidelity and to
  `metric_definition_version: 1`, satisfying the increment gate.
- Not-applicable/not-observed values are reported as such, never as zero.
- A `converge` CLI command with a documented contract writes the report to a
  named path or the default and mutates no run artifact.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens stay byte-identical; `EVENT_VERSION` stays 2; no dependency change
  in `tangle-model` or `tangle-sim`.

If the slice outgrows one session, land the first bounded increment (the runner
plus run-level metrics and the convergence verdict), commit with `Refs TAS-041`,
leave the node `active` with a concrete `next` naming the disaggregation, and
report.

# Done when

- `converge` runs one scenario at Fast, Standard, and Fine over a shared seed
  bank and writes a machine-readable sensitivity report.
- The report gives per-metric values per fidelity, the refinement change, and a
  convergence verdict against a declared tolerance, flagging material
  sensitivity.
- The report is disaggregated by mode and by movement.
- Every reported metric links to its manifests at each fidelity and to
  `metric_definition_version: 1`.
- The ordering is deterministic, covered by a test.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

The seed bank (TAS-039), aggregation (TAS-038), paired comparison (TAS-040), and
per-run `metrics.json` (TAS-037) are in place; [[DEF-004-metric-definition-v1]]
is the versioned definition this report cites. Increment 6's gate "selected
findings remain directionally stable at Fine fidelity; material sensitivity is
reported rather than hidden" builds directly on this report. The release-mode
benchmarks and profiler captures are the next direct child (E).

# Result

Pending.
