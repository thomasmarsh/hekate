---
status: resolved
context_rev: 1
priority: P2
updated: 2026-09-15T20:29:45Z
summary: The four_leg_signal_v1 red-queue emergency-cap residual is accepted at a tested 32 m/s² backstop ceiling (−28.2549 m/s² worst step, seed 0, tick 1221, agent 24), scoped to low-speed signalized queue formation.
---

# Outcome

Increment 2's controlled `car_following_v1` benchmark proves bounded
acceleration, braking, and speed under real following, but its sibling
`four_leg_signal_v1` engages the last-resort anti-overlap position cap while a
red-light queue forms. That cap sits outside the IDM clamp and can command a
single step beyond the profile's comfortable braking bound (worst step
−28.3 m/s² in seed 0, reproduced by
`apps/hekate-cli/tests/emergency_cap.rs` and characterized under `# Result`).

Track and resolve that residual so nominal queue formation is either bounded by
the sampled comfortable deceleration or the accepted bound is explicit, rather
than silently relying on the emergency backstop.

# Done when

- A regression test reproduces the `four_leg_signal_v1` seed-0 cap engagement
  and characterizes the worst deceleration step and where in queue formation it
  occurs.
- The queue-formation step is bounded so the emergency cap does not bind under
  nominal red-queue formation, or the residual is explicitly accepted with a
  documented numeric bound and a test that fails if the bound is exceeded.
- The controlled `car_following_v1` benchmark and the walking/scene goldens are
  unchanged.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `tangle check nodes`.

# Context

Area [[IDX-001-hekate]].
Depends on [[TAS-027-phase-1-increment-2-vehicle-flow-controls]] at context_rev 2.

Increment 2 recorded the cap engagement as an accepted limitation of the
controlled benchmark scope. This node owns the follow-up: the residual is real
outside that scope and is worth one bounded fix or an explicit, tested bound.

# Result

The residual is accepted on the node's acceptance branch: the step is
characterized, its mechanism is documented, and a regression test fails if the
accepted bound is exceeded.

- **Reproduction and characterization.** `apps/hekate-cli/tests/emergency_cap.rs`
  runs `four_leg_signal_v1` seed 0 for 4000 ticks at step 0.05 and tracks the
  worst one-step acceleration: −28.2549 m/s² at tick 1221, agent 24, with
  `emergency_cap_steps() == 11` against comfortable_brake 2.256 m/s² and a
  4.99 m body. The test records the complete list of cap-engaged ticks: the 11
  cap steps land on ticks 1137, 1138, 1211, 1212, 1213, 1214, 1220, 1221, 1222,
  1223, and 1224 — one per step, all low-speed queue close-ups behind a stopped
  leader. The test also asserts that the worst step's own tick engaged the cap,
  so the characterized step is the backstop residual rather than an unrelated
  profile step.
- **Mechanism.** The kernel's last-resort position cap
  (`new_speed = min(new_speed, gap / dt)`) is a position clamp with no explicit
  acceleration limit, so a 2 cm gap at 1.79 m/s implies an unbounded one-step
  deceleration. This is a low-speed backstop artifact, not a high-speed
  emergency brake; no kernel code changed.
- **Accepted bound.** `ACCEPTED_CAP_STEP_MPS2: f64 = 32.0` in the test is the
  accepted backstop ceiling, not a comfort target; the test asserts
  `worst_accel >= -ACCEPTED_CAP_STEP_MPS2`, `emergency_cap_steps() >= 1`, and
  that the worst step's tick engaged the cap.
- **Scope.** The same test runs `car_following_v1` seed 0 for 4000 ticks and
  asserts `emergency_cap_steps() == 0`, scoping the residual to signalized
  queue formation.
- **Documentation.** `docs/known_limitations.md` (Motion) records the mechanism,
  the measured worst step, and the accepted 32 m/s² ceiling;
  `scripts/check-harness-inventory.sh` declares the ignored test as a slow
  harness target. The Motion bullet's ceiling is worded as a regression tripwire
  for this fixture and seed, not a property of the kernel, and the position clamp
  is stated to remain unbounded in principle. The same edit removed a stale
  parenthetical from the Control bullet that still called this node proposed and
  unbuilt, a factual correction now that the node is resolved.

Evidence: `cargo test -p hekate-cli --test emergency_cap -- --ignored --nocapture`
passes and prints `four_leg_signal_v1 seed 0: worst step -28.2549 m/s^2 at tick
1221, agent 24 (emergency cap steps 11, cap ticks [1137, 1138, 1211, 1212, 1213,
1214, 1220, 1221, 1222, 1223, 1224])`; `scripts/check-harness-inventory.sh`
exits 0. The controlled `car_following_v1` benchmark and the walking/scene
goldens are unchanged; the wider five-gate run is the coordinator's integration
step.
