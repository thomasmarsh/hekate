---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Fix the increment-0 benchmark matrix and quantitative validation tolerances.
next: Write the benchmark matrix mapping every mode, interaction family, and mixed-mode cell to a fixture and its quantitative tolerance.
---

Parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]].

# Outcome

Per `PHASE_2_PLAN.md` Increment 0 and the validation ladder: a checked-in
benchmark matrix names every independent-mode, pairwise, and mixed-mode cell,
its fixture, and the quantitative tolerance that decides pass or fail, including
which fidelity presets apply.

# Done when

- The matrix covers every Phase 2 mode, every material mode pair, and each interaction family with an explicit supported, impossible, or deferred disposition.
- Every cell names its fixture, the quantity compared, the tolerance, and the fidelity presets.
- Tolerances are metric bounds, not golden-file equality, and each names the baseline it derives from.

# Context

Owned by [[TAS-018-phase-2-increment-0-baseline-extension-contract]]; this is a planning artifact for later increments, not an implementation task.
