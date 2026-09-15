---
context_rev: 1
status: resolved
updated: 2026-09-15T19:42:19Z
summary: Drive the lane_transitions constraint rules from an in-crate scripted fixture.
---

Parent [[tas-5syjgmtrwvgqe66yn9w1j5v0p4-make-the-expensive-behavior-tests-fast-and]].

# Outcome

The four `lane_transitions` leader/follower constraint rules run from a crate-internal two-band scripted fixture in the `crates/hekate-sim/src/sim.rs` test module next to `push_rider`, under a second per rule, with the integration constraint tests replaced and their thresholds re-derived on the scripted placement.

# Done when

- A two-band scripted fixture in the `#[cfg(test)]` module next to `push_rider` places the rider and its companion directly and drives each constraint rule in well under a second.
- The four rules still hold: a body ahead on the rider's own band bounds the committed approach but the rider still crosses; a body ahead on the destination band bounds the committed approach before handoff while the no-body control stays at free flow; a body closing from behind on the destination band holds the crossing and aborts on lost clearance; and a body behind on the rider's own band stays its follower through the crossing without overlap or a hold.
- The integration constraint tests and their constraint-only helpers are removed from `crates/hekate-sim/tests/lane_transitions.rs`, and every threshold is re-derived on the scripted placement.
- `cargo nextest run -p hekate-sim` passes and the fixture rules are sub-second.
- `tangle check` passes.

# Context

Informed by [[tho-55ch2x2wgytsh1xew9pbjf5py3-classify-default-suite-test-cost-and-design-the]].

The integration constraint family is expensive because it authors a streamed scenario and drives 600 ticks per rule. The scripted fixture must place bodies deterministically and stop as soon as the rule is observed.

# Result

The four rules now run from `constraint_bands_v2()` — an in-crate two-band document — and the
streamed constraint family is gone from `crates/hekate-sim/tests/lane_transitions.rs`.

**Fixture.** `guide_a` on band `a` (`y = -1.5..1.5`), `pass_lane` at `y = -2.0`, `guide_b` on
band `b` (`y = 1.5..4.5`), one `a_beside_b` adjacency, one `through` movement, the maneuver
fixture's rider mode with `lateral { target_clearance_m: 0.75, horizon_s: 6.0 }`, `commit
{ min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 }`, and one population demand with
`count: 0`, so only a test's own placements move. `push_body` was generalised out of `push_rider`
(path, distance, offset, speed, and whether the body carries the bounded-steering envelope a
maneuver candidate needs), and `push_rider` is a thin wrapper, so the existing maneuver unit tests
are byte-identical in behaviour. Each rule reaches the kernel through the module's existing
private seams (`route_state_for`, `request_lateral_maneuver`, `nearest_leader`, `sim.agents`): no
public API, event, trace, or metric changed.

**Placements.** The rider starts at `s = 60`, `d = 0`; the body it passes is a 0.5 m circle 30 m
ahead on `pass_lane`, far enough that the completion guard — the rider's rear past that body's
front by the target clearance — cannot end the maneuver before the shared boundary. Every drive
stops the moment the rule is observed, under a 120-tick cap.

- `constraint_an_own_band_leader_bounds_the_committed_crossing`: a 1 m/s body 14.2 m ahead on the
  rider's own band, placed at the start of the committed leg (a *preparing* decision predicts the
  request's within-facility corridor, which a same-lane body inside the horizon would reject as
  infeasible). `nearest_leader` names it while the rider still rides `a`; the minimum committed
  speed falls to **3.36 m/s** from the 6 m/s free flow, and the rider still crosses with no abort.
- `constraint_a_destination_band_leader_bounds_the_committed_crossing`: a 1 m/s body 14.2 m ahead
  on the destination band, through the same adjacency. The rider still owns band `a`, the far-side
  read names that body at 1 m/s, and the minimum committed speed falls to **4.12 m/s**. The
  control — the identical scripted crossing with no destination body — holds **6.00 m/s** and
  crosses, so the bound is the destination body's and nothing else's.
- `constraint_a_body_closing_from_behind_on_the_destination_band_holds_the_crossing`: the rider
  commits with the destination band clear, then a 12 m/s body is placed 36 m behind it there. The
  6 m/s difference carries it alongside exactly as the 6.0 s horizon ends, where the outbound
  corridor has reached the destination band and the two footprints nearly touch. The rider aborts
  on the first committed step with `ClearanceLost` and never crosses.
- `constraint_an_own_band_follower_stays_behind_and_does_not_hold_the_crossing`: a 6 m/s body 5 m
  behind the rider on its own band. Every source-band tick reads the rider as that body's
  `nearest_leader`, the separation never falls below the 1.8 m touching distance (observed minimum
  5.00 m), and the rider crosses.

**Re-derived thresholds.** The streamed family's `< 5.0 m/s` bound still separates a constrained
approach from the control on the scripted placement (3.36 and 4.12 against 6.00), so it is kept
verbatim; the follower's 1.8 m no-overlap bound is kept because the scripted separation only opens.
`emergency_cap_steps() == 0` is kept in all four.

**Anti-teleport bound.** The streamed family's `max_step_m` assertion is restored on the scripted
fixture: `drive_scripted_crossing` (and the follower rule's own loop) measures the largest per-step
world displacement of any live body, from the poses the drive starts at, and each test asserts it
against `CONSTRAINT_MAX_BODY_SPEED_MPS * dt + SETTLE_TOLERANCE_M` — the fixture's fastest scripted
body at 12 m/s. A handoff that moved a pose rather than reading it from the integrated world pose
would fail instead of passing as a crossing. Observed maxima are 0.30 m for the 6 m/s bodies and
0.60 m for the 12 m/s closing body against the 0.6 m + 1e-3 m bound, and the tracker is live in every
driven window: tightening the constant to 6 m/s fails the closing-companion rule at exactly 0.600 m.
One inert divergence from the deleted companion modes is known and recorded here: a `lateral = false`
companion is pushed from the rider mode with only the bounded-steering envelope withheld, so its route
state still carries the template's `target_clearance_m` and `horizon_s`, where the deleted companion
modes declared no `lateral` object at all. It has no behavioural effect — the companion is never a
maneuver candidate and both fields are read only by the maneuver pass — so it is left as-is rather
than widening the fixture with a second mode template.

**Focused gate.** `cargo nextest run -p hekate-sim`: **457 tests, 457 passed**, 42.3 s wall / 2m 13.6 s
`user` on an unloaded host, no slow-timeout lines; re-run under heavy host load it is 84.2 s wall /
2m 15.9 s `user` with 3 pre-existing `lane_transitions` checks still flagged SLOW (none of the new
tests; they run in 0.08–1.04 s even loaded). With the anti-teleport bound in place,
`cargo nextest run -p hekate-sim --lib constraint` is 5 passed in 0.51 s and
`cargo nextest run -p hekate-sim --test lane_transitions` is 14 passed in 18.7 s (was 18 tests in
38.9 s wall / 2m 0.6 s `user`; the four rule tests alone were 17.3–34.2 s each). `cargo fmt` and
`cargo clippy -p hekate-sim --all-targets -- -D warnings` are clean, and `tangle check` passes.

# Manifest

- source: crates/hekate-sim/src/sim.rs
- source: crates/hekate-sim/tests/lane_transitions.rs
- verify: tangle check
