---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Reconcile the benchmark matrix and narrow-mode cards with checked Increment 2 evidence.
next: Replace planned Increment 2 evidence with checked paths and update both narrow-mode model cards.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

The benchmark matrix and bicycle/scooter model cards point to checked Increment
2 evidence and state the implemented assumptions, ranges, limitations, and
incompatible fidelity settings without expanding the release claim.

# Done when

- Markdown and JSON matrix representations agree on checked passing, close-pass,
  prediction, and wrong-way fixture paths, presets, quantities, tolerances,
  baselines, supported/impossible/deferred cells, and zero remaining planned
  Increment 2 evidence.
- Bicycle and scooter cards describe lateral dynamics, decision cadence,
  predictor horizon, permissions, clearance-band interpretation, unsafe-commit
  behavior, wrong-way context, validated ranges, evidence, and failure modes.
- Cards explicitly exclude balance, lean, falls, biomechanics, implicit
  sidewalk riding, calibrated crash probability, and field calibration.
- Every evidence link exists and every numeric claim is copied from a resolved
  test result, not inferred; matrix consistency and model-card tests pass.
- No source, simulation, scenario, tolerance, or interaction disposition changes.

# Context

Gated on [[TAS-104-bound-predicted-versus-executed-clearance]],
[[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]],
[[TAS-106-check-in-increment-2-passing-fixtures]],
[[TAS-107-check-in-contextual-wrong-way-fixtures]], and
[[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]. Owns
docs/benchmark-matrix.md, docs/benchmark-matrix.json, and the existing bicycle
and scooter model cards only.
