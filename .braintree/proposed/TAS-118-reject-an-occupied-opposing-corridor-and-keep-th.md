---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:43Z
summary: Reject an occupied opposing corridor and keep the ordinary lifecycle.
next: Reject an occupied opposing corridor through ordinary feasibility and keep the normal identity, events, and metrics.
---

Parent [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].

# Outcome

An occupied opposing corridor is handled by the ordinary feasibility and gap
machinery, and the wrong-way agent keeps its normal identity, events, metrics, and
lifecycle.

# Done when

- An occupied opposing corridor yields an infeasible claim or a bounded wait,
  brake, or abort; no flag disables collision, clearance, safety, or leader
  queries.
- The agent retains its normal stable ID, mode template, profile, metrics
  dimensions, event order, and lifecycle.
- Focused tests cover occupied rejection, head-on following and yield, collision
  visibility, and disconnected rejection.

# Context

Extends [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; reads
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the occupancy
rejection and the ordinary-path preservation.
