---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Check in contextual wrong-way fixtures including an occupied opposing corridor.
next: Author and test clear, prohibited, disconnected, and occupied opposing-traversal scenarios.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Checked version-2 scenarios prove contextual opposing-route selection, ordinary
head-on interaction, explicit rule evidence, and the inability to bypass an
occupied or disconnected corridor.

# Done when

- Fixtures cover a permitted nominal choice, a contextual prohibited-but-
  connected wrong-way choice, a physically disconnected rejection, and an
  occupied opposing corridor with deterministic wait/brake/abort behavior.
- Tests assert perceived rule, reason, affected movements, interval boundaries,
  distance/duration/exposure/encounters/conflicts, ordinary collision and
  near-miss visibility, and route completion or explicit non-entry.
- Reversing facility/reference declarations does not change the physical
  outcome after stable IDs are accounted for.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no interaction disposition or tolerance is widened.
- CLI validate and run plus cargo test --workspace pass at required presets.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] and
[[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]]. Owns
scenarios/phase2/inc2 wrong-way fixtures, one focused integration suite, and
only their path entries in docs/benchmark-matrix.md and .json.

Gates my scenario artifacts enter: apps/tangle-cli/tests/migration_regression.rs
enumerates scenarios/**/*.json5; crates/tangle-present/tests/v2_fixtures.rs may
enumerate version-2 fixtures; benchmark-matrix checked-path tests enumerate the
matrix fixture paths.
