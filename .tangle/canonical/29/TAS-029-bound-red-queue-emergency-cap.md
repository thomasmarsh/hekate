---
status: proposed
context_rev: 1
priority: P2
updated: 2026-09-15T01:22:43Z
summary: Increment 2's four_leg_signal_v1 engages the emergency anti-overlap cap during red-queue formation with a −28.3 m/s² worst step, so bounded queue comfort outside the controlled benchmark is unverified.
next: Reproduce the four_leg_signal_v1 seed-0 emergency-cap engagement and identify the admission or queue step whose commanded deceleration exceeds the sampled comfortable braking bound.
---

# Outcome

Increment 2's controlled `car_following_v1` benchmark proves bounded
acceleration, braking, and speed under real following, but its sibling
`four_leg_signal_v1` engages the last-resort anti-overlap position cap while a
red-light queue forms. That cap sits outside the IDM clamp and can command a
single step beyond the profile's comfortable braking bound (worst step
−28.3 m/s² in seed 0, per
[[TAS-027-phase-1-increment-2-vehicle-flow-controls]] at context_rev 2).

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
