---
context_rev: 1
priority: P1
updated: 2026-09-13T00:00:00Z
summary: Phase 1 Increment 4 adds a deterministic uniform-grid broad phase, exact and swept geometry queries, typed safety events with a versioned union, online TTC/minimum-separation and PET occupancy, and viewer event overlays and inspector links.
next: Formalize the slice-D spatial index into a deterministic uniform-grid broad phase with stable candidate ordering, and add exact box/box, circle/circle, and box/circle distance/intersection queries with randomized all-pairs property tests.
---

# Outcome

Per `PHASE_1_PLAN.md` Increment 4, the simulator gains a validated geometry and
safety-event layer over the shared vehicle/pedestrian world:

- Deterministic uniform-grid broad phase with stable candidate ordering.
- Exact box/box, circle/circle, and box/circle distance and intersection
  queries.
- Swept candidate bounds and time-of-impact shape casts for tunneling
  protection.
- Typed collision, near-miss, violation, entry/exit, queue, and
  control-transition events.
- Online TTC and minimum-separation tracking; conflict-region occupancy
  intervals for PET.
- Viewer overlays and inspector links from an event to its participants.

Increment 4 also absorbs the TAS-028 residuals: F4, make `EVENT_VERSION`
describe the record union, and F5, carry the agent mode through
`Event::Spawned` and `SceneBody::project`.

Any schema change stays additive to schema version 1: validate, regenerate
`schemas/scenario-source.schema.json`, and keep the drift test. No schema
version 2.

Constraint: `f64`/`glam::DVec2`, single-threaded state-affecting tick, stable
ordering with explicit tie-breakers (ascending `AgentId`), no Bevy types in
`tangle-model` or `tangle-sim`. Do not break existing goldens or baselines
without a deliberate, reported regeneration.

# Done when

- Property tests compare indexed candidates with an all-pairs reference on
  randomized small worlds.
- Swept fixtures detect crossing bodies that do not overlap at either tick
  endpoint.
- Event pairs have deterministic ordering and are emitted once according to a
  documented lifecycle.
- Fine-step differential tests agree with analytic/simple fixtures within
  declared tolerances.
- `EVENT_VERSION` describes the full event union (F4), and the agent mode is
  carried through `Event::Spawned` and `SceneBody::project` (F5).
- Viewer overlays and inspector links connect an event to its participants.
- Online TTC and minimum-separation tracking, and conflict-region occupancy
  intervals for PET, have evidence.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Area [[IDX-001-tangle]].
Depends on [[TAS-028-phase-1-increment-3-pedestrians-mixed-interaction]] at context_rev 1.

Increment 3 landed pedestrians, mixed interaction, the shared `index.rs`
candidate-query foundation, `Event::Yielded`, and replaceable controllers. This
increment formalizes the broad phase, adds exact and swept geometry queries, and
builds the typed safety-event and interaction-metrics layer on top. It also
carries the two recorded TAS-028 residuals (F4 event-version union, F5
mode-blind `Event::Spawned`/`SceneBody::project`).

`TAS-029-bound-red-queue-emergency-cap` is an independent P2 follow-on owned by
`IDX-001`; it does not gate this increment.

# Result

Slices are recorded here as they integrate.

# Limitations

None yet.
