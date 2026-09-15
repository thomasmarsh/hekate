---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T05:29:52Z
summary: Check in Increment 2 narrow passing, motor overtaking, and lane-transition fixtures.
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

Depends on [[TAS-093-enable-same-facility-narrow-user-passing]] at context_rev 1.
Depends on [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]] at context_rev 1.
Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Owns scenarios/phase2/inc2 passing fixtures, one focused integration suite, and only
their path entries in docs/benchmark-matrix.md and .json.

Gates my scenario artifacts enter: apps/hekate-cli/tests/migration_regression.rs
enumerates scenarios/**/*.json5; crates/hekate-present/tests/v2_fixtures.rs may
enumerate version-2 fixtures; benchmark-matrix checked-path tests enumerate the
matrix fixture paths.

# Slices

- [[TAS-129-check-in-the-increment-2-passing-fixtures]] Core passing fixtures.
- [[TAS-130-add-unsafe-passing-variants-and-wire-the-benchma]] Unsafe variants and matrix wiring.

# Result

Both slices landed: [[TAS-129-check-in-the-increment-2-passing-fixtures]] (the
three core fixtures and their focused suite, resolved) and
[[TAS-130-add-unsafe-passing-variants-and-wire-the-benchma]] (the unsafe and
prohibited-boundary variants plus the three passing benchmark-matrix entries,
resolved). `scenarios/phase2/inc2/` holds the five fixtures,
`crates/hekate-sim/tests/inc2_passing_fixtures.rs` runs them, and both matrix
representations carry the checked-in paths with no tolerance or disposition
widened.

Preset clause, recorded as delegated and not claimed: the "tests run every
relevant fixture at the benchmark matrix presets" clause of this node's `# Done
when` is NOT executed by the focused suite, which runs the fixtures only at the
pinned Standard (0.05 s) step. Per-preset reproduction is delivered by
[[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] (child of
[[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]), which is
gated on this node; the Phase 1 and Increment 1 preservation half is
[[TAS-134-prove-stream-isolation-and-preserve-the-phase-1]]'s. No preset coverage
is claimed as done anywhere.

Verified: all five gates green on `a56d688` — `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo
test --workspace --all-features` (998 passed, 0 failed, 1 ignored),
`./scripts/check-dependency-direction.sh`, and `tangle check` (194 nodes).
