---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Increment 4 adds bus stops with aggregate passenger demand, conserved boarding, dwell, and person metrics.
next: Add stops, waiting areas, and aggregate passenger demand with capacity, boarding, alighting, and dwell.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 4:

- Stops, waiting areas, deterministic aggregate passenger cohorts, capacity,
  boarding, alighting, denied boarding, dwell, and departure.
- Bus approach, berth alignment, stop service, and deterministic merge-back
  behavior through ordinary gap-selection and priority rules.
- Transit events, occupancy and person metrics, inspector state, and replay
  support.
- Isolated low-demand, capacity-constrained, variable-dwell, blocked-berth, and
  merge fixtures.

# Done when

- Passenger conservation accounts for every waiting, boarded, alighted, denied,
  and remaining passenger.
- The configured dwell formula is reproduced by hand-computable fixtures and
  its named random stream is isolated.
- Bus occupancy never exceeds capacity or becomes negative.
- Vehicle throughput and person-throughput remain distinct throughout
  summaries and comparisons.

Parent [[TAS-017-phase-2-mixed-traffic]].
