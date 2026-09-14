---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Check in the contextual wrong-way fixtures.
next: Add the permitted, prohibited-but-connected, disconnected, and occupied-corridor fixtures with their rule assertions.
---

Parent [[TAS-107-check-in-contextual-wrong-way-fixtures]].

# Outcome

Checked version-2 scenarios prove contextual opposing-route selection, ordinary
head-on interaction, and explicit rule evidence.

# Done when

- Fixtures cover a permitted nominal choice, a contextual prohibited-but-connected
  wrong-way choice, a physically disconnected rejection, and an occupied opposing
  corridor with deterministic wait, brake, or abort behavior.
- Tests assert perceived rule, reason, affected movements, interval boundaries,
  distance, duration, exposure, encounters, conflicts, ordinary collision and
  near-miss visibility, and route completion or explicit non-entry.
- CLI validate and run pass.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Extends [[TAS-107-check-in-contextual-wrong-way-fixtures]]; reads the fixture
gates in [[THO-016-increment-2-event-metric-trajectory-presenter-an]].
`apps/tangle-cli/tests/migration_regression.rs` enumerates `scenarios/**/*.json5`.
Owns the core wrong-way fixtures; the matrix wiring is the sibling slice.
