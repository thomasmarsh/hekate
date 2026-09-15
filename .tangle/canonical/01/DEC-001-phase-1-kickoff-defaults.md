---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Phase 1 kickoff defaults are macOS, JSON5 plus JSON Schema, and a signalized four-leg comparison case.
---

# Decision

Proceed with the PHASE_1_PLAN.md recommendations unless an alternative is
selected:

1. Primary development OS is macOS, with Linux CI build/test coverage before the
   first release.
2. Scenario source syntax is JSON5 parsed with Serde, plus a generated,
   checked-in JSON Schema from Schemars, with semantic validation beyond what
   JSON Schema can express.
3. The Phase 1 comparison case is two fixed-time signal/control variants of one
   four-leg car/pedestrian intersection.

# Rationale

macOS matches the current development environment. JSON5 keeps the format close
to plain JSON while allowing comments and trailing commas for hand-authored
geometry. A fixed-time signal comparison avoids forcing yielding and lateral
behavior into Phase 1.

# Consequences

- `hekate-model` uses JSON5 and Schemars rather than plain JSON or RON.
- A roundabout comparison is deferred until yielding and lateral path behavior
  exist.

Area [[IDX-001-hekate]].
