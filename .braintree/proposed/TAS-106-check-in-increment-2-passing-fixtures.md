---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Check in Increment 2 narrow passing, motor overtaking, and lane-transition fixtures.
next: Author and test the three passing families plus prohibited and close-clearance variants.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Checked version-2 scenarios exercise same-facility narrow passing,
motor-over-narrow overtaking, and configured lane/facility changes, including
close-clearance and prohibited-boundary variants, through the CLI.

# Done when

- scenarios/phase2/inc2 contains minimal valid fixtures for bicycle/scooter
  passing, a motor vehicle overtaking a narrow user, and a configured transition
  around a slower leader, plus unsafe/prohibited variants needed by the gate.
- Each fixture asserts attempt/commit/abort/complete behavior, continuous
  lateral samples, motion and boundary limits, exact close-pass evidence, route
  completion, and no silent contact.
- Tests run every relevant fixture at the benchmark matrix presets and preserve
  Phase 1/Increment 1 behavior when Increment 2 policy is absent.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no disposition or tolerance is widened.
- CLI validate and run, cargo test --workspace, schema drift, and dependency
  direction checks pass.

# Context

Gated on [[TAS-093-enable-same-facility-narrow-user-passing]],
[[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]],
[[TAS-095-complete-lane-transitions-and-safe-aborts]], and
[[TAS-101-measure-close-passes-with-exact-clearance-evidence]]. Owns
scenarios/phase2/inc2 passing fixtures, one focused integration suite, and only
their path entries in docs/benchmark-matrix.md and .json.

Gates my scenario artifacts enter: apps/tangle-cli/tests/migration_regression.rs
enumerates scenarios/**/*.json5; crates/tangle-present/tests/v2_fixtures.rs may
enumerate version-2 fixtures; benchmark-matrix checked-path tests enumerate the
matrix fixture paths.
