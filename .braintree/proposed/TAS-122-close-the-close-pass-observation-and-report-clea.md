---
context_rev: 1
priority: P1
updated: 2026-09-14T22:52:14Z
summary: Close the close-pass observation and report clearance metrics.
next: "[[TAS-140-accumulate-close-pass-metric-families]]"
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
`Event::ClosePass` record. `crates/tangle-sim/src/close_pass.rs (ClosePassTracker)`
now records each completed overtaking interval's exact minimum clearance, the tick
it occurred, the relative speed at that minimum, side, facility, per-band
durations keyed by `ClearanceBandId`, violating bands, and the
crossed-boundary/entered-opposing evidence, closing one observation per pair on
completion or termination. `crates/tangle-sim/src/event.rs` adds the `ClosePass`
payload under the existing `EVENT_VERSION = 3` (appended after
`OpposingTraversal`, existing order values unchanged), emitted from
`Simulation::advance_one_tick`, serialized by `apps/tangle-cli/src/trace.rs
(EventRecord)`, and matched by every consumer (`apps/tangle-tui`,
`tangle-viewer`, `crates/tangle-present`, `apps/tangle-cli/src/run_metrics.rs`).
Tests: 17 unit + 6 integration in the close-pass suites, including aborted and
opposing-facility encounters.

Remaining (split into three small children): the metric-definition v3 node and
`METRIC_DEFINITION_VERSION` increment; the close-pass family accumulation in
`RunMetrics`; and the run-artifact/aggregate/compare surface. `METRIC_DEFINITION_VERSION`
is still 2 — the metric half of this Done when is not yet met.

Validation: `cargo test --workspace` (85 targets, 0 failed), clippy `-D warnings`
clean, fmt clean, dependency direction OK, `braintree check`.
