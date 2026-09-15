---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Increment 6 adds versioned mixed-mode safety metrics, operational summaries, and disaggregated batch comparison.
next: Version the interaction taxonomy and mode-pair conflict metrics on top of the mode events.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 6:

- Versioned interaction taxonomy and mode-pair conflict metrics, articulated
  segment evidence, close-pass reports, and wrong-way exposure.
- Person- and vehicle-based operational summaries, bus service summaries, and
  pedestrian-group summaries, with an explicit `unknown` classification rather
  than forcing ambiguous interactions.
- Batch comparison and confidence intervals disaggregated by mode, movement,
  facility, and participant pair.
- Common-random-number handling that preserves comparable demand and profiles
  when a design changes feasible routes.
- Viewer inspection and replay for maneuver histories, groups, occupancy, body
  segments, and mixed-mode safety events.

# Done when

- Every aggregate value can be traced to versioned event/trajectory definitions
  and source manifests.
- Metrics state when TTC or another surrogate is inapplicable instead of
  emitting a misleading number.
- Serial and parallel batches produce identical per-run hashes.
- Output size and runtime remain within declared sampling and benchmark
  budgets.

Parent [[TAS-017-phase-2-mixed-traffic]].
