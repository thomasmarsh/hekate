---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T04:55:53Z
summary: Check in the Increment 2 passing fixtures.
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

# Result

The three matrix-named core fixtures are checked in under
`scenarios/phase2/inc2/` (`narrow_passing_v2.json5`,
`motor_passing_narrow_v2.json5`, `motor_lane_change_v2.json5`), and
`crates/hekate-sim/tests/inc2_passing_fixtures.rs` runs each one through the
kernel at seed 0 and asserts the maneuver lifecycle (attempt, commit,
completion, no abort), finite lateral samples and no teleport, the sampled
profile and world-boundary limits, exactly one overtaking interval carrying the
authored clearance bands in declaration order, route completion of both
participants, and no contact or overlap. `hekate-cli validate` and `run --seed 0
--ticks 900` pass for all three, and `hekate-cli`'s `migration_regression` and
`scenarios` suites enumerate the new directory and pass.

Measured kernel facts the fixtures pin (no tolerance or disposition widened):

- one completed within-facility pass engages the anti-overlap position cap for
exactly one step, on the *passed* body, because the leader scan selects leaders
by path and progress; the landed `narrow_passing.rs` model fixture reports the
same one-per-pass relationship, so the suite asserts one cap step per executed
pass (and zero for the change of lane, whose crossing hands the passer off
before it draws ahead of the leader) and the maneuver clears no corridor
through the cap;
- `Event::Entry`/`Event::Exit` are crossing/conflict-region edges, so route
completion is read from `Spawned` followed by `Despawned { reason: ExitedPath }`;
- a cross-facility change of lane is a tactical leaf's request, so
`motor_lane_change_v2` needs its suite-side request (the same seam
`lane_transitions.rs` uses) and its pass is measured on the carriageway before
the handoff, which also gives the observation its `close_pass` boundary
evidence.

Remaining scope (TAS-130): the unsafe/prohibited variants and the
benchmark-matrix path wiring.

Gate evidence (fresh `gate129` run from the repository root on
`686cb01` + this session's clippy fix):

| gate | command | exit | wall s | result |
| --- | --- | --- | --- | --- |
| format | `cargo fmt --all --check` | 0 | 1.0 | clean |
| lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1.0 | clean |
| test | `cargo test --workspace --all-features` | 0 | 345.0 | 996 passed, 0 failed, 1 ignored |
| dependency direction | `./scripts/check-dependency-direction.sh` | 0 | 1.0 | `dependency direction OK` |
| graph | `tangle check` | 0 | 1.0 | `graph check: passed (194 nodes)` |

Gate 2 first ran red at 11 s with one `clippy::collapsible_match`
(`-D warnings`) in the new `crates/hekate-sim/tests/inc2_passing_fixtures.rs`
(`Event::Collision { .. } => { if *contacting { .. } }`); the fresh worker
collapsed it to the `contacting: true` arm in the same in-scope test file and
re-ran fmt (clean) and clippy (clean). No sibling seam, public schema, or event
surface changed.
