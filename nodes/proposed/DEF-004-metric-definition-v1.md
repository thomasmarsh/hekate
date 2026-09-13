---
context_rev: 1
priority: P1
updated: 2026-09-13T00:55:58Z
summary: The versioned metric definition v1 names every metric Increment 5 reports, its formula, units, applicability, disaggregation keys, and tie-breaks, so a run artifact or aggregate can link each reported number to a stable definition revision.
next: Settle the definition from the implemented metric and event surfaces, then resolve this node.
---

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Increment 5's gate requires that "every reported metric links back to
manifest(s) and a versioned metric definition". This node is that durable,
independently resumable definition: it is the authority the run summary,
aggregation, and sensitivity report cite, and it is versioned so a later metric
change can be a new revision rather than a silent reinterpretation.

The definition must be settled from what the implementation actually computes —
`crates/tangle-sim/src/metrics.rs` (time to collision, minimum separation,
occupancy/PET) and `crates/tangle-sim/src/event.rs` (the typed safety records:
collision, near miss, violation, entry/exit, queue, control transition) — not
invented. If a `PHASE_1_PLAN.md` metric is not implemented yet (for example
throughput, delay, or level of service), the definition names it as deferred
rather than claiming it.

# Invariant

Pending settlement. The settled invariant will record:

- `metric_definition_version: 1`.
- For every reported metric: name, formula/derivation, unit, applicability or
  quality status (for example TTC's not-applicable case), and its stable
  tie-break.
- The disaggregation keys the metrics can be sliced by: mode
  (`vehicle`/`pedestrian`) and movement (the turn/route/movement identity the
  model exposes; if the model exposes more than one candidate, name the chosen
  one and why).
- Which metrics are deferred to a later version, and the rule for bumping the
  version.
- The artifact field that carries this version (for example
  `metric_definition_version` in the summary and aggregation outputs).

# Done when

- The invariant is settled from the implemented surfaces, naming each metric,
  its unit, applicability, disaggregation keys, and tie-break, with the exact
  source locations that compute it.
- `metric_definition_version` is fixed at 1 and the version-bump rule is
  stated.
- The node is resolved (the act of settling it) and is the definition later
  Increment 5 artifacts cite.

# Result

Pending.
