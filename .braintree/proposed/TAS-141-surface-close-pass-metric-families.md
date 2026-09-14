---
context_rev: 1
updated: 2026-09-14T22:46:00Z
summary: Surface close-pass metric families in run artifacts and comparison
next: Serialize and preserve the close-pass metric families through run artifacts, aggregate, and compare.
---

Parent [[TAS-122-close-the-close-pass-observation-and-report-clea]].

# Outcome

The close-pass metric families survive the immutable run artifact and appear in
aggregate and compare under metric definition v3.

# Done when

- The run-directory metric artifact serializes the close-pass families and
  dimensions and they round-trip through replay.
- Aggregate and compare preserve the new families and dimensions without
  widening an existing matrix cell or disposition.
- Focused run-directory, aggregate, and compare tests pass.

# Context

Extends [[TAS-122-close-the-close-pass-observation-and-report-clea]] and depends
on the sibling accumulation node. Reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. No new metric
semantics.
