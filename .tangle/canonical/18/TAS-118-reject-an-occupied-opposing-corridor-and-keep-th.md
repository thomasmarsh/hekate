---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Reject an occupied opposing corridor and keep the ordinary lifecycle.
---

Parent [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].

# Outcome

An occupied opposing corridor is handled by the ordinary feasibility and gap
machinery, and the wrong-way agent keeps its normal identity, events, metrics, and
lifecycle.

# Done when

- An occupied opposing corridor yields an infeasible claim or a bounded wait,
  brake, or abort; no flag disables collision, clearance, safety, or leader
  queries.
- The agent retains its normal stable ID, mode template, profile, metrics
  dimensions, event order, and lifecycle.
- Focused tests cover occupied rejection, head-on following and yield, collision
  visibility, and disconnected rejection.

# Context

Gated on [[TAS-097-make-contextual-wrong-way-decisions-reproducible]].
Extends [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; reads
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the occupancy
rejection and the ordinary-path preservation.

# Result

Landed the occupied-corridor bound and the ordinary-lifecycle preservation, with
one production defect fixed and six focused tests.

## Defect and fix

`crates/hekate-sim/src/sim.rs (Simulation::nearest_leader)` handed the leader's
own speed to the longitudinal model, so the oncoming body a turned rider drives
into was presented as a leader receding at its own speed. The rider closed on it
at the sum of the two speeds while IDM's closing-speed term saw no closure, the
profile's comfortable braking was never applied in time, and only the anti-overlap
position cap stopped the pair: at the fixture's 17 m gap the run reported
`Event::Collision` with `contacting: true` and counted the backstop in
`emergency_cap_steps`, and even the roomy fixture's 44.7 m gap ended at zero
clearance. The fix is the smallest ordinary correction — the constraint carries
the leader's speed along the agent's own travel axis, so a same-way leader keeps
its own speed and an oncoming one takes the opposite sign — and the model's own
`dv` term then reads the true closure. Falsified both ways: reverting the
expression makes `an_occupied_opposing_corridor_bounds_the_turned_rider` fail at
`-2.9e-15 m`, and with it the rider holds a 1.69 m standstill gap with
`emergency_cap_steps() == 0`.

No flag, mode branch, or scenario name is involved, the collision scan and every
clearance, safety, and leader query stay on, and no event or metric shape changed.

## Tests (`crates/hekate-sim/tests/wrong_way.rs`)

- `an_occupied_opposing_corridor_bounds_the_turned_rider` — occupied rejection as
  a bounded brake: a rider that turns into the oncoming body holding the corridor
  comes to rest behind it with a positive gap and no contact, with no yield, stop
  line, signal, or maneuver in play, so the leader ahead of it in its actual
  travel direction is the only constraint that can hold it, and the anti-overlap
  cap never binds.
- `the_collision_scan_reports_the_encounter_the_turned_rider_cannot_avoid` —
  collision visibility and head-on yield: where the geometry leaves no stopping
  room, the ordinary scan still reports the pair in its near-miss band and then
  in its contact band, the bodies never overlap, and the interaction metrics
  observe the pair under its ordinary agent-pair dimension.
- `a_wrong_way_rider_keeps_its_identity_profile_and_lifecycle` — clause 2: one
  spawn and one despawn record name the same `AgentId`, the mode and the profile
  admission sampled are unchanged, every tick keeps the documented within-tick
  order, and the completed trip is served under the ordinary mode and run
  dimensions.
- `a_nominal_only_participant_sharing_the_facility_never_reaches_the_entry` —
  the falsifying mixed-capability acceptance input: one participant carries
  `reverse_direction` and one is the same mode with that tactic removed, on the
  same facility and movement. Expected behavior, asserted: the capable rider's
  request is accepted and its traversal turns, while the nominal-only
  participant's request is rejected outright and it never travels the opposing
  traversal.
- `a_disconnected_opposing_traversal_is_refused_and_the_rider_completes_its_nominal_route`
  — disconnected rejection, total: the request records nothing and the rider
  keeps its nominal traversal and completes its nominal route.
- `no_authored_flag_can_disable_a_query` — the wrong-way policy carries the
  contract's three thresholds and its document rejects an unknown key, so a flag
  that would switch a collision, clearance, safety, or leader query off cannot be
  authored at all.

When the encounter is geometrically unavoidable the pair ends in a zero-clearance
standoff produced by the documented anti-overlap backstop; the leader constraint
cannot prevent a contact that the geometry forbids avoiding, and the bounded
brake the contract asks for is what the machinery delivers.

Evidence, all from this worktree at base `dd455e6`: `cargo test -p hekate-sim
--test wrong_way` 12 passed; `cargo test -p hekate-sim --lib` 215 passed; `cargo
test -p hekate-sim` all 29 targets green (no Phase 1 / Increment 1 / TAS-113..117
regression); `cargo test -p hekate-cli --test golden_trace` and `cargo test -p
hekate-present --test scene_golden` green, so no golden moved; `cargo clippy
--workspace --all-targets --all-features -- -D warnings` clean; `cargo fmt --all
--check` clean; `scripts/check-dependency-direction.sh` OK.

Complete. Pending parent roll-up: [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]
still routes `next` to this node and its write set excludes this one, so the
advance is the coordinator's integration action, verified with `tangle check
--allow-pending-advance TAS-098`.
