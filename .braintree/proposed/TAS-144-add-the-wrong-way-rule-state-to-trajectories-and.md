---
context_rev: 1
priority: P1
updated: 2026-09-15T00:14:35Z
summary: Add the wrong-way rule state to trajectories and snapshots.
next: Add the optional `perceived_rule` and `opposing_direction` columns to the full snapshot and the sampled trajectory artifact under the trajectory version TAS-088 already bumped.
---

Parent [[TAS-123-record-the-wrong-way-interval-and-rule-state-tra]].

# Outcome

Sampled trajectories and full snapshots expose the optional wrong-way rule
state, so a consumer reads why a body is on an opposing traversal without a
second lookup and without a per-step event.

# Done when

- The full snapshot's route-state sample and the sampled-trajectory artifact gain
  the optional `perceived_rule` and `opposing_direction` columns, present only
  for an agent that has the state and absent rather than defaulted otherwise.
- The columns land under `TRAJECTORY_FORMAT_VERSION` 3, the single bump
  [[TAS-088-add-route-relative-lateral-agent-state]] landed for the whole
  additive column union: no second bump, and no existing column changes meaning
  or position.
- Sparse rule-state transitions stay event records: no sampled row is added or
  duplicated for a state change, and the artifact stays subject to the
  manifest's sampling policy.
- Focused tests cover the columns' presence and absence, the artifact
  round-trip, a completed run directory's immutability, and replay.

# Context

Extends [[TAS-123-record-the-wrong-way-interval-and-rule-state-tra]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. The interval and
its `Event::OpposingTraversal` boundaries are the parent's landed half.

The snapshot route-state sample lives in `crates/tangle-sim/src/snapshot.rs`,
which the parent's write set excluded, so choosing that seam — or a read
surface on `crates/tangle-sim/src/sim.rs` that the recorder reads instead — is
this leaf's first decision. The artifact's own `perceived_rule` and
`opposing_direction` spellings and types (whether `opposing_direction` is a
direction or a flag) are fixed here from the contract's column names.

Owns the snapshot and trajectory rule-state columns only; do not add the
aggregate wrong-way metrics of
[[TAS-124-add-disaggregated-wrong-way-metrics-and-version]] or the scene
overlays of [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]].
