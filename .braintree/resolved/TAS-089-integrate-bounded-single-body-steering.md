---
context_rev: 1
priority: P1
updated: 2026-09-14T02:17:29Z
summary: Integrate bounded steering and lateral motion for single-body wheeled agents.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

A lateral target produces continuous single-body wheeled motion whose speed,
acceleration, braking, steering angle/rate, curvature, and lateral acceleration
stay inside compiled limits, with world pose reconstructed then projected to
route coordinates.

# Done when

- MotionCommand can carry the bounded steering information TAS-083 fixes while
  preserving the existing longitudinal command path.
- Each step integrates without setting d or world position directly to a target,
  reconstructs the physical pose, and projects it back for drift evidence.
- The corridor boundary and existing collision/safety caps constrain the same
  proposed world step; a failed request brakes or holds rather than clipping.
- Straight, curved, forward, and reverse unit fixtures assert per-step limits,
  continuous displacement, projection drift, and no one-tick lane-centre snap.
- Existing car, narrow-longitudinal, pedestrian, and Phase 1 golden tests pass.

# Context

Gated on [[TAS-088-add-route-relative-lateral-agent-state]]. Owns a focused
single-body steering module or the narrow/controller seams, MotionCommand and
sim integration required by it, and focused tests. Do not implement gap
prediction, tactical state transitions, passing selection, or wrong-way choice.

# Result

TAS-089 is complete. A route-relative wheeled agent now integrates a lateral
request as a bounded world step — a capped heading rate and lateral
acceleration — and projects the integrated pose back onto the facility
reference; a request whose step would leave the usable corridor holds rather
than clipping. The existing longitudinal command path is unchanged, so a run
with no lateral request is byte-identical and **no golden was regenerated**.

**Bounded steering primitive** (`crates/tangle-sim/src/steering.rs`, new, a
public module exported from `lib.rs`):

- `bounded_steering_step(geometry, request, envelope, dt) -> Option<SteeringStep>`
  is the pure integration step. It projects the current pose onto the reference,
  derives the desired heading error from the offset target
  (`d_dot = v * sin(theta_error)`), clamps the heading rate to
  `min(heading_rate_max_rad_s, lateral_accel_max_mps2 / max(v, v_floor))`,
  integrates the world heading and the world position from the current pose, and
  projects the integrated position back for `s_m`/`d_m` — the drift check. It
  never writes the target offset to a world position or to `d`. A speed at or
  below zero holds the pose.
- `SteeringLimits` (`heading_rate_max_rad_s`, `lateral_accel_max_mps2`),
  `BoundedSteering` (`limits` + `LateralCorridor`), `SteeringRequest`, and
  `SteeringStep` are the public types; `LATERAL_APPROACH_S = 2.0 s` is the
  documented approach time constant. `LateralCorridor` is the facility's usable
  interval in the reference frame; the step mirrors it into the agent's own
  travel frame exactly as it mirrors `d`, so a reverse step checks the signed
  interval in its own frame.
- The corridor is tested on the integrated pose; a step that would leave it
  returns `None`, which is the "brakes or holds rather than clipping" contract.
  The turning-limit rule `|kappa| <= steering_rate_max_rad_s / v` is the same
  bound as `|heading_rate| <= steering_rate_max_rad_s`, since `kappa = rate / v`.

**Command and integration seams:**

- `crates/tangle-sim/src/stage.rs`: `MotionCommand` gains
  `RouteSteering { heading_rad, speed_mps }` for a route-relative wheeled agent;
  `Longitudinal` and `Steering` are unchanged, so the existing path emits the
  same variant.
- `crates/tangle-sim/src/agent.rs`: `RouteState` carries
  `bounded_steering: Option<BoundedSteering>`, set through the new
  `RouteState::with_bounded_steering`; `project` leaves it `None`, so a
  projection with no envelope stays longitudinal-only exactly as Increment 1.
- `crates/tangle-sim/src/sim.rs`: `route_state_for` now also seeds the envelope
  from the agent's sampled narrow profile (limits) and the facility's
  `usable_lateral_interval` (corridor). `command_motion` turns a fixed
  `RouteState::target_offset_m` into a `RouteSteering` step computed with the
  **already capped** longitudinal speed, so the corridor check and the existing
  leader/stop-line/yield caps constrain the same proposed world step; an
  infeasible step produces a hold (`speed_mps: 0.0`). `advance_physics`
  integrates the `RouteSteering` command in world coordinates
  (`position += from_angle(heading) * speed * dt`), reprojects into the route
  frame, and updates the path progress from the projected `s`. Two small helpers
  (`route_geometry`, `lateral_request`) keep the seam readable. `longitudinal`
  advance is byte-identical.
- `crates/tangle-sim/src/narrow.rs`: `NarrowProfile` gains
  `lateral_accel_max_mps2: Option<f64>`, drawn **after** the existing profile
  draws (so an existing narrow profile is unchanged) and `None` when the
  template's compiled profile declares none; the bicycle and scooter cards list
  it.

**Tests** (`crates/tangle-sim/tests/bounded_steering.rs`, new, 7 tests):

- `a_straight_step_obeys_the_compiled_limits_and_moves_toward_the_target` —
  per-step limits (`|heading_rate| <= 0.9`, `v * |rate| <= 2.0`), heading
  integration `heading = rate * dt`, world displacement `= speed * dt`, and
  movement toward (not to) the target.
