---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:12Z
summary: Make contextual bicycle and scooter wrong-way decisions reproducible and inspectable.
next: [[TAS-115-define-contextual-wrong-way-decision-inputs-and]]
---

Parent [[TAS-096-contextual-wrong-way-travel]].

# Outcome

An eligible narrow agent makes one deterministic, reason-coded decision about
opposing traversal from perceived rule, route savings, expected delay, facility
type, observed density, urgency, and its stable compliance profile.

# Done when

- The decision reads only the observer-stage context fixed by TAS-083 and does
  not add a detailed visibility-error subsystem.
- Random choice, if required, is keyed by root seed, run ID, stable AgentId, and
  the versioned maneuver stream; draw order and unrelated agents cannot change
  it.
- Decision output contains perceived permission/obligation, selected nominal or
  opposing option, reason code, relevant movement IDs, and the context values
  used.
- No physically connected opposing option yields an explicit rejection before
  any claim or motion; a legal prohibition remains available to a
  non-compliance decision.
- Table-driven and fixed-seed tests cover each reason, threshold ties,
  compliance extremes, reversed declaration order, and unrelated-agent
  isolation.

# Context

Gated on [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] and
[[TAS-091-resolve-gap-claims-and-maneuver-transitions]]. Owns wrong-way
observation/decision logic, the maneuver RNG stream, and focused tests. Do not
execute the route or emit public events.

# Slices

- [[TAS-115-define-contextual-wrong-way-decision-inputs-and]] Wrong-way decision inputs and reason-coded output.
- [[TAS-116-key-the-wrong-way-draw-and-reject-impossible-opp]] Draw keying and impossible-option rejection.
