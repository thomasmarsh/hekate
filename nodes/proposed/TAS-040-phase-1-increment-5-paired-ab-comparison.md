---
context_rev: 1
priority: P1
updated: 2026-09-13T01:55:00Z
summary: Slice C2b of Phase 1 Increment 5 adds a `compare` command that pairs two batches run from one common-random-number seed bank and reports per-metric mean differences with a paired confidence interval, disaggregated by mode and movement and linked to both manifests and metric_definition_version 1.
next: Read TAS-038's aggregation and TAS-039's seed-bank/manifest reference, then implement the paired A/B comparison with a documented paired CI and tests.
---

# Outcome

Add a `compare` command to the Tangle CLI that performs the A/B comparison the
CRN seed bank exists for. It reads two completed batches (`--a` and `--b`) and
the seed bank they were run from, proves both sides used the same bank (same
content hash) and the same seed order, and then pairs the runs by seed.

For every reported metric it computes the per-seed paired difference
`d_i = value_A(seed_i) - value_B(seed_i)` and reports the mean difference with a
paired confidence interval (Student-t on the differences, consistent with
TAS-038's method). Results are disaggregated by mode (`ModePair`) and by
movement (the DEF-004 movement key), ordered deterministically, and each
comparison links to both run manifests and to `metric_definition_version: 1`.

Requirements:

- A missing seed on either side, a bank-hash mismatch, a different seed order, a
  duplicate seed, or a metric one side reports and the other does not is refused
  with an actionable diagnostic rather than silently unpaired.
- Not-applicable/not-observed pairs are excluded from the paired statistic and
  counted, so a metric whose pair has no value for some seeds reports its paired
  `n` rather than a fabricated zero.
- The command writes a machine-readable comparison artifact (for example
  `comparison.json`) to a named path or the default, and mutates no run artifact.
- The paired comparison is the CRN payoff: state in the node result how pairing
  differs from the unpaired per-side interval (the variance the two variants
  share is removed).

If the slice outgrows one session, land the paired engine and run-level metrics
first, commit with `Refs TAS-040`, leave the node `active` with a concrete
`next` naming the disaggregation, and report.

# Done when

- `compare` pairs two batches from one seed bank and writes a machine-readable
  per-metric paired comparison with mean difference and a documented paired
  confidence interval.
- The comparison is disaggregated by mode and by movement.
- Every comparison links to both run manifests and to
  `metric_definition_version: 1`.
- Unpaired or mismatched inputs (missing seed, bank-hash mismatch, duplicate
  seed, differing metric availability) are refused, covered by tests.
- The ordering is deterministic, covered by a test.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

TAS-039 (C2a) added the versioned seed bank, the `seed-bank` command, and
`batch --seed-bank` with the bank identity in the batch manifest. TAS-038 (C1b)
added across-seed aggregation with a documented Student-t interval. This child
combines them into the paired A/B comparison the plan asks for. The Fast/
Standard/Fine convergence runner and sensitivity report (D) will consume this.

# Result

Pending.
