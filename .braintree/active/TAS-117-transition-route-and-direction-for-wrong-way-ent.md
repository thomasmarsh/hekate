---
context_rev: 1
priority: P1
updated: 2026-09-14T15:31:55Z
summary: Transition route and direction for wrong-way entry and completion.
next: "[[TAS-138-order-a-turned-riders-leaders-and-encounters]]"
---

Parent [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].

# Outcome

A wrong-way agent enters the connected opposing traversal through the ordinary
transition contract and makes correct progress and completion in both authored
reference directions.

# Done when

- Route state changes direction and facility through the same connected-transition
  contract as an ordinary lateral maneuver, with the world pose continuously
  authoritative.
- Opposing leaders and encounters are ordered in the actual travel direction, and
  route progress and completion work in both authored reference directions.
- Focused tests cover clear opposing entry and route completion.

# Context

Depends on [[TAS-097-make-contextual-wrong-way-decisions-reproducible]] at context_rev 1.
Extends [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; reads
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the route and
direction transition; do not disable collision or add a scripted trajectory.

# Result

Landed the wrong-way entry: the runtime path from a selected opposing decision to
route state travelling on the connected opposing traversal, with direction,
progress, and completion correct in both authored reference directions.

- `crates/tangle-sim/src/sim.rs (Simulation::request_wrong_way_entry)`: the
  caller-driven entry seam, modelled on `Simulation::request_lateral_maneuver`.
  It records the request and returns `false`, recording nothing, when the
  scenario authors no `maneuver_policy.wrong_way`, the agent is not alive, it
  carries no route state, its mode's compiled tactics carry no
  `reverse_direction`, or the compiled topology connects no opposing traversal.
- `crates/tangle-sim/src/sim.rs (Simulation::resolve_wrong_way_entries)`: the
  kernel pass, run at the start of a tick immediately after
  `Simulation::resolve_maneuvers` and before any agent command, so every decision
  reads the same immutable tick-start observation as the maneuver batch. Requests
  are evaluated in ascending agent id order; each agent's draw is
  `wrong_way::maneuver_draw(seed, agent, ordinal)` for its own decision ordinal,
  and the ordinal advances on every evaluation, so a rejected or nominal decision
  cannot perturb another agent's draws. It is inert unless the scenario authors
  `wrong_way`, so no Phase 1 / Increment 1 / TAS-113/114/115/116 behaviour changes.
- `crates/tangle-sim/src/sim.rs (Simulation::wrong_way_inputs)`: builds
  `WrongWayInputs` from the compiled policy and the tick-start state only — the
  opposing traversal's physical connectivity from
  `CompiledFacility::physically_possible_directions`, the facility's authored
  nominal direction, the two remaining lengths and the expected speed (desired
  speed capped by the facility limit), the observed opposing density, the
  applicable `FacilityTraversalPolicy::nominal_effect`, the sampled compliance and
  desired speed, and the scenario thresholds.
- `crates/tangle-sim/src/sim.rs (Simulation::opposing_traversal_direction,
  Simulation::enter_opposing_traversal)`: resolve the connected opposing
  traversal and move route ownership in one step through the same discipline the
  facility handoffs use — the world pose is the integrated truth and is never
  moved, the new `s_m`/`d_m` are that same pose projected onto the reference in
  the new travel frame (`Simulation::reproject_route_state`), the stored
  `AgentStore.heading_rad` stays the reference tangent so `travel_heading` of the
  new sign carries the travel frame, and the movement route is left behind
  exactly as a handoff leaves it.
- `crates/tangle-sim/src/agent.rs (RouteState)`: `wrong_way_entry_requested` and
  `wrong_way_decisions` carry the recorded request and the agent's decision
  ordinal; both start clear in `RouteState::project`.

Evidence, all from this worktree at base 289fa190:
`cargo test -p tangle-sim --test wrong_way` 4 passed (clear opposing entry turns
and keeps the pose continuous; completion in the forward-reference and
reverse-reference directions each through the ordinary connector handoff onto the
connected reverse traversal; refusal without the capability or without a
connected opposing traversal); `cargo test -p tangle-sim --lib` 215 passed;
`cargo test -p tangle-sim` all suites green (no Phase 1 / Increment 1 / TAS-113..116
regression); `cargo clippy --workspace --all-targets --all-features -- -D warnings`
clean; `cargo fmt --all --check` clean; `scripts/check-dependency-direction.sh` OK;
`braintree check` passes.

Remaining scope:

- Opposing leaders and encounters ordered in the actual travel direction is
  unverified; that is the child [[TAS-138-order-a-turned-riders-leaders-and-encounters]].
- The entry resolves the same-facility reverse traversal, whose connectivity the
  compiled topology makes possible (a connector leaving or entering along that
  direction, which is what a two-way facility authors). An opposing traversal
  reachable only by *crossing* a compiled adjacency is not landed here: that
  needs the ordinary lateral maneuver's crossing at the compiled shared
  boundary, and `Simulation::request_wrong_way_entry` refuses it rather than
  taking a route-only move away from that boundary.
