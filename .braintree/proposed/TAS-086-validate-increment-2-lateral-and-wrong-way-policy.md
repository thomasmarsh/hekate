---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Validate Increment 2 lateral, clearance, transition, and wrong-way policy.
next: Add stable diagnostics and malformed-policy regression cases for every Increment 2 rule.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

Scenario validation rejects unsafe, dangling, contradictory, or physically
impossible Increment 2 policy before simulation and reports stable actionable
diagnostics.

# Done when

- Validation covers finite positive horizons, non-negative clearances,
  increasing/non-overlapping clearance bands, motion-limit feasibility, usable
  width, connector continuity, holder/target kinds, tactic capability, and
  physically connected reverse traversal.
- It rejects transitions across disjoint facilities, policies whose target body
  cannot fit, overtake permission with no capable holder, contradictory effects
  at equal specificity, and contextual wrong-way permission with no opposing
  path.
- Legal prohibition remains distinct from physical impossibility in diagnostic
  codes and messages; a prohibited but connected traversal remains available as
  violation context.
- Table-driven tests assert stable paths/codes for each failure and show that
  malformed inputs never panic.
- All Increment 1 valid fixtures continue to validate unchanged.

# Context

Gated on [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]] and
[[TAS-085-compile-increment-2-policy-and-traversal-semantics]]. Owns
crates/tangle-model/src/validate.rs and focused facility/policy validation tests.
Do not implement tactical runtime decisions.
