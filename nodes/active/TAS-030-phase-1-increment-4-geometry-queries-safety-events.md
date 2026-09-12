---
context_rev: 1
priority: P1
updated: 2026-09-12T21:33:37Z
summary: Phase 1 Increment 4 adds a deterministic uniform-grid broad phase, exact and swept geometry queries, typed safety events with a versioned union, online TTC/minimum-separation and PET occupancy, and viewer event overlays and inspector links.
next: Add swept candidate bounds and time-of-impact shape casts for tunneling protection, with swept fixtures that do not overlap at either tick endpoint.
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

## Slice A — deterministic broad phase and exact geometry queries

Slice A formalises the Increment 3 `index.rs` candidate query into the
Increment 4 broad phase and adds the exact narrow-phase queries it feeds. It
does not rewire the tick: the kernel's crossing-occupancy query keeps its
existing semantics, so the walking trace golden, the scene golden, and the
Phase 1 baseline are unchanged.

### Broad phase

`BroadPhase` is a public uniform grid keyed by `AgentId`. `rebuild` takes
`(AgentId, BodyShape)` pairs in any order and sorts cells and residents by
ascending `AgentId`, so the candidate set is a pure function of the indexed
bodies, never of insertion order. `candidates_in_aabb` returns every body whose
centre cell overlaps a box; `candidates_overlapping` widens that box by the
largest body circumradius, so a body whose centre is outside the region is still
returned when its extent reaches in; `candidate_pairs` enumerates every unordered
pair whose enclosing boxes overlap, once each, in ascending
`(AgentId, AgentId)` order. No hash map is iterated for state-affecting logic.
`SpatialIndex` is the crate-internal `AgentStore` adapter: it maps each live
agent to its body shape and rebuilds the broad phase once per tick from the
shared, stable-order store, skipping dead slots, and its reused body buffer
keeps the tick allocation-free after warmup.

### Exact queries

`crates/tangle-sim/src/query.rs` defines `BodyShape` (a vehicle is an oriented
box, a pedestrian a circle of half its reported body length), `Aabb`, and the
signed-clearance query `body_clearance_m` with the predicate `bodies_intersect`
(`clearance < 0`). The model card documents the semantics: positive clearance is
the exact disjoint distance, negative is `-penetration_depth`, zero is contact,
and the query is symmetric. Circle/circle and circle/box are analytic; box/box
uses the separating-axis test, returning the exact vertex-to-box distance when
disjoint and the least projection overlap when penetrating. The only tolerance
is `CONTACT_EPSILON_M = 1e-9 m`, the band at exact box/box contact that reads as
touching rather than sub-nanometre penetration. `f64`/`glam::DVec2` only.

### Schema impact

None. No scenario-source or record schema changed, so schema version 1 is
unchanged and no schema regeneration or drift-test update was needed.

### Public surface

`tangle_sim::{BroadPhase, Aabb, BodyShape, body_clearance_m, bodies_intersect,
CONTACT_EPSILON_M}`. `SpatialIndex`, `GRID_CELL_SIZE_M`, and
`circle_overlaps_ring` stay crate-internal; the tick integration is unchanged.

### Evidence

- Unit tests in `query.rs` and `index.rs`: hand-checked signed clearances for
  all three query pairs, rotated-box separation, body bounds, deterministic pair
  ordering, input-order independence, and the centre-outside query.
- `crates/tangle-sim/tests/geometry_queries.rs`: randomized all-pairs property
  tests over 12 fixed seeds and 16-body worlds. Indexed candidate pairs equal
  the all-pairs bounds-overlap reference and, filtered by the exact test, equal
  the all-pairs exact-overlap reference; every queried body whose bounds overlap
  the query is returned; exact clearances and intersection match an independent
  brute-force reference within `1e-9 m`; candidate order is ascending and unique.
- All five gates pass on the final tree: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check nodes`.

### Deferred

Slices B–E remain: swept candidate bounds and time-of-impact casts, typed safety
events with deterministic lifecycle and ordering, online TTC /
minimum-separation tracking and PET occupancy, and viewer overlays and
inspector links. The kernel also still owns the Increment 3 residual that a
committed vehicle does not reserve a crossing region and that pedestrian
avoidance is a bounded closing-component cap rather than a swept query.

# Limitations

None yet.
