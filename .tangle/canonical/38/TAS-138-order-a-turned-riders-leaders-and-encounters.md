---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Order a turned rider's leaders and encounters by its travel direction.
---

Parent [[TAS-117-transition-route-and-direction-for-wrong-way-ent]].

# Outcome

A rider that has entered the opposing traversal has its leaders and its
encounters ordered by the direction it actually travels, in both authored
reference directions, so a consumer reading the run sees the same order the
rider experiences.

# Done when

- Focused tests show a turned rider's leader query follows its actual travel
  direction rather than the authored nominal one, in both reference directions.
- The within-tick ordering of the turned rider's own records follows the same
  travel direction, so no record is ordered by the nominal direction it no
  longer travels.

# Context

Extends [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; the
entry, opposing progress, and route completion landed in
[[TAS-117-transition-route-and-direction-for-wrong-way-ent]]. Reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the ordering
evidence for the turned rider's leaders and encounters; the head-on response
(rejection, bounded wait, brake, abort) stays with
[[TAS-118-reject-an-occupied-opposing-corridor-and-keep-th]], and the encounter
event payloads and metric aggregation stay with
[[TAS-099-increment-2-events-metrics-and-output]].

# Result

Both criteria hold with focused tests only: no production fix was needed, and no
event payload, metric, trajectory, version, golden, or baseline changed.

Verified seams:

- `crates/hekate-sim/src/sim.rs (Simulation::nearest_leader)`: own and candidate
  progress are `self.agents.direction[index] * distance_m`, and
  `Simulation::enter_opposing_traversal` writes that sign, so a turned rider's
  leader query runs in the signed travel frame. It never reads
  `AgentStore.heading_rad` (the reference-tangent heading, which the entry
  deliberately leaves unchanged) nor the facility's authored nominal direction.
  The only other progress-keyed orderings in the crate are already travel-signed
  (`crate::pedestrian::route_progress_m`) or agent-keyed
  (`sim.rs`'s within-tick sort and `stage.rs`'s claim batch), so no record is
  ordered by the nominal direction.
- `crates/hekate-sim/src/event.rs (Event::order_key)`: the within-tick key reads
  only the agent id, `EventKind::order`, the variant's own key (partner agent,
  region, path, or sub-kind) and the edge flag. No distance, progress, or
  direction term exists, and the one record that carries an arc length
  (`Event::Spawned { distance_m }`) keys identically at both reference ends.

Evidence, all from this worktree at base 3e39ec5:

- `cargo test -p hekate-sim --test wrong_way` 6 passed (the four TAS-117 entry
  and completion tests plus
  `a_turned_rider_is_bounded_by_the_queue_behind_it_in_both_reference_directions`
  and `the_turned_riders_own_records_keep_one_order_in_both_reference_directions`).
- The queue fixtures hold a rider queue at a pedestrian-occupied crossing (a
  `yield` rule, so no signal and no compliance is involved), then turn the front
  rider: the vehicle queued behind it is ahead of the turned rider in its actual
  travel direction and behind it in the authored nominal one. The turned rider
  carries no yield, no signal decision, and no lateral maneuver afterwards, and
  its own desired speed is 6 m/s, so the leader its actual direction names is
  the only thing that can hold it: it stays at rest (max 0.0 m/s) for 200 steps
  in both fixtures (minimum bumper gap 1.885 m in both), records no collision,
  and takes no emergency cap step.
- Falsification probes run in the worktree and reverted: with
  `Simulation::nearest_leader` reading the authored forward frame
  (`direction = 1.0`) the forward-reference turned rider collides twice with the
  body ahead of it, and with `direction = -1.0` the reverse-reference one does
  the same, so each test fails on a nominal-frame leader query rather than
  passing vacuously.
- The ordering test asserts the documented order for every tick in both
  fixtures, and that in the tick carrying records about the turned rider and its
  leader, the turned rider's own record comes first although that body is the
  backmarker in the authored nominal direction — an order that nominal progress
  cannot produce. It also asserts the turned rider's own records (spawn, yield
  begin, queue edges, yield end) order identically in both reference directions
  while `Spawned.distance_m` reports 0 m in one and 220 m in the other.
- `cargo test -p hekate-sim` all suites green (no Phase 1 / Increment 1 /
  TAS-113..117 regression); `cargo clippy --workspace --all-targets
  --all-features -- -D warnings` clean; `cargo fmt --all --check` clean;
  `scripts/check-dependency-direction.sh` OK.

Files: `crates/hekate-sim/tests/wrong_way.rs`.

Pending parent advance: resolving this node leaves
[[TAS-117-transition-route-and-direction-for-wrong-way-ent]] routing to a
resolved child, so its `next` must advance to
[[TAS-118-reject-an-occupied-opposing-corridor-and-keep-th]]; TAS-117 is outside
this slice's write set and is reported rather than edited.
