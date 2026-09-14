---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:12Z
summary: Measure close passes with exact clearance, relative speed, duration, and rule evidence.
next: [[TAS-121-detect-executed-overtakes-and-accumulate-clearan]]
---

Parent [[TAS-099-increment-2-events-metrics-and-output]].

# Outcome

Each executed overtake produces at most one deterministic observation per
participant pair containing the exact minimum body clearance, relative speed,
band durations, side, facility, and boundary or opposing-facility evidence.

# Done when

- Detection is tied to an actual overtaking interval and exact world body
  queries, not centre distance, lane ID, or an uncalibrated TTC surrogate.
- Multiple configured clearance bands accumulate duration independently and
  retain their stable IDs; bands are definitions, not universal safety claims.
- The minimum is derived from sampled/swept clearance, records its time and
  relative speed, and is symmetric in pair query order while retaining actor
  and passed-user roles.
- One observation/event closes on pass completion or termination; metrics
  report attempts, aborts, completions, violations, and clearance distributions
  by mode pair, movement, facility, and applicability under a bumped metric
  definition.
- Analytic unit tests and integration tests cover safe, close, multi-band,
  boundary-crossing, opposing-facility, aborted, and no-overtake encounters.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]] and
[[TAS-100-version-the-maneuver-event-and-trace-surface]]. Owns a focused
close-pass tracker, required Event payload, metrics definitions/aggregation,
run artifact serialization, and tests. Reuse tick_minimum_clearance_m and swept
queries; do not implement tactical eligibility.

# Slices

- [[TAS-121-detect-executed-overtakes-and-accumulate-clearan]] Overtake detection and band durations.
- [[TAS-122-close-the-close-pass-observation-and-report-clea]] Observation closure and clearance metrics.
