---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Close still-open close-pass intervals at run end
---

Parent [[TAS-122-close-the-close-pass-observation-and-report-clea]].

# Outcome

A close-pass interval that is still alongside when the run ends is closed on
termination under the ordinary lifecycle, so it appears as one observation with
its accumulated minimum, time, relative speed, and band durations.

# Done when

- The run loop closes still-open intervals before reading `ClosePassTracker` (or
  `Simulation::finish` does it), so no overtake in progress at termination is
  dropped.
- The closed-at-run-end observation carries the same minimum, time, relative
  speed, and band evidence as a completed one, with one record per pair.
- Focused tests cover an interval still open at the final tick.

# Context

Extends [[TAS-122-close-the-close-pass-observation-and-report-clea]]. The
contract fixes run end as an interval close boundary.
`crates/hekate-sim/src/close_pass.rs (ClosePassTracker::close_open)` and
`crates/hekate-sim/src/sim.rs (Simulation::close_open_close_passes)` exist but
were never called by the CLI run loop. Reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]].

# Result

The run loop closes the run's still-open intervals before anything reads the
tracker. `apps/hekate-cli/src/trace.rs (canonical_run_captured)` calls
`Simulation::close_open_close_passes` once, after the loop's final tick and
before `RunMetricsRecorder::finish(&sim)`, which is the only product reader of
`ClosePassTracker` (`apps/hekate-cli/src/run_metrics.rs
(RunMetricsRecorder::close_pass_metrics)`); `Simulation::finish` then consumes the
simulation, so no consumer can observe the tracker before the closure. The
closure emits no `ClosePass` event — no tick remains to carry one — so the
canonical trace bytes and every golden hash are unchanged.

A closed-at-run-end interval carries the same evidence as a completed one because
both go through one close path: `ClosePassTracker::close_open` converts each open
`OpenOvertake` with `OpenOvertake::into_observation`, so the observation records
the least swept clearance, its time and relative speed, one duration per
participating band keyed by its stable `ClearanceBandId`, the violating bands, and
the boundary/opposing evidence, exactly once per pair; the close is idempotent and
`observe` never reopens a closed pair.

Tests: `crates/hekate-sim/tests/close_pass.rs
(an_interval_still_open_at_the_final_tick_closes_with_the_completed_evidence)`
stops a run on the last observed tick of an interval and shows the run-end closure
reproduces that interval's observation field for field as its one record for the
pair. `apps/hekate-cli/tests/run_metrics.rs
(a_pass_still_open_at_the_final_tick_is_reported_with_its_evidence)` shows
`canonical_run_captured` reports the interval its hand-closed reference reports,
count for count and family for family; removing the loop's closure fails it (0
observed against 1).

Validation: `cargo test -p hekate-sim --test close_pass`, `cargo test -p
hekate-sim --lib`, `cargo test -p hekate-cli --test run_metrics`, `cargo test -p
hekate-cli --test golden_trace --test increment6_trace --test run_directory`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt
--all --check`, `scripts/check-dependency-direction.sh`, `tangle check`.
