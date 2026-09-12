---
context_rev: 1
priority: P1
updated: 2026-09-12T15:12:03Z
summary: Increment 5 adds pedestrian groups and pairwise mode-pair interaction fixtures across the interaction families.
next: Add pedestrian group generation with shared crossing intent, bounded cohesion, splitting, and rejoining.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 5:

- Group generation, shared crossing intent, cohesion, splitting, and rejoining
  as a relationship among ordinary pedestrian agents, not one large body.
- Pairwise fixtures for every material mode pair and interaction family:
  following, crossing, merging, passing, opposing, and shared facility.
- Decision-reason and interaction-classification records sufficient to explain
  yields, stops, rejected gaps, and deadlocks.
- A scripted density ramp that identifies the operating range before persistent
  gridlock or model breakdown.

# Done when

- Group members retain individual contacts and can split rather than forcing an
  infeasible group envelope.
- Nominal pairwise fixtures complete without NaNs, unresolved overlaps, or
  indefinite mutual yielding.
- Every mode pair has an explicit supported, impossible, or deferred
  disposition for each interaction family.
- Results outside the validated density range are labeled rather than silently
  treated as credible.

Parent [[TAS-017-phase-2-mixed-traffic]].
