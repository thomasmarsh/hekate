---
context_rev: 1
priority: P1
updated: 2026-09-14T02:42:36Z
summary: Predict usable maneuver corridors and conservative front, rear, side, and swept clearance.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

A pure deterministic query evaluates whether a proposed lateral trajectory fits
its connected facilities and conservatively predicts minimum body-to-body
clearance over the configured horizon.

# Done when

- Inputs are compiled geometry, current world bodies and velocities, bounded
  candidate motion, body envelope, target clearance, and horizon; outputs name
  feasibility plus the limiting boundary or agent and front/rear/side/swept
  clearance facts.
- Candidate collection uses the ordinary broad phase, stable AgentId ordering,
  and exact body shapes; the final result does not depend on insertion order.
- Prediction covers the whole transition corridor, facility boundaries, and
  both current and destination flows, not only endpoint lane-centre gaps.
- Analytic straight-line fixtures cover approaching rear, slower front,
  side-by-side, crossing sweep, empty corridor, curved boundary, and ties.
- A conservative result may reject a marginal gap but never reports feasible
  when its own fine subdivision detects less than target clearance.

# Context

Gated on [[TAS-089-integrate-bounded-single-body-steering]]. Owns a focused
prediction/corridor module, minimal query exports, and unit tests. Reuse
body_clearance_m, swept bodies, tick_minimum_clearance_m, CompiledFacility, and
the spatial index; do not choose or commit tactics.

# Result

TAS-090 is complete. A pure, deterministic, non-mutating query predicts the
corridor and clearance facts of one candidate lateral maneuver and returns
feasibility plus the limiting boundary or agent. It chooses no tactic, commits
nothing, and reads no wall clock, filesystem, or UI type.

**New module** `crates/tangle-sim/src/prediction.rs` (public, re-exported from
`lib.rs`). One entry point:

- `predict_maneuver_corridor(inputs: ManeuverInputs, bodies: &[PredictedBody])
  -> ManeuverPrediction`.
- `ManeuverInputs` carries the compiled geometry (`&CompiledReferencePath`), the
  facility band width, the travel `direction`, the current world body envelope
  (`BodyShape`), the speed, the candidate `target_offset_m` (agent travel
  frame), the compiled `BoundedSteering` envelope, the `target_clearance_m`, the
  `horizon_s`, the run's `cadence_s`, and a finite `subdivisions` count.
- `PredictedBody { id: AgentId, shape: BodyShape, velocity_mps: DVec2 }` is one
  current world body.
- `ManeuverPrediction { corridor: Vec<CorridorSample>, clears:
  PredictedClearances, verdict: PredictionVerdict }`. `CorridorSample` is a
  time-stamped pose in world and route (`s_m`, `d_m`) coordinates.
- `PredictedClearances { front: Option<ClearanceFact>, rear: Option<..>, side:
  Option<..>, swept: ClearanceFact }`; `ClearanceFact { clearance_m, object,
  time_s, closing_speed_mps }`; `LimitingObject::{Agent(AgentId), BandEdge}`;
  `PredictionVerdict::{Feasible, Infeasible { limiting }}`.
- `MAX_PREDICTION_STEPS` and `DEFAULT_SUBDIVISIONS` are the documented bounds.

**How each Done-when bullet is met.**

- *Inputs and outputs*: the compiled reference plus facility width, the world
  bodies and velocities, the bounded candidate motion (integrated with TAS-089's
  `bounded_steering_step` at `cadence_s / subdivisions`), the envelope, target
  clearance, and horizon drive the query; the verdict names `Feasible` or the
  limiting `LimitingObject`, and the four facts carry the signed clearance, the
  limiting object, the time of the minimum, and the closing speed.
- *Broad phase, exact shapes, insertion order*: candidates come from the
  ordinary `BroadPhase` rebuilt from the body shapes and queried with the box of
  the whole corridor widened by the largest body displacement over the horizon
  (and, by the query, the largest circumradius). It returns ascending `AgentId`
  order, and the facts fold with a strict `<`, so the result is a pure function
  of the body *set*; the `the_result_is_independent_of_body_insertion_order`
  fixture asserts `[a, b] == [b, a]`.
