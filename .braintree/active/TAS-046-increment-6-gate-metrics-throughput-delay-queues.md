---
context_rev: 1
priority: P1
updated: 2026-09-13T11:48:58Z
summary: Slice A of Phase 1 Increment 6 implements the operational gate metrics not reported at v1 - throughput, delay, and queues, plus any unreported conflict/event family - bumps metric_definition_version to 2, and supersedes DEF-004 with the settled v2 definition DEF-005, all with units, applicability, tie-breaks, and mode/movement disaggregation.
next: Implement throughput, delay, and queue metrics and report any remaining gate conflict/event family, bump metric_definition_version to 2, and settle DEF-005 while superseding DEF-004.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Increment 6's gate requires the comparison report to show throughput, delay, and
queues. `DEF-004-metric-definition-v1` explicitly deferred all three (and level
of service), so implementing any of them changes the reported metric set and
forces `metric_definition_version: 2` under the v1 version-bump rule. This slice
implements them and settles the new definition rather than editing the resolved
v1 in place: [[DEF-005-metric-definition-v2]] is the v2 authority and DEF-004 is
superseded. Already-reported v1 artifacts stay attributed to DEF-004.

Consumes the v1 surfaces `crates/tangle-sim/src/metrics.rs`,
`crates/tangle-sim/src/event.rs`, `crates/tangle-cli/src/run_dir.rs`, and the
aggregation/comparison artifacts Increment 5 landed. No new output dependency may
enter `tangle-model` or `tangle-sim`; metric computation may live in the layer
that owns the value, but serialization/output dependencies belong in the
CLI/output layer.

# Outcome

The gate metrics the v1 definition deferred are implemented and versioned, and
the metric definition advances to v2:

- Throughput (persons/agents served per unit time).
- Delay (at least travel time and stopped/control delay).
- Queues (length and duration; queue events already exist).
- Level of service only if trivial and already implied; otherwise leave named as
  deferred in v2.
- Any conflict/event family the gate or comparison report needs that is not yet
  reported.

Each new metric has a unit, an applicability/quality status (reported, not
applicable, not observed), a tie-break, and mode and movement disaggregation
matching the v1 conventions. `metric_definition_version` is 2 wherever a metric
value is emitted.

# Done when

- Throughput, delay, and queue length/duration metrics are implemented and
  tested (unit or property tests for each formula), with units, applicability
  and quality status, tie-break, and mode/movement disaggregation.
- Every unimplemented comparison metric remains explicitly named as deferred in
  v2 rather than silently omitted.
- `metric_definition_version` is 2 in every surface that emits it; a v1 artifact
  is never relabelled.
- [[DEF-005-metric-definition-v2]] is settled from the implemented surface,
  naming each new metric's formula, unit, applicability, tie-break, and exact
  source location, and is resolved.
- `DEF-004-metric-definition-v1` records `disposition: superseded` and
  `Superseded by [[DEF-005-metric-definition-v2]]`; the exact pinned-consumer
  search returns no stale consumer.
- `braintree check` passes, and the working node status, body, and any moves are
  committed coherently.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.
