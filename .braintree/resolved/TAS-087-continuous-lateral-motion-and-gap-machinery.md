---
context_rev: 1
priority: P1
updated: 2026-09-14T03:32:36Z
summary: Continuous lateral motion, clearance prediction, and deterministic gap claims are complete.
---

Parent [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]].

# Outcome

Single-body wheeled agents can evolve continuously in s/d and world space,
predict front/rear/side/swept clearance, and progress through the documented
maneuver state machine under deterministic competing claims.

# Done when

- [[TAS-088-add-route-relative-lateral-agent-state]] adds authoritative
  route-relative tactical state and exposes it without changing collision truth.
- [[TAS-089-integrate-bounded-single-body-steering]] reconstructs and projects
  bounded world motion without lateral snapping or limit violations.
- [[TAS-090-predict-maneuver-corridors-and-clearance]] evaluates usable
  corridors and conservative finite-horizon clearance.
- [[TAS-091-resolve-gap-claims-and-maneuver-transitions]] owns the complete
  following/preparing/committed/returning/aborted lifecycle and tie-break.
- Unit and focused integration tests pass at Fast, Standard, and Fine where the
  benchmark matrix requires them.

# Context

Gated on [[TAS-082-increment-2-authored-and-compiled-contract]]. The
coordinator owns ledger roll-up only; its children own implementation seams.

# Result

All four children resolved; none disposed. The lateral-motion and gap machinery
is complete on the compiled contract from TAS-082.

- **Route-relative state.** [[TAS-088-add-route-relative-lateral-agent-state]]
  initializes `RouteState` (facility, `s`/`d`, five-state `ManeuverState`,
  target offset/facility, predicted gap, target clearance/horizon) from compiled
  facility projection, reprojects through the controller-stage pipeline in
  stable AgentId order, and exposes optional route state on the snapshot and the
  trajectory artifact under `TRAJECTORY_FORMAT_VERSION` 2 -> 3. Passenger cars
  and narrow agents share one physical-family representation; pedestrians and
  version-1 paths keep their output.
- **Bounded steering.** [[TAS-089-integrate-bounded-single-body-steering]] adds a
  pure integrator that caps heading rate by the mode's steering-rate and
  lateral-acceleration limits, integrates world heading/position, projects back
  onto the facility reference, and holds rather than clipping when a step would
  leave the usable corridor; `MotionCommand::RouteSteering` preserves the
  longitudinal path byte-identically when no lateral target is fixed.
- **Prediction.** [[TAS-090-predict-maneuver-corridors-and-clearance]] adds
  `predict_maneuver_corridor`, reporting feasibility plus front/rear/side/swept
  clearance facts with their limiting object, time, and closing speed over the
  whole transition corridor; candidate collection uses the ordinary broad phase
  in stable AgentId order and exact body shapes, invariant to insertion order.
- **Claims and lifecycle.** [[TAS-091-resolve-gap-claims-and-maneuver-transitions]]
  owns the complete lifecycle and batch claim arbitration with the total winner
  key (committed before preparing, then smaller entry distance, then smaller
  AgentId), one transition record per edge for TAS-100, and the ordered
  brake/hold/abort commitment-loss policy without mid-step revocation.

Gate evidence: `cargo test -p tangle-sim` 327 passed; `cargo test --workspace`
830 passed; `scripts/check-dependency-direction.sh` reports
`dependency direction OK`; `braintree check` passes on the landed graph. All
Fast/Standard/Fine-relevant suites pass; the benchmark-matrix tolerance suites
for predicted-versus-executed clearance remain TAS-104's acceptance evidence.
No Phase 1 or Increment 1 behavior, event/metric version, or Phase 1 baseline
changed; the only version bump is the declared `TRAJECTORY_FORMAT_VERSION`, with
no golden regenerated.
