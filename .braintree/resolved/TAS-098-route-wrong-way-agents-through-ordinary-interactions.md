---
context_rev: 1
priority: P1
updated: 2026-09-14T16:29:43Z
summary: Route wrong-way agents through ordinary steering, collision, and lifecycle paths.
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

Depends on [[TAS-097-make-contextual-wrong-way-decisions-reproducible]] at context_rev 1.
Owns wrong-way route integration and focused tests. No scripted trajectory,
collision bypass, public event payload, or metric aggregation belongs here.

# Result

Complete. Both children resolved:
[[TAS-117-transition-route-and-direction-for-wrong-way-ent]] landed the
pose-preserving wrong-way entry plus opposing progress/completion in both
authored reference directions, with its ordering child
[[TAS-138-order-a-turned-riders-leaders-and-encounters]] proving leader and
record ordering follows the actual travel direction;
[[TAS-118-reject-an-occupied-opposing-corridor-and-keep-th]] proved the occupied
corridor is bounded by the ordinary leader/collision machinery (fixing
`Simulation::nearest_leader` to carry an oncoming body's speed along the travel
axis) and preserved identity, profile, metrics dimensions, event order, and
lifecycle. No flag disables collision, clearance, safety, or leader queries.
Validation: `cargo test -p tangle-sim` all targets green (12 `wrong_way`), the
coordinator's `cargo test --workspace`, clippy `-D warnings` clean, fmt clean,
and dependency direction OK.

# Slices

- [[TAS-117-transition-route-and-direction-for-wrong-way-ent]] Route and direction transition.
- [[TAS-118-reject-an-occupied-opposing-corridor-and-keep-th]] Occupied-corridor rejection and lifecycle.
