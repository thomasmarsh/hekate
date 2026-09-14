---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Define the exact Increment 2 schema, state machines, policies, and evidence semantics.
next: Extend docs/schema-v2-contract.md with the complete Increment 2 consumer contract.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

docs/schema-v2-contract.md extends version 2 in place with the complete
consumer-visible Increment 2 contract before implementation begins.

# Done when

- Exact source fields, types, units, omission/default rules, identifier targets,
  and normalized examples are fixed for lateral-use policy, target clearance
  and horizon, adjacent-lane or facility transitions, clearance bands, unsafe
  commit policy, lane_use/overtake/crossing permissions, and contextual
  nominal-direction violation.
- Compiled semantics separate nominal, permitted, and physically possible
  traversal; define usable and predicted corridors; and name front, rear, side,
  and swept clearance facts.
- The following/preparing/committed/returning/aborted transitions, simultaneous
  claim tie-break, commitment loss policy, wrong-way decision inputs, and
  violation interval boundaries are unambiguous and deterministic.
- The event and metric section names payloads, ordering, applicability, and the
  exact definition-version bumps that later consumers must observe.
- Increment 1 deferral rows are reconciled, later-increment scope remains
  deferred, and all cited implementation seams exist.

# Context

Read PHASE_2_PLAN.md sections Continuous lateral motion, Overtaking and close
passing, Wrong-way movement, Safety operations and outputs, Reproducibility and
fidelity, and Decisions for implementation; read the resolved Increment 1
contract leaf [[TAS-073-extend-the-version-2-schema-contract-with-increm]].
Owns docs/schema-v2-contract.md only. Do not edit source, generated schema,
benchmark matrix, or resolved node files.

This documentation is the dependency contract for TAS-084 through TAS-110.