- `a_straight_request_never_snaps_to_the_target_in_one_tick` — the first tick
  moves `< 0.1 m` of a `1.0 m` request, the offset advances monotonically by at
  most `speed * dt` per tick, converges, and takes more than 10 ticks.
- `a_curved_reference_advances_progress_continuously_and_projects_back` —
  progress advances continuously on an arc, the reported `(s, d)` is exactly the
  projection of the integrated world pose, and the arc-length advance matches
  the world displacement projected on the local tangent (projection drift
  bounded).
- `a_reverse_step_mirrors_the_offset_frame_and_advances_backward` — reverse
  travel advances toward the path start, a positive agent-frame offset moves to
  negative world `y`, and the offset reaches its target under the same limits.
- `a_request_that_would_leave_the_corridor_holds_instead_of_clipping` — an
  out-of-corridor request is rejected (`None`) after the feasible steps made
  progress, and no returned step exceeds the boundary.
- `the_more_restrictive_of_the_rate_and_lateral_acceleration_limits_binds` —
  at 20 m/s the acceleration bound gives `0.1 rad/s`; at 0.6 m/s the rate bound
  gives `0.9 rad/s`.
- `a_zero_speed_request_holds_the_pose`.

`crates/tangle-sim/src/sim.rs` unit test
`a_bounded_steering_request_integrates_in_world_and_reprojects_without_snapping`
(over a new `BOUNDED_STEERING_V2` bikeway fixture) exercises the whole pipeline:
a rider spawns with route state and an envelope, a fixed `target_offset_m`
produces a bounded step, the route coordinates equal the projection of the
reported world pose every tick, the offset advances continuously without
regressing or jumping past the bounded step, the first tick moves `< 0.1 m`, and
the request makes continuous progress.

**Acceptance** (exact commands and observed results):

- `cargo test -p tangle-sim` — 152 lib + all suites pass, 0 failed; the new
  `bounded_steering` suite has 7 tests.
- `cargo test --workspace` — every suite passes, 0 failed, including
  `tangle-cli` (`golden_trace`, `baseline`, `migration_regression`,
  `increment6_trace`, `trajectories`, `run_directory`), `tangle-present`
  (`scene_golden`, `v2_fixtures`), and the sim suites. **No golden was
  regenerated**, so every checked-in trace hash is byte-identical.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features`
  — clean; `cargo fmt --all -- --check` — clean.
- `braintree check` while TAS-089 was `proposed` — `graph check: passed (154
  nodes)`; after the move, `braintree check --allow-pending-advance TAS-087` —
  `graph check: passed (154 nodes)` ([[TAS-087-continuous-lateral-motion-and-gap-machinery]]'s
  `next` names this now-resolved node, which is the parent advance the
  coordinator owns).

## Remaining scope (recorded, not deferred to a new node)

- Only a capsule whose sampled `NarrowProfile` carries `lateral_accel_max_mps2`
  gets a `BoundedSteering` envelope. A **box** (car) lateral-capable mode — for
  example an Increment 2 `change_lane`/`overtake` car — samples no
  lateral-acceleration or steering-rate value into `VehicleProfile`, so no
  envelope is seeded for it and it stays longitudinal-only. Extending
  `VehicleProfile`/`sample_profile` (`crates/tangle-sim/src/profile.rs`, outside
  this leaf's seams) is required by the car lateral leaves
  ([[TAS-093-enable-same-facility-narrow-user-passing]],
  [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]]).
- The lateral target itself, the usable/predicted corridors, and the maneuver
  lifecycle remain [[TAS-090-predict-maneuver-corridors-and-clearance]] and
  [[TAS-091-resolve-gap-claims-and-maneuver-transitions]]; this leaf integrates a
  target once it is fixed and never selects one, so no shipping scenario
  produces `MotionCommand::RouteSteering` yet.
- `RUN_MANIFEST_VERSION` and the recorded lateral-decision cadence and
  prediction horizon remain open (they are also recorded in
  [[TAS-088-add-route-relative-lateral-agent-state]]).

## Friction (for the session FBK; no FBK node was created)

- Attempted: seed the compiled bounded-steering limits where the contract puts
  them — the profile's `steering_rate_max_rad_s` and `lateral_accel_max_mps2`.
  Friction: `NarrowProfile` carried `steering_rate_max_rad_s` but not
  `lateral_accel_max_mps2`, and `VehicleProfile` carries neither, so the
  per-agent sampled limits the contract names were not reachable for a box mode
  and only partially reachable for a capsule. Improvement: state, in the
  contract or tasking, which sampled profile type must carry the Increment 2
  lateral limits, or land them in the shared `profile.rs` sampler before the
  leaves that read them.
- Attempted: express the corridor type directly. Friction:
  `tangle_model::UsableLateralInterval` has private fields and no public
  constructor, so a `tangle-sim` integration test cannot build one to drive the
  primitive; the module maps it into a plain `LateralCorridor`. Improvement:
  expose a public constructor on `UsableLateralInterval`, or state that the
  consuming crate owns its own corridor value type.
- Attempted: hold the Done-when bullet "the corridor boundary and existing
  collision/safety caps constrain the same proposed world step" as an explicit
  two-version step object. Friction: the existing collision caps are speed caps
  and the corridor is a position bound, so they meet only through the single
  capped speed the step integrates; there is no separate proposed-step record to
  constrain. Improvement: the contract could say the shared bound is the capped
  speed, or name a proposed-step record both a corridor and a cap can reject.
