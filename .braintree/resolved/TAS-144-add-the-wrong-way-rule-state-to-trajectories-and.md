---
context_rev: 1
priority: P1
updated: 2026-09-15T00:57:38Z
summary: Add the wrong-way rule state to trajectories and snapshots.
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

# Result

Both columns landed on the route-state sample and in the artifact, under the
version TAS-088 already bumped.

- `crates/tangle-sim/src/snapshot.rs` (`RouteStateSample::perceived_rule`,
  `RouteStateSample::opposing_direction`): the rule state rides the route-state
  sample, so it is absent for a pedestrian and a legacy version-1 path-following
  agent, which carry no route state at all. The spelling this leaf fixed is a
  direction, not a flag: `perceived_rule` is `Option<PermissionEffect>` (the
  applicable `nominal_direction` statement, absent when none binds the pair) and
  `opposing_direction` is `Option<MovementDirection>`.
- `crates/tangle-sim/src/sim.rs` (`Simulation::wrong_way_rule_state`): the state
  is present exactly while the body centre lies inside the object's compiled
  reference extent and its traversal direction is against the object's rule
  direction — the object's authored nominal direction, which an `either` object
  does not have and a facility without a reference path cannot bound. That is the
  predicate `crate::wrong_way::traversal_record` opens and closes the interval
  on, so a row on an opposing traversal carries `Some(Reverse)`/`Some(Forward)`
  and every nominal row, every `either` object, and every legacy row carries
  `null` rather than a default. `opposing_direction` is the direction the body
  travels, i.e. the direction opposing the rule; `perceived_rule` is the most
  recent wrong-way decision's own when that decision selected the opposing option
  on this facility, and otherwise the applicable compiled traversal policy's
  nominal effect — the same precedence the interval record uses.
- `apps/tangle-cli/src/trajectories.rs` (`trajectory_columns`, `TrajectorySample`,
  `TrajectoryColumns`, `labels`, `permission_effect_from_label`,
  `movement_direction_from_label`): the artifact appends `perceived_rule` and
  `opposing_direction` as nullable `Utf8` columns after
  `predicted_min_clearance_m`, and the reader decodes the same labels back, so no
  existing column changes meaning or position and an unknown label is a schema
  error rather than a dropped state. `TRAJECTORY_FORMAT_VERSION` stays `3`: the
  two columns are the rest of the additive union TAS-088 bumped, and no second
  bump is made.

Sparse transitions stay events: the columns are per-row state with no row added,
removed, or duplicated for a transition, and the artifact remains bounded by the
manifest's sampling policy alone. Nothing was added to `crates/tangle-sim/src/wrong_way.rs`
(this leaf's write set excluded it), so the snapshot's presence predicate is a
second spelling of the interval's open boundary; `the_sampled_rule_state_agrees_with_the_interval_boundary`
pins the two together on the entry boundary.

Evidence, all in this worktree: `cargo test -p tangle-sim` green (all suites;
`--lib` 243 passed, `--test route_relative_state` 10 passed with 3 new tests);
`cargo test -p tangle-cli` green (including `trajectories` 12 passed with 2 new,
`replay` 8 passed with 1 new, `run_directory` 11 passed, and `golden_trace`,
which guards the Phase 1 trace hashes — no Phase 1 golden changed);
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
`cargo fmt --all --check` clean; `scripts/check-dependency-direction.sh` OK;
`braintree check` passes.

New focused tests: presence — a rider whose authored reverse traversal is against
its forward-nominal facility carries the perceived `permit` and `Reverse`
(`crates/tangle-sim/tests/route_relative_state.rs`,
`an_opposing_traversal_carries_its_perceived_rule_and_direction`); absence — the
nominal traversal and the `either` object carry `None`
(`a_steering_agent_spawns_with_route_coordinates_from_the_facility_projection`,
`an_either_object_carries_no_rule_state`); agreement with the sparse event
record (`the_sampled_rule_state_agrees_with_the_interval_boundary`); artifact
round-trip and shape (`the_artifact_round_trips_optional_route_state`, the
declared-columns schema test); a real run's rows and the no-extra-row bound
(`apps/tangle-cli/src/trajectories.rs`,
`an_opposing_run_records_its_rule_state_into_the_artifact`); the written
artifact's rule-state rows and a completed run directory's immutability
(`apps/tangle-cli/tests/trajectories.rs`,
`the_artifact_records_rule_state_and_a_completed_run_stays_immutable`,
`a_phase1_run_carries_no_rule_state_in_any_row`); and replay
(`apps/tangle-cli/tests/replay.rs`,
`replay_keeps_the_recorded_rule_state_and_touches_nothing`).

Open reading recorded, not resolved: demand is admitted after the interval pass
observes a tick, so a row can carry the state on the very tick a body is
admitted, one tick before the interval's first boundary event. The two surfaces
agree from the following tick; no required test depends on the one-tick window,
and the helper's doc says the boundary agreement is per tick.