- *Whole corridor, boundaries, both flows*: the corridor is the fine-grained
  candidate sweep, not two lane centres; the band-edge fact is the least
  envelope-to-band distance over the whole corridor, computed in the facility's
  route frame (`width_m / 2 - |d| - envelope half-extent along the reference
  normal`), so it holds on a curved reference; every world body in the corridor
  box is tested by exact shape, so a body in the current flow and one at the
  destination offset are both facts
  (`the_corridor_covers_the_current_and_destination_flows`).
- *Fixtures*: `crates/tangle-sim/tests/prediction.rs` (12 tests) covers the
  approaching rear, the slower front, side-by-side, the crossing sweep caught
  between samples, the empty corridor, the curved boundary, and the exact tie,
  plus insertion-order independence, the conservative minimum across
  subdivisions, and a `CompiledFacility`-sourced request. Ten `#[cfg(test)]`
  tests in the module mirror the analytic cases at crate scope.
- *Conservatism*: feasibility is defined by the same fine subdivision the result
  reports, and each fine step is read with the exact swept minimum
  (`tick_minimum_clearance_m`) rather than its endpoints, so a crossing between
  two samples is caught even at one subdivision; the result can never be
  `Feasible` while a minimum it computed is below target.

**Acceptance** (exact commands and observed results):

- `cargo test -p tangle-sim` — 24 suites, 301 passed, 0 failed; the library has
  162 tests (152 pre-existing + 10 new) and the new `prediction` suite has 12.
- `cargo test --workspace` — exit 0, every suite passes, 0 failed; no golden was
  regenerated, so every checked-in trace hash is byte-identical.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets` — clean;
  `cargo fmt --all -- --check` — clean.
- `braintree check` while TAS-090 was `proposed` — `graph check: passed (154
  nodes)`; after the move, `braintree check --allow-pending-advance TAS-087` —
  `graph check: passed (154 nodes)` ([[TAS-087-continuous-lateral-motion-and-gap-machinery]]'s
  `next` names this now-resolved node, the parent advance the coordinator owns).

## Remaining scope (recorded, not deferred to a new node)

- The band edge is the *current* facility's boundary. A lateral transition into
  an adjacent facility needs the destination band's far edge expressed in the
  agent's travel frame; the compiled adjacency exposes the destination
  traversal and side but no band separation, so the caller cannot derive it from
  compiled data alone. Destination *flows* are still covered, because every
  world body is tested by exact shape regardless of facility. The lane-transition
  leaf ([[TAS-095-complete-lane-transitions-and-safe-aborts]]) or a follow-up
  can extend `ManeuverInputs` with the resolved transition bounds once the
  adjacency carries a separation.
- The predictor integrates with `bounded_steering_step`, which bounds the motion
  to the *current* usable corridor, so an out-of-corridor target is reported
  `Infeasible { BandEdge }` rather than predicted past the boundary. That is the
  intended feasibility boundary; crossing into the destination band is a
  claim/handoff concern owned by [[TAS-091-resolve-gap-claims-and-maneuver-transitions]].

## Friction (for the session FBK; no FBK node was created)

- Attempted: express "a body ahead" for the `front_clearance_m` fact exactly as
  the contract words it. Friction: the contract says front is "a body ahead
  (greater route progress on the same traversal)" and side is "a body whose
  progress interval overlaps the agent's", so a *slower leading* body the agent
  catches up to satisfies both readings, and a horizon-swept interval reading
  turns the "slower front" fixture into a side fact. Improvement: state that the
  progress interval is a body's current longitudinal footprint (its projected
  arc length plus and minus its own half-extent), classified at the current
  instant, with the minimum taken over the horizon — which is what the fixtures
  require and what this module implements.
- Attempted: read the band edge from a `CompiledFacility` so the input is one
  compiled object. Friction: `CompiledFacility` has no public constructor, so a
  `tangle-sim` integration and unit test cannot build one; binding the module to
  it would force every test through `CompiledScenario::compile_v2`, and the
  existing `steering.rs` seam already takes `&CompiledReferencePath` plus the
  width. Improvement: expose a public facility constructor or state that a
  consuming crate owns its own band value, as TAS-089 recorded for
  `UsableLateralInterval`.
- Attempted: derive the destination band's boundary for a cross-facility
  transition from compiled data. Friction: `CompiledFacilityAdjacency` names the
  destination traversal and side but no band separation or offset, so the
  predicted corridor's boundary on the destination band is not derivable from
  the compiled graph. Improvement: record the adjacency separation (or the
  destination band's offset in the source band's frame) in the compiled
  adjacency, or state that the transition bounds are a runtime input.
