---
context_rev: 1
priority: P1
updated: 2026-09-12T21:46:49Z
summary: Phase 1 Increment 4 adds a deterministic uniform-grid broad phase, exact and swept geometry queries, typed safety events with a versioned union, online TTC/minimum-separation and PET occupancy, and viewer event overlays and inspector links.
next: Add typed collision, near-miss, violation, entry/exit, queue, and control-transition events with deterministic ordering and a documented emission-once lifecycle, make EVENT_VERSION describe the full event union (F4), and carry the agent mode through Event::Spawned (F5).
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

## Slice B — swept candidate bounds and time-of-impact casts

Slice B adds the continuous query the static layer cannot answer: a body that
crosses another between two ticks without overlapping at either tick endpoint.
Like slice A it does not rewire the tick. The kernel's crossing-occupancy query
is unchanged, so the walking trace golden, the scene golden, and the Phase 1
baseline are unchanged; wiring a swept query into the tick would widen that
candidate set, so it belongs to slice C, which owns the collision and near-miss
events that give the cast a consumer.

### Swept bounds

`SweptBody` is one body over one tick: its shape at the tick start plus a linear
displacement in metres. The kernel integrates position only, so a body keeps its
heading and extents for the tick and a cast solves translation only.
`SweptBody::swept_bounds` is the tight box around the swept volume: each axis of
the enclosing box varies linearly over the tick, so the extremes are the tick
endpoints and the box of the two endpoint boxes is exact, containing both
exactly. `SweptBody::swept_reach_m` is the circumradius plus the displacement
magnitude, the furthest the body can be from its start centre, which is the
bound the broad phase widens a query by.

`SweptBroadPhase` in `index.rs` composes slice A's `BroadPhase`: the grid
indexes each body's start centre and stores its tick-swept bound per body. A
query widens by the largest swept reach and every candidate is then filtered by
its own swept bound, so `candidates_overlapping` returns exactly the bodies
whose swept bound overlaps the query. Candidates are ascending and unique by
`AgentId`, pairs are ascending `(first, second)` with `first < second`, and a
rebuild is a pure function of the indexed bodies, so the pair set is a superset
of every pair that can touch during the tick, including the crossing pairs a
start-bound index misses.

### Time-of-impact casts

`time_of_impact(first, second)` returns `Some(TimeOfImpact { time, normal,
clearance_m })` or `None`. The signed clearance between two convex bodies
translating linearly is convex in the tick fraction: the separation is the
distance from `t` times the relative displacement to the Minkowski difference of
the two start shapes, a convex set, and the distance to a convex set is convex.
The clearance rate is therefore non-decreasing and the band-entry set is a
single interval, so the cast bisects the tick at most twice: once for the first
fraction where the clearance rate turns non-negative, which is the first contact
when one exists, and once for the first band entry. Both predicates are monotone
and bisection is exact up to floating point; because the cast reads the whole
interval instead of sampling it, an arbitrarily narrow crossing window between
the endpoints is still found.

Documented semantics: a hit is a first entry into the contact band
(`CONTACT_EPSILON_M`, so exact touching counts and a graze within a nanometre
reads as a touch); start overlap reports `time = 0.0` with a negative clearance
and the minimum-translation normal; a pair whose closest approach stays above
the band reports `None`, which covers parallel and coincident motion because a
constant clearance never closes. The normal is the unit direction from the first
body toward the second, from `body_contact_normal`, which also documents its
per-pair tie-breaks; the cast reuses the static queries rather than restating
them, so a swept and a static query agree exactly on what touching means.
`TOI_TIME_TOLERANCE` (`1e-12` of a tick) is the declared resolution of a reported
time, and no hit is reported when the located closest approach exceeds the band
by more than the relative displacement times that tolerance.

### Public surface

`tangle_sim::{SweptBody, SweptBroadPhase, TimeOfImpact, time_of_impact,
TOI_TIME_TOLERANCE, body_contact_normal}`. `BodyShape::translated` stays
crate-internal. The tick integration, `SpatialIndex`, and
`crates/tangle-sim/Cargo.toml` are untouched, so no dependency was added.

### Schema impact

None. No scenario-source or record schema changed, so schema version 1 is
unchanged and no schema regeneration or drift-test update was needed.

### Evidence

- Unit tests in `swept.rs`, `index.rs`, and `query.rs`: the swept bound contains
  both endpoint boxes exactly and equals the body box for a still body; the
  swept phase pairs a crossing body the static phase misses; a candidate whose
  centre is outside the query; candidate and pair ordering plus rebuild
  input-order independence; start-overlap and touching-at-start semantics;
  parallel and coincident motion; grazes inside and above the band; cast
  symmetry; and the contact normal's separating properties (stepping along it
  changes the clearance at unit rate, stepping by the penetration depth leaves
  the pair exactly touching, and corner-to-corner proximity is not a face axis).
- `crates/tangle-sim/tests/swept_queries.rs`: hand-computed fixtures for a fast
  box crossing a thin box (no endpoint overlap, first contact at 0.29 of the
  tick, and the static phase pairs nothing while the swept phase pairs the
  crossing), a fast circle crossing a circle (first contact at 1/3), crossing
  diagonal paths (an analytic time with a diagonal normal), and 20 narrow-window
  crossings (wall thickness 0.02–0.4 m at 5–100 m per tick) that a static
  endpoint test misses. Randomized property tests over 8 fixed seeds compare the
  swept candidate pairs with the all-pairs swept-bound reference, the
  cast-filtered candidates with the all-pairs contact reference, and every cast
  with a uniform-sampling reference (4096 samples plus a bracketed refinement,
  with a Lipschitz certificate for absences), and check the documented
  invariants: `time` in `[0, 1]`, unit normal, clearance inside the band, no
  earlier sample inside the band, and argument symmetry. Declared tolerances:
  `1e-9` of a tick for times, `1e-6` for normals, and `CONTACT_EPSILON_M` for the
  band.
- All five gates pass on the final tree: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check nodes`. The walking trace golden, the scene golden, and the
  Phase 1 baseline pass without regeneration.

### Deferred

- Tick wiring: the swept candidate query and the cast are not yet on the tick
  path. Slice C owns the collision and near-miss events that consume them, and
  the crossing-occupancy query stays on slice A's static candidates until that
  consumer exists, because widening it now would change vehicle yielding without
  a recorded before/after.
- The cast bisects to a fixed resolution rather than carrying a
  continuous-advancement fast path; profile slice C before adding one.
- `PHASE_1_PLAN.md`'s numeric choices name `parry2d-f64` query primitives for
  exact distance/intersection and shape casting. Slice A built the exact queries
  on `glam::DVec2` and slice B extends that layer, so the crate still has no
  parry dependency; the deviation stands for the coordinator to accept or
  reverse.

# Limitations

None yet.
