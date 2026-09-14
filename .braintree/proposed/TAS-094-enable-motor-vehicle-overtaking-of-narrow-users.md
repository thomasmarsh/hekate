---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Enable component-driven motor-vehicle overtaking of narrow users.
next: Integrate motor overtaking eligibility, displacement, clearance, and return.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

A single-body motor vehicle can pass a slower bicycle or scooter with lateral
displacement only when authored permission, body-aware geometry, visibility,
and predicted clearance allow it.

# Done when

- Eligibility and rejection reasons use body/motion/tactic/access components,
  not passenger_car, bicycle, or scooter identifiers.
- The predictor accounts for the motor vehicle box, narrow capsule, boundary or
  opposing-facility use, relative speed, front/rear traffic, and target
  clearance throughout displacement and return.
- Prohibited boundary crossing cannot occur silently; an allowed use of an
  opposing facility remains an ordinary connected transition with a claim.
- Focused tests cover safe pass, insufficient lateral room, insufficient rear
  gap, opposing occupancy, permission prohibition, and deterministic side
  choice.
- Existing longitudinal car following and vehicle-yielding fixtures remain
  unchanged when no overtake capability/policy is authored.

# Context

Gated on [[TAS-093-enable-same-facility-narrow-user-passing]]. Owns the
component-driven motor-over-narrow tactic seam and focused tests. Do not
implement generic adjacent-facility transition completion, events, or metrics.
