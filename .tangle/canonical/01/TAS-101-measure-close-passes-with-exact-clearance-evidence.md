---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Measure close passes with exact clearance, relative speed, duration, and rule evidence.
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

# Result

Complete: both slices resolved and every `# Done when` bullet holds against their
evidence.

Detection and band accumulation: `crates/hekate-sim/src/close_pass.rs
(ClosePassTracker)` detects an overtaking interval from the pair's longitudinal
footprints on one shared reference and measures clearance with the exact swept
world-body query `crate::metrics::tick_minimum_clearance_m`, not centre distance,
lane id, or a time-to-collision surrogate; each configured
`CompiledClearanceBand` accumulates its own duration under its stable
`ClearanceBandId` (17 unit tests, including `nested_bands_accumulate_independently_and_monotonically`, the
mode gate, and the symmetry of the minimum in pair query order).

Observation closure and metrics: `ClosePassTracker::close_open` closes one
observation per pair on completion or termination — including the run-end closure
[[TAS-142-close-still-open-close-pass-intervals-at-run-end]] wired into
`apps/hekate-cli/src/trace.rs (canonical_run_captured)` — and
[[TAS-139-define-close-pass-metric-v3]] raised `METRIC_DEFINITION_VERSION` to 3 with
the families [[TAS-140-accumulate-close-pass-metric-families]] accrues by mode pair,
movement, facility, and applicability, surfaced by
[[TAS-141-surface-close-pass-metric-families]] and carried by
[[TAS-143-aggregate-and-compare-close-pass-families]]. Encounter coverage: the
close-pass suites include safe, close, multi-band, boundary-crossing
(`a_boundary_crossing_is_evidence_on_the_observation`), opposing-facility
(`a_body_on_the_opposing_traversal_records_its_evidence`), aborted
(`an_aborted_pass_closes_exactly_one_observation`), and no-overtake
(`an_abreast_convoy_is_not_an_overtake`, `equal_speeds_yield_no_overtake`) cases.
