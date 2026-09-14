---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Resolve simultaneous gap claims and the complete lateral maneuver state machine.
next: Implement deterministic claim arbitration and every documented transition with focused tests.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

The tactical stage advances exactly one explicit maneuver lifecycle per eligible
agent and resolves simultaneous claims on shared corridor space with a stable,
documented priority and AgentId tie-break.

# Done when

- following enters preparing only with a target and candidate corridor;
  preparing commits only after a successful claim; committed reaches returning
  after clearing the passed obstacle; returning reaches following at its target
  position; preparing or committed can enter aborted under the fixed policy.
- Claims are collected from one immutable observation, arbitrated as a batch
  before commands, and cannot depend on mutable agent iteration order.
- Loss of predicted clearance after commitment produces the documented
  brake/hold/abort choice without revoking another winner mid-step.
- Timeouts and target disappearance have explicit deterministic transitions;
  all state transitions expose one record for later TAS-100 event emission.
- Unit tests cover the legal transition table, rejected claims, same-gap ties,
  reversed insertion order, target despawn, timeout, and committed hazard.

# Context

Gated on [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] and
[[TAS-090-predict-maneuver-corridors-and-clearance]]. Owns the tactical-choice
and claim-resolution seams in crates/tangle-sim plus focused tests. It supplies
mechanism only; tactic eligibility for the passing families is TAS-093 through
TAS-095.
