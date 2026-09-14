---
context_rev: 1
priority: P1
updated: 2026-09-14T23:19:20Z
summary: Close still-open close-pass intervals at run end
next: Call Simulation::close_open_close_passes before the run reads the tracker so a still-open pass interval closes on termination.
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
`crates/tangle-sim/src/close_pass.rs (ClosePassTracker::close_open)` and
`crates/tangle-sim/src/sim.rs (Simulation::close_open_close_passes)` exist but
are never called by the CLI run loop. Reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]].
