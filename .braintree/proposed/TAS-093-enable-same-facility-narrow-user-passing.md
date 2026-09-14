---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Enable bicycle and scooter passing of slower narrow users on one shared facility.
next: Integrate same-facility narrow passing from eligibility through clearance and return.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

A bicycle or scooter with pass capability can overtake a slower narrow leader
inside one shared continuous-width facility using the common predictor, claim,
state-machine, steering, collision, and longitudinal-control paths.

# Done when

- Eligibility requires capability, applicable permission, route benefit,
  sufficient usable width, visible slower leader, target clearance, and a
  feasible horizon; each rejected precondition has an inspectable reason.
- The target offset selects a deterministic side from geometry and policy, the
  passing agent claims a body corridor, clears the leader, and returns without
  changing route identity or teleporting.
- Leader/follower longitudinal behavior remains active throughout; a pass is
  not implemented as speed or position scripting.
- Bicycle-passes-scooter, scooter-passes-bicycle, no-width, prohibited, and
  no-benefit focused tests cover both travel directions and stable decisions.
- No named bicycle or scooter branch enters shared controller, query, collision,
  or output modules.

# Context

Gated on [[TAS-091-resolve-gap-claims-and-maneuver-transitions]]. Owns the
narrow passing eligibility/target tactic seam and focused tests. Do not add
motor-vehicle overtaking, connector transitions, close-pass metrics, or broad
acceptance fixtures.
