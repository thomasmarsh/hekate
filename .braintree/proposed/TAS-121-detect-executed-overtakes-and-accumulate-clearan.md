---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:43Z
summary: Detect executed overtakes and accumulate clearance-band durations.
next: Detect an actual overtaking interval from exact world body queries and accumulate each configured clearance band.
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

Extends [[TAS-101-measure-close-passes-with-exact-clearance-evidence]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns detection and
band accumulation; do not bump a metric version here.
