---
context_rev: 1
priority: P1
updated: 2026-09-14T00:27:04Z
summary: Route wrong-way agents through ordinary steering, collision, and lifecycle paths.
next: Integrate opposing traversal selection with ordinary route state, gap claims, and completion.
---

Parent [[TAS-096-contextual-wrong-way-travel]].

# Outcome

After a contextual decision, a bicycle or scooter enters and completes the
physically connected opposing traversal through the normal route, steering,
gap, collision, yielding, safety, and despawn machinery.

# Done when

- Route state changes direction/facility through the same connected-transition
  contract as an ordinary lateral maneuver, with world pose continuously
  authoritative.
- An occupied opposing corridor yields an infeasible claim or bounded wait,
  brake, or abort; no flag disables collision, clearance, safety, or leader
  queries.
- Opposing leaders and encounters are ordered in actual travel direction, and
  route progress/completion work in both authored reference directions.
- The agent retains its normal stable ID, mode template, profile, metrics
  dimensions, event order, and lifecycle.
- Focused tests cover clear opposing entry, occupied rejection, head-on
  following/yield response, collision visibility, route completion, and
  disconnected rejection.

# Context

Gated on [[TAS-097-make-contextual-wrong-way-decisions-reproducible]]. Owns
wrong-way route integration and focused tests. No scripted trajectory,
collision bypass, public event payload, or metric aggregation belongs here.
