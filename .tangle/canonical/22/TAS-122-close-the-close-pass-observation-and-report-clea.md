---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Close the close-pass observation and report clearance metrics.
---

Parent [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].

# Outcome

Each executed overtake yields at most one deterministic observation per
participant pair with its exact minimum, relative speed, and band evidence, and
the close-pass metrics report it by dimension.

# Done when

- The minimum is derived from sampled or swept clearance, records its time and
  relative speed, and is symmetric in pair query order while retaining actor and
  passed-user roles.
- One observation or event closes on pass completion or termination; metrics
  report attempts, aborts, completions, violations, and clearance distributions by
  mode pair, movement, facility, and applicability under a bumped metric
  definition.
- Tests cover aborted and opposing-facility encounters.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Extends [[TAS-101-measure-close-passes-with-exact-clearance-evidence]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the observation
lifecycle and the metric definition bump; detection is the sibling slice.

# Result

Primary deliverable landed: the close-pass observation closure and the
`Event::ClosePass` record. `crates/hekate-sim/src/close_pass.rs (ClosePassTracker)`
now records each completed overtaking interval's exact minimum clearance, the tick
it occurred, the relative speed at that minimum, side, facility, per-band
durations keyed by `ClearanceBandId`, violating bands, and the
crossed-boundary/entered-opposing evidence, closing one observation per pair on
completion or termination. `crates/hekate-sim/src/event.rs` adds the `ClosePass`
payload under the existing `EVENT_VERSION = 3` (appended after
`OpposingTraversal`, existing order values unchanged), emitted from
`Simulation::advance_one_tick`, serialized by `apps/hekate-cli/src/trace.rs
(EventRecord)`, and matched by every consumer (`apps/hekate-tui`,
`hekate-viewer`, `crates/hekate-present`, `apps/hekate-cli/src/run_metrics.rs`).
Tests: 17 unit + 6 integration in the close-pass suites, including aborted and
opposing-facility encounters.

Remaining (split into three small children): the metric-definition v3 node and
`METRIC_DEFINITION_VERSION` increment; the close-pass family accumulation in
`RunMetrics`; and the run-artifact/aggregate/compare surface. `METRIC_DEFINITION_VERSION`
is still 2 — the metric half of this Done when is not yet met.

Validation: `cargo test --workspace` (85 targets, 0 failed), clippy `-D warnings`
clean, fmt clean, dependency direction OK, `tangle check`.

Roll-up: complete. That `Remaining` list was the plan at the first landing; the
children it named are now all resolved, plus two the plan did not yet name. [[TAS-139-define-close-pass-metric-v3]]
authored [[DEF-006-metric-definition-v3]] and raised `METRIC_DEFINITION_VERSION` to
3 (`apps/hekate-cli/src/run_metrics.rs`). [[TAS-140-accumulate-close-pass-metric-families]]
accrued the overtaking and close-pass families in `RunMetricsRecorder` from the
edge-triggered `Event::Maneuver` records and the tracker's closed observations, with
explicit applicability. [[TAS-141-surface-close-pass-metric-families]] carried the
block into `metrics.json` and [[TAS-143-aggregate-and-compare-close-pass-families]]
carried it into aggregate and compare without widening an existing cell.
[[TAS-142-close-still-open-close-pass-intervals-at-run-end]] supplied the last
piece of the termination half: `apps/hekate-cli/src/trace.rs
(canonical_run_captured)` now closes the run's still-open intervals before the
metric capture, so an interval still alongside at the final tick closes as one
observation with the same evidence a completed one carries.

Every `# Done when` bullet holds against that evidence: the swept minimum with its
time and relative speed, symmetric in pair query order while retaining the actor
and passed-body roles (`crates/hekate-sim/src/close_pass.rs`, its 17 unit tests and
the close-pass integration suite); one closure per pair on completion or
termination with the v3 families by mode pair, movement, facility, and
applicability; and aborted and opposing-facility encounter tests. No child is left
open, so the node resolves.
