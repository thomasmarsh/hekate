---
context_rev: 1
priority: P1
updated: 2026-09-14T13:33:34Z
summary: Reproduce every Increment 2 fixture and golden its transitions.
next: Reproduce each fixture at fixed seeds and presets and golden every maneuver transition and wrong-way interval lifecycle.
---

Parent [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]].

# Outcome

Every Increment 2 fixture reproduces its decisions, claims, events, metrics, and
trace hash at fixed seeds and required presets.

# Done when

- Each passing and wrong-way fixture repeats identically at fixed seeds and
  required presets; a declared seed bank reproduces per-seed batch artifacts and
  replay verifies their event streams.
- Golden traces cover every maneuver transition and wrong-way interval lifecycle,
  including stable simultaneous-claim ordering.

# Context

Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the
reproducibility and goldens; stream isolation is the sibling slice.
