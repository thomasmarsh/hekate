---
context_rev: 1
priority: P1
updated: 2026-09-13T20:54:12Z
summary: Increment 1 adds continuous-width facilities and bicycle/scooter modes with longitudinal behavior and isolated fixtures.
next: [[TAS-079-prove-narrow-mode-reproducibility-and-independen]]
---

# Outcome

Per `PHASE_2_PLAN.md` Increment 1:

- Continuous-width facilities, reference-path coordinates (arc length `s`,
  signed lateral offset `d`, tangent, normal, curvature), directional
  traversal, access rules, and movement connectors.
- Version-2 validation for containment, usable width, curvature against turning
  limits, connector continuity, directional reachability, mode-to-facility
  access, legal versus physically possible routes, and spawn clearance for the
  largest eligible body.
- Bicycle and scooter templates, demand, profiles, bodies, isolated controllers,
  and model cards.
- Longitudinal following, stops, signals, priorities, facility selection, and
  route completion before free lateral maneuvers are enabled.
- Isolated straight, curve, braking, following, signal, and crossing fixtures
  for both modes.

# Done when

- Path/world coordinate round trips remain within declared tolerance on
  straight and curved fixtures.
- Bicycle and scooter motion respects dimensions, speed, acceleration, braking,
  steering, and facility boundaries.
- Repeated seeded runs reproduce demand, profiles, decisions, events, and trace
  hashes.
- Each mode passes independent fixtures without relying on car-specific
  dimensions or controller defaults.

Parent [[TAS-017-phase-2-mixed-traffic]].
