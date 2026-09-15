---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T05:29:52Z
summary: Add unsafe-passing variants and wire the benchmark matrix.
---

Parent [[TAS-106-check-in-increment-2-passing-fixtures]].

# Outcome

Unsafe, close-clearance, and prohibited-boundary passing variants are checked in
and wired to the benchmark matrix without widening any disposition or tolerance.

# Done when

- Unsafe and prohibited-boundary variants needed by the gate are present and
  assert the documented rejection or violation.
- Tests run every relevant fixture at the matrix presets and preserve Phase 1 and
  Increment 1 behavior when Increment 2 policy is absent.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no disposition or tolerance is widened; schema drift and
  dependency direction pass.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Extends [[TAS-106-check-in-increment-2-passing-fixtures]]; reads the fixture gates
in [[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the unsafe
variants and the matrix wiring; the core fixtures are the sibling slice. The
accepted evidence below is measured at the Standard (0.05 s) preset;
[[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] extends it to the
other matrix presets.

# Result

Two checked-in variants and the three passing matrix entries are wired; no
tolerance or disposition was widened or narrowed (`T-O1`/`T-O2`/`T-O3` and
`T-H1`/`T-H2` are untouched, and the JSON's dispositions and counts are
unchanged).

- `scenarios/phase2/inc2/narrow_passing_unsafe_v2.json5` is the unsafe variant:
the narrow pair and `pass` tactic of `narrow_passing_v2` on a 3.6 m bikeway with
the same two authored bands (0.5 m violation, 2.0 m study) and a 0.4 m lateral
target, so the executed pass is a documented violation. Measured at seed 0 /
0.05 s: one interval (ticks 895..925), minimum clearance 0.4001 m at 44.75 s
below the 0.5 m threshold, violating band 0 recorded, both bands 1.55 s, no
contact or overlap, one anti-overlap cap step; the CLI `close_pass` event carries
`violating_bands: [0]`. It is the violation evidence, not the cell's `T-O2`
reference pass — that remains `narrow_passing_v2`.
- `scenarios/phase2/inc2/motor_lane_change_boundary_v2.json5` is the
prohibited-boundary variant: an adjacent `reverse` band the car may not travel
forward in, so its requested change of lane around a slower bicycle is prevented.
Measured at seed 0 / 0.05 s: the bicycle is admitted at tick 17 and the car at
tick 371, the car follows at 9.72 m, and the run records
`ManeuverReason::BoundaryForbidden` with zero facility handoffs, zero maneuver
transitions, no overtaking interval, no contact, and zero caps.
- `crates/hekate-sim/tests/inc2_passing_fixtures.rs` gains the two tests asserting
those facts; the suite is five tests.
- Matrix wiring only: the three passing slugs moved from the `### 7.3 Planned
fixtures` table into `### 7.2 Checked-in fixtures` in `docs/benchmark-matrix.md`,
and from `fixtures.planned_patterns` into a new
`fixtures.checked_in_increment_2` object in `docs/benchmark-matrix.json`.
`narrow_wrong_way` stays planned: its fixture and entry are the wrong-way
siblings' scope.

Verified: `cargo fmt --all` clean; `cargo clippy -p hekate-sim --all-targets -- -D
warnings` clean; both fixtures CLI-validate and run at seed 0;
`cargo test -p hekate-sim --test inc2_passing_fixtures` 5 passed;
`cargo test -p hekate-cli --test migration_regression` 4 passed;
`cargo test -p hekate-cli --test scenarios` 6 passed;
`./scripts/check-dependency-direction.sh` reports `dependency direction OK`.

Handoff: a preset sweep of these fixtures is not free — the demand arrivals are
drawn per tick, so the Fine and Fast presets admit only the leader within the
same simulated horizon (measured at 0.02 s / 4750 ticks). Per-preset
reproduction needs the demand re-pinned and belongs to the reproducibility
slice.

## Gate evidence

Fresh `gate130` run from the repository root on `a56d688`:

| gate | command | exit | wall s | result |
| --- | --- | --- | --- | --- |
| format | `cargo fmt --all --check` | 0 | 1.1 | clean |
| lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1.5 | clean |
| test | `cargo test --workspace --all-features` | 0 | 179.6 | 998 passed, 0 failed, 1 ignored |
| dependency direction | `./scripts/check-dependency-direction.sh` | 0 | 0.7 | `dependency direction OK` |
| graph | `tangle check` | 0 | 0.4 | `graph check: passed (194 nodes)` |

Preset clause, recorded as delegated and not claimed: the "tests run every
relevant fixture at the matrix presets" clause of this node's `# Done when` is
NOT executed by this node's suite, which runs the five fixtures only at the
pinned Standard (0.05 s) step. Per-preset reproduction is delivered by
[[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] (child of
[[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]), which is
gated on [[TAS-106-check-in-increment-2-passing-fixtures]]. No preset coverage is
claimed here.
