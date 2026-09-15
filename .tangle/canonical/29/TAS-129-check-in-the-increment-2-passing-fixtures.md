---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Check in the Increment 2 passing fixtures.
next: Add the minimal passing, motor-overtaking, and lane-change fixtures under scenarios/phase2/inc2 with their assertions.
---

Parent [[TAS-106-check-in-increment-2-passing-fixtures]].

# Outcome

Checked version-2 scenarios exercise same-facility narrow passing,
motor-over-narrow overtaking, and configured lane or facility changes through the
CLI.

# Done when

- `scenarios/phase2/inc2` contains minimal valid fixtures for bicycle and scooter
  passing, a motor vehicle overtaking a narrow user, and a configured transition
  around a slower leader.
- Each fixture asserts attempt, commit, abort, and complete behavior, continuous
  lateral samples, motion and boundary limits, exact close-pass evidence, route
  completion, and no silent contact.
- CLI validate and run pass.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Extends [[TAS-106-check-in-increment-2-passing-fixtures]]; reads the fixture gates
in [[THO-016-increment-2-event-metric-trajectory-presenter-an]].
`apps/hekate-cli/tests/migration_regression.rs` enumerates `scenarios/**/*.json5`,
so this new directory enters that suite. Owns the core fixtures; the unsafe
variants are the sibling slice.
