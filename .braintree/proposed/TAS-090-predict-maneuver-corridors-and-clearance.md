---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Predict usable maneuver corridors and conservative front, rear, side, and swept clearance.
next: Implement the pure finite-horizon corridor predictor with analytic unit fixtures.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

A pure deterministic query evaluates whether a proposed lateral trajectory fits
its connected facilities and conservatively predicts minimum body-to-body
clearance over the configured horizon.

# Done when

- Inputs are compiled geometry, current world bodies and velocities, bounded
  candidate motion, body envelope, target clearance, and horizon; outputs name
  feasibility plus the limiting boundary or agent and front/rear/side/swept
  clearance facts.
- Candidate collection uses the ordinary broad phase, stable AgentId ordering,
  and exact body shapes; the final result does not depend on insertion order.
- Prediction covers the whole transition corridor, facility boundaries, and
  both current and destination flows, not only endpoint lane-centre gaps.
- Analytic straight-line fixtures cover approaching rear, slower front,
  side-by-side, crossing sweep, empty corridor, curved boundary, and ties.
- A conservative result may reject a marginal gap but never reports feasible
  when its own fine subdivision detects less than target clearance.

# Context

Gated on [[TAS-089-integrate-bounded-single-body-steering]]. Owns a focused
prediction/corridor module, minimal query exports, and unit tests. Reuse
body_clearance_m, swept bodies, tick_minimum_clearance_m, CompiledFacility, and
the spatial index; do not choose or commit tactics.
