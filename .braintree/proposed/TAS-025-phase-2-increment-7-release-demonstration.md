---
context_rev: 1
priority: P1
updated: 2026-09-12T15:12:03Z
summary: Increment 7 releases the two-variant mixed-traffic demonstration with evidence, model cards, and reproduction.
next: Author the two scenario-only mixed-traffic variants and the checked-in experiment specification.
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 7:

- Two scenario-only variants of one compact intersection containing all
  supported modes and one bus stop.
- A checked-in experiment specification, demand bank, seed bank, manifests,
  results, and comparison report.
- Independent mode evidence, pairwise matrix, full mixed-mode evidence, and
  Standard/Fine sensitivity results.
- Model cards, validated operating ranges, known limitations, replay
  recordings, and one-command reproduction.

# Done when

- Both variants complete agent, passenger, route, and occupancy conservation
  checks.
- The report compares throughput, person-delay, queues, bus service, group
  outcomes, passing clearance, wrong-way exposure, collisions, TTC, PET, and
  minimum separation without hiding a harmed mode in an aggregate.
- Material conclusions remain directionally stable at Fine fidelity or are
  explicitly reported as sensitive.
- A scenario author can add a new template assembled from existing components
  without changing shared physics, safety, metric, or presentation code.

Parent [[TAS-017-phase-2-mixed-traffic]].
