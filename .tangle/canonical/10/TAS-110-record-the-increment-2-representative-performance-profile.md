---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Record the representative mixed-mode profile and Increment 2 performance budget.
next: Benchmark the fixed representative profile and derive budgets from the Phase 1 baseline.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

A checked representative Increment 2 mixed-mode profile records the cost of
lateral decisions, candidates, prediction, and output and sets declared later
optimization budgets relative to the frozen Increment 0 Phase 1 baseline.

# Done when

- One checked scenario/profile combines passenger cars, bicycles, and scooters
  with passing and opposing interactions at a declared density and fixed seed
  bank; it is an evidence workload, not the final release comparison.
- Release-mode measurements on named hardware record agent-steps/s, tactical
  and broad-phase candidates, prediction work/agent-step, simulated/wall time,
  output bytes/simulated hour, and presenter frame time where applicable.
- The report compares the same common workload at disabled-versus-enabled
  lateral machinery and to the frozen Phase 1 baseline without claiming
  cross-hardware equivalence.
- Budgets, dominant measured work, profiler commands, raw artifact hashes, and
  rerun commands are checked in; no optimization is performed in this leaf.
- The workload reproduces, tests pass, and unsupported density/fidelity ranges
  are labeled.

# Context

Gated on [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]] and
[[TAS-109-document-increment-2-evidence-and-model-limits]]. Owns one Increment 2
performance workload, its seed bank if distinct, and benchmark/report artifacts
under the repository's existing benchmark conventions. Do not optimize code or
author the final mixed-mode release fixture.

Gates my scenario and benchmark artifacts enter: scenario enumeration, any
checked benchmark-artifact manifest test, and cargo test --workspace.

# Slices

- Add the representative mixed-mode workload fixture: cars, bicycles, and
  scooters with passing and opposing interactions at a declared density and a
  fixed seed bank.
- Record release-mode measurements on named hardware: agent-steps per second,
  tactical and broad-phase candidates, prediction work per agent-step,
  simulated/wall time, output bytes per simulated hour, and presenter frame time.
- Write the disabled-versus-enabled and Phase 1 baseline comparison, budgets,
  profiler commands, raw artifact hashes, and rerun commands; label unsupported
  density and fidelity ranges.
