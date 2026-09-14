---
context_rev: 1
priority: P1
updated: 2026-09-14T18:09:04Z
summary: Detect executed overtakes and accumulate clearance-band durations.
---

Parent [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].

# Outcome

An executed overtake is detected from exact world body queries and each
configured clearance band accumulates its duration independently.

# Done when

- Detection is tied to an actual overtaking interval and exact world body queries,
  not centre distance, lane ID, or an uncalibrated TTC surrogate.
- Multiple configured clearance bands accumulate duration independently and
  retain their stable IDs; bands are definitions, not universal safety claims.
- Analytic and integration tests cover safe, close, multi-band, boundary-crossing,
  and no-overtake encounters.

# Context

Depends on [[TAS-095-complete-lane-transitions-and-safe-aborts]] at context_rev 1.
Depends on [[TAS-100-version-the-maneuver-event-and-trace-surface]] at context_rev 1.
Extends [[TAS-101-measure-close-passes-with-exact-clearance-evidence]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns detection and
band accumulation; do not bump a metric version here.

# Result

Complete. `crates/tangle-sim/src/close_pass.rs (ClosePassTracker)` detects an
executed overtake as an actual overtaking interval on a shared reference (same
path and travel direction, longitudinal footprints overlapping, the faster body
passing the slower) and accumulates each configured clearance band's duration
independently and in declaration order, keyed by `ClearanceBandId`. Detection
uses only exact world body queries (`tick_minimum_clearance_m`, `body_clearance_m`,
`SweptBody`), never centre distance, lane id, or a TTC surrogate; the band
duration is the subset of the interval the tick clearance sat at or below the
band threshold, gated by `CompiledClearanceBand::applies_to`. The tracker is a
pure observer (it cannot change a trajectory, event stream, or golden) wired
into `Simulation::advance_one_tick` and read through
`Simulation::close_pass_tracker`. Tests: nine analytic unit tests (close, safe,
nested multi-band, mid-interval boundary crossing, abreast convoy, head-on,
different paths, mode gate, report-order/determinism) plus four integration
tests (car-overtaking-bicycle every band, determinism, equal-speed no-overtake,
single-mode none).

Validation: `cargo test -p tangle-sim` all targets green (`close_pass` 4), the
coordinator's `cargo test --workspace`, clippy `-D warnings` clean, fmt clean,
and dependency direction OK. No event, metric, trajectory, golden, baseline, or
version changed.
