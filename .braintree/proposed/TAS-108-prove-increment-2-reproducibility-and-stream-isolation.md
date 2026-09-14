---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:12Z
summary: Prove Increment 2 trace reproducibility and maneuver-stream isolation.
next: [[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]]
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Every Increment 2 fixture reproduces its decisions, claims, events, metrics, and
trace hash, and adding an unrelated agent or demand stream cannot perturb
another agent's maneuver or wrong-way draws.

# Done when

- Each passing and wrong-way fixture repeats identically at fixed seeds and
  required presets; a declared seed bank reproduces per-seed batch artifacts
  and replay verifies their event streams.
- Golden traces cover every maneuver transition and wrong-way interval
  lifecycle, including stable simultaneous-claim ordering.
- Stream-isolation tests add unrelated car, narrow demand, and reversed
  declaration order without changing owned maneuver draws or unaffected agent
  traces.
- A falsification probe demonstrates the suite catches draw-order coupling or
  an unkeyed maneuver choice.
- Existing demand, profile, compliance, and perception streams and Phase 1
  baseline artifacts remain unchanged.

# Context

Gated on [[TAS-106-check-in-increment-2-passing-fixtures]] and
[[TAS-107-check-in-contextual-wrong-way-fixtures]]. Owns focused CLI and
tangle-sim determinism suites, an Increment 2 seed bank, and Increment 2-only
goldens. Do not change production behavior except for a coupling defect proved
by the tests.

Gates my seed-bank and golden artifacts enter: scenario fixture enumeration,
CLI golden/fixture discovery, and cargo test --workspace.

# Slices

- [[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] Fixture reproducibility and goldens.
- [[TAS-134-prove-stream-isolation-and-preserve-the-phase-1]] Stream isolation and Phase 1 baseline.
