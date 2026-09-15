---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Split the agent update into four explicit controller-stage interfaces.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

`hekate-sim` exposes the four `PHASE_2_PLAN.md` controller stages as explicit
interfaces — relevant-world query, tactical choice, motion control, and physical
advance — with the existing vehicle and pedestrian models routed through them
and unchanged behavior.

# Done when

- Each stage has a named interface with immutable observations and a returned command.
- The Phase 1 vehicle and pedestrian updates use the stages with no trace-hash change.
- The controller seam stub-swap test still proves the kernel reaches models only through the interface.

# Context

No gate; independent of the component stream of [[TAS-058-agent-components-and-controller-stages]].

# Result

`hekate-sim` now exposes the four `PHASE_2_PLAN.md` controller stages as named
interfaces with immutable observations and returned commands, and the Phase 1
vehicle and pedestrian updates run through them with byte-identical behavior.
The three `Done when` criteria hold.

- **Each stage is a named interface with an immutable observation and a
  returned command.** `crates/hekate-sim/src/stage.rs` (new) defines
  `RelevantWorldQuery`, `TacticalChoice`, `MotionControl`, and
  `PhysicalAdvance`. Stage 1 takes `&mut self, index` and returns the crate's
  immutable `Observation` (`VehicleObservation` / `PedestrianObservation`),
  whose fields the kernel fills from the constraint set, leader, upcoming
  controls, route target, and nearby bodies. Stage 2 takes `&self` plus
  `&Observation` and returns a `Tactic` record carrying `reason`, `target`,
  `commitment`, `abort`, and the kernel-clock `started_at`. Stage 3 takes
  `&mut self` plus `&Observation`, `&Tactic`, and `dt`, and returns a bounded
  `MotionCommand` (`Idle` / `Longitudinal` / `Steering`). Stage 4 consumes the
  `Observation` and `&MotionCommand`, integrating the implied pose and emitting
  the step's diagnostics. `crates/hekate-sim/src/sim.rs` implements all four on
  `Simulation` and drives them in order from `Simulation::step_agent`
  (`query_world`, then `choose_tactic`, then `command_motion`, then
  `advance_physics`). Every interaction decision stays in the kernel: leader,
  stop-line, crossing-yield, and conflict selection, the safety position caps,
  and the emergency counters live in the stage implementations, not in a model.
- **The Phase 1 updates use the stages with no trace-hash change.** Routing the
  existing vehicle and pedestrian updates through the stages is behavior
  preserving. The three canonical trace hashes are unchanged
  (`tests/golden/*.trace.jsonl` still match their recorded `.sha256`:
  `walking_guide_v1` `60bd030f…`, `four_leg_pedestrian_ew_priority_v1`
  `a9666e0a…`, `four_leg_pedestrian_ns_priority_v1` `35221e91…`), and no golden
  trace, event stream, or baseline file is modified.
- **The controller seam stub-swap test still proves the interface.**
  `crates/hekate-sim/src/controller.rs`'s
  `the_kernel_reaches_both_modes_only_through_the_interfaces` still passes
  unchanged: swapping in a fixed-brake vehicle model and a standstill
  pedestrian model changes both modes' motion, proving the kernel reaches each
  mode's model only through the stage interfaces.

Focused per-stage tests in `crates/hekate-sim/src/sim.rs` (`#[cfg(test)]`) call
one interface at a time: `the_query_and_tactic_stages_select_the_vehicle_leader`
(stage 1 vehicle observation + stage 2 `Follow` tactic with target and lifecycle
fields), `the_tactic_stage_commits_the_stop_line_maneuver` (stage 2's
`Committed` stop-line tactic + stage 3's stop-line-bounded command),
`the_motion_and_advance_stages_command_and_integrate_the_vehicle` (stage 3
profile-bounded command + stage 4 integration),
`the_motion_stage_reaches_the_vehicle_model_through_the_interface` (stage 3
reaches the replaceable model), and
`the_four_stages_drive_a_demand_pedestrian` (all four stages on a live demand
pedestrian).

Evidence: `cargo test -p hekate-sim` passes (138 unit + integration tests,
including the stub-swap test and the per-stage tests); `cargo test --workspace`
passes with all golden traces and baselines unchanged;
`scripts/check-dependency-direction.sh` reports `dependency direction OK`;
`tangle check` passes while the node is still `proposed`.

Handoff: after this node moves to `resolved`, `tangle check` reports the
expected `next-resolved-node` diagnostic for
[[TAS-058-agent-components-and-controller-stages]] because its `next` names this
resolved child; TAS-058 stays open and is owned by the coordinator.
