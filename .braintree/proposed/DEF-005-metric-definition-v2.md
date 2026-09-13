---
context_rev: 1
priority: P1
updated: 2026-09-13T11:48:58Z
summary: Metric definition v2 adds the operational metrics v1 deferred - throughput, delay, and queue length/duration - to the v1 interaction and event-count metrics, fixes metric_definition_version at 2, and supersedes DEF-004-metric-definition-v1 once settled from the implemented surface.
---

# Context

Parent [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]].

This revision replaces metric definition v1 (the resolved `DEF-004` node): Increment 6's gate needs
throughput, delay, and queues, which v1 explicitly deferred, so the reported
metric set changes and the version-bump rule in v1 fixes a new revision rather
than a silent reinterpretation. Two definition revisions never share a version,
so v2 is this node and v1 stays the authority for artifacts already reported at
v1.

While the invariant is unsettled this node stays `proposed`. Settling it is the
act of resolving it, and it is settled only from the metrics slice's implemented
surface, with exact source locations.

# Invariant

_(unsettled — to be settled by TAS-046 from the implemented metric surface)_

Metric definition v2 carries forward every v1 metric unchanged and adds the
operational metrics Increment 6 reports:

- **Throughput** — unit and denominator to be stated.
- **Delay** — travel time and stopped/control delay, units and derivation to be
  stated.
- **Queue length and queue duration** — units and derivation to be stated,
  distinct from the existing per-agent `Event::Queue`.
- **Any other comparison metric implemented by the slice**, with formula, unit,
  applicability/quality status, tie-break, and mode/movement disaggregation.
- **Level of service** and any other still-unimplemented v1-deferred metric is
  named deferred, not claimed.

`metric_definition_version` is fixed at 2, and the v1 version-bump rule and the
reporting-constant list carry forward. The field `metric_definition_version`
appears beside every reported metric value.

# Done when

- The invariant is settled from the implemented surfaces, naming each new
  metric's formula, unit, applicability, tie-break, and exact source location,
  and restating the carried-forward v1 set and the still-deferred metrics.
- `metric_definition_version` is fixed at 2 and the bump rule is restated.
- The resolved v1 definition node records `disposition: superseded`, its
  canonical `Superseded by` edge points at this node, and no pinned consumer is
  stale.
- The node is resolved (the act of settling it) and is the definition the
  Increment 6 run summary, aggregation, and comparison report cite.
