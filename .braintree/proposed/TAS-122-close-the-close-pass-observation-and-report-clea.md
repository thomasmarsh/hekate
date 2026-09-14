---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Close the close-pass observation and report clearance metrics.
next: Close one observation per participant pair and report close-pass metrics under a bumped metric definition.
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
