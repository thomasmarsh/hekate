---
context_rev: 1
priority: P1
updated: 2026-09-12T23:24:00Z
summary: Phase 1 Increment 4 adds a deterministic uniform-grid broad phase, exact and swept geometry queries, typed safety events with a versioned union, online TTC/minimum-separation and PET occupancy, and viewer event overlays and inspector links.
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

## Slice C — typed safety events, EVENT_VERSION union, and spawned mode

Slice C adds the typed safety-event layer over the shared world and closes the
two recorded TAS-028 residuals. The tick still does not rewire its
crossing-occupancy query: the safety pass observes each integrated tick before
new demand is admitted, and the yield rule keeps slice A's static candidates, so
the walking trace is unchanged apart from the two deliberate record changes
below.

### Variants

`crate::event` gains one closed union with the existing lifecycle records:

- `Collision { agent, other, clearance_m, contacting }` — actual body overlap or
  contact, swept over the tick, with `agent` the lower id of the pair.
- `NearMiss { agent, other, clearance_m, entering }` — a pair that came within
  `safety::NEAR_MISS_THRESHOLD_M` (1.0 m) without contact.
- `Violation { agent, kind }` — `RanRedLight` for a vehicle that crossed its
  stop line while the recorded decision was to proceed past a forbidding head,
  `CrossedAgainstSignal` for a pedestrian that entered a crossing region while
  the recorded decision was to cross against a forbidding signal.
- `Entry`/`Exit { agent, region }` — a body reaching, then clearing, a crossing
  or conflict region (`RegionKey`), for both modes.
- `Queue { agent, joined }` — reached, or left, a standstill: longitudinal speed
  at or below `safety::QUEUE_STOP_SPEED_MPS` (1e-3 m/s).
- `ControlTransition { agent, control, active }` — a recorded decision state
  change: `SignalStop` for a vehicle holding at a signal-controlled line,
  `CrossingWait` for a pedestrian waiting at a signal-controlled crossing. The
  yield transition keeps its own `Event::Yielded` record, which carries the
  crossing it belongs to.

`Event::Spawned` gains `mode: AgentMode` (F5), and `AgentMode::label()` is the
stable spelling for a trace or inspector. `EventKind` is the payload-free
discriminator, every variant exposes `Event::kind()`, and `Event::agent()` now
covers every record.

### Ordering and lifecycle

`Event::order_key()` is the documented within-tick order, and each step sorts its
buffer by it after every emission point: ascending `AgentId`, then ascending
`EventKind::order()`, then a tag that separates the variant's key space, then the
variant's own stable key (partner agent, crossing, region, path, or sub-kind),
then the record's edge flag. The sort is stable, so two records that agree on all
of that are the same variant with the same key, which the lifecycle forbids; a
residual tie can only be two identical records. No hash map is iterated anywhere
in the pass, and the open-state sets are ordered.

Every variant is edge-triggered on a per-tick predicate, never per-step:

- contact: a candidate pair whose signed clearance reached or passed zero at some
  time in the tick, confirmed with slice B's `time_of_impact` so a sub-tick touch
  is not missed. A Lipschitz certificate (the signed clearance cannot change
  faster than the relative displacement) skips the cast for pairs that cannot
  have been close.
- near miss: the band `(0, 1.0 m]` at some time in the tick, less contact. The
  two families are nested per-tick predicates, not one predicate with a special
  case: the contact band sits inside the near-miss band, so a pair in contact has
  no open near miss and the tick that begins a contact also ends the pair's near
  miss. Both families therefore alternate begin, end, begin for every pair. The
  un-grown `SweptBroadPhase` pair set (bounds overlap) is not enough for the
  band, so the candidate grid indexes each body grown by half the threshold.
- entry/exit: the body's tick-swept bounding circle reaches, then clears, the
  region ring, so a region the body reaches inside one tick is not missed. The
  bounding circle may report entry up to `circumradius + |displacement| / 2`
  early; exact box-versus-polygon clipping is not implemented.
- queue and control transition: tick-end *state* predicates over the agent's
  speed and the decision record the kernel already stores, so a state that comes
  and goes inside a tick is not a transition of that state.
- violation: once per crossing action, from the decision record alone. The
  vehicle case reads the previous and current decision in
  `update_signal_decision`: a `PastStopLine` that follows a proceeding decision
  with a still-positive stop-line gap is the crossing tick, and the `PastStopLine`
  record keeps every later tick from repeating it. The pedestrian case fires on
  the region-entry edge while the recorded decision still describes the approach
  (`crossing_gap_m > 0`) on a `DontWalk` signal.
- `Spawned`/`Despawned` delimit an agent's stream, so a despawn clears the agent's
  open pair and region states without a further record, and closes its open queue
  and control states with their own departure records. An open pair whose
  candidate window has passed is closed by an explicit stale pass, so no end
  record can be missed.

### Version change (F4)

`EVENT_VERSION` goes 1 → 2, the first version that describes the whole record
union: version 1 named only `Spawned`, `Despawned`, and `Yielded`, and `Spawned`
was mode-blind. The canonical trace header now records `event_version` next to
`schema_version`, so a consumer reading only the artifact can tell which union
the records belong to; the baseline manifest already recorded it.

### Golden regeneration (deliberate)

Two record changes invalidate every event golden, and both are reported:

- `tests/golden/walking_guide_v1.trace.jsonl`: the header gains
  `"event_version":2`, and each of the six spawned records gains
  `,"mode":"vehicle"` (+120 bytes: 1197 → 1317 for the standard preset). No other
  record changed, and the walking run emits no safety record.
- `tests/golden/walking_guide_v1.trace.sha256`:
  `dc3207b1…` → `60bd030f…` (full digests below in the commit message).
- `baselines/phase1/baseline.json`: `event_version` 1 → 2 and the three preset
  trace hashes and byte counts (fast 1193 → 1313, standard 1197 → 1317, fine
  1198 → 1318). Counts and the convergence statement are unchanged.

`baselines/phase1/performance.json` is a machine artifact with no trace hashes
and no test that compares it, so it was left untouched. The renderer and scene
goldens do not depend on events and are unchanged.

### Schema impact

None. No scenario-source or record schema changed, so schema version 1 is
unchanged and no schema regeneration or drift-test update was needed.

### Public surface

`tangle_sim::{Event, EventKind, ControlTransitionKind, RegionKey, ViolationKind,
NEAR_MISS_THRESHOLD_M, QUEUE_STOP_SPEED_MPS, band_entry}`, `AgentMode::label`, and
the existing `EVENT_VERSION` at 2. `SafetyMonitor` and the safety pass stay
crate-internal; `BodyShape::inflated` stays crate-private. Consumers updated:
`crates/tangle-sim/tests/*` (new `safety_events.rs`), `apps/tangle-cli`
(`trace.rs` record shapes and header, `tests/scenarios.rs`), `apps/tangle-tui` and
`apps/tangle-viewer` (explicit arms for the new kinds).

### Evidence

- Unit tests in `safety.rs`: the swept circle contains every pose of the tick; a
  pair's whole lifecycle edge by edge (band begin, contact begin ending the band,
  separation, band again, band end) with hand-computed clearances; the stale-pair
  close; a despawn closing queue and control states exactly once; a wait decision
  reporting one control transition; and a body crossing a region wholly inside
  one tick still reporting entry and then exit, with one violation on that entry
  edge.
- `crates/tangle-sim/tests/safety_events.rs`: the documented tie-breakers on
  hand-built equal-tick records; the mixed benchmark's stream ordered on every
  tick, reproduced for the same seed and divergent for another; pair edges checked
  against an independent swept query built from the observed frames, so a
  reported edge must be supported by the geometry; region edges alternating for
  both region kinds; queue and control edges alternating per agent with both modes
  reaching them; one red-light violation per crossing action with the recorded
  reason distinguishing a noncompliant run from an entry the vehicle could not
  brake out of; one crossing-against-signal violation per entry, and none for a
  compliant pedestrian.
- Mixed-benchmark evidence over the checked-in `mixed_interaction_v1` sweep (24
  seeds × 4000 ticks): seed 0 emits 2 collision, 147 near-miss, 73 entry, 71 exit,
  944 queue, and 72 control-transition records, all in the documented order, with
  both modes present in the entry, exit, queue, and collision or near-miss
  families; the whole sweep covers all six families.
- All five gates pass on the final tree: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check nodes`.

### Deferred

- `PHASE_1_PLAN.md` Increment 4's online TTC and minimum-separation tracking and
  the conflict-region occupancy intervals for PET are slice D; the region records
  the occupancy intervals are built from are already in place.
- The near-miss threshold (1.0 m) and the standstill speed (1e-3 m/s) are declared
  reporting constants, not calibrated measures; slice D's metrics might justify
  revisiting them with a recorded before/after.
- Region occupancy is a bounding-circle test, which can report entry up to
  `circumradius + |displacement| / 2` early; an exact box-versus-polygon test
  remains available if a consumer needs the tighter boundary.
- The cast bisects to a fixed resolution rather than carrying a
  continuous-advancement fast path; slice C adds one cast per close candidate
  pair per tick, and the Lipschitz certificate keeps that bounded.

## Slice D — online TTC, minimum separation, and PET occupancy intervals

Slice D adds the online interaction metrics the plan asks for: the simulator
itself reports how close its bodies came to a conflict, rather than a post-hoc
analysis of recorded trajectories. Like slices A–C it does not rewire the tick.
The pass observes the integrated state after the safety records are emitted and
before new demand is admitted, reads every input by reference, and emits no
event, so the walking trace golden, the scene golden, and the Phase 1 baseline
are byte-identical and no golden was regenerated.

### Definitions

`crates/tangle-sim/src/metrics.rs` owns the pass and its model card states the
definitions, units, applicability, and tie-breaks:

- **Time to collision** (`time_to_collision(first, second, step)`) predicts
  contact from the state the tick produced: the tick-end poses plus the tick's own
  displacement as each body's velocity. `Some(0.0)` when the pair already touches
  or overlaps — the overlap itself is carried by `Event::Collision`;
  `Some(seconds)` when the pair is closing and the predicted contact is within
  `TTC_HORIZON_S` (5 s); `None` when the pair is not closing (receding, parallel,
  or keeping a constant separation), when the predicted contact is beyond the
  horizon, or when the extrapolated paths' closest approach stays clear of
  contact, so a pair whose paths cross but pass clear reports no value. "Closing"
  is the sign of the clearance rate at the observed state, the shared
  `clearance_rate` the swept cast uses. The first contact fraction is a bisection:
  the relative translations at which two convex bodies touch form the Minkowski
  difference of their shapes, a convex set, so the touching fractions are a
  single interval. A cheap necessary condition first rejects pairs whose centres
  cannot come within the sum of their circumradii anywhere in the window, which
  is 89% of the observed candidate pairs on the mixed benchmark and keeps the
  pass bounded. Reported to `TTC_TIME_TOLERANCE_S` (1e-6 s).
- **Minimum separation** (`tick_minimum_clearance_m(first, second)`) is the least
  signed clearance over the tick from the same exact query the geometry layer
  exposes: both endpoints, plus the located closest approach when the pair closes
  and then separates inside the tick. A pair that stays disjoint has a signed
  clearance that is convex in the tick fraction, so the located turning point is
  the exact minimum; a pair that touches has one interval of contact and the
  located turning point is its entry, so a contacting pair never reports a clear
  separation. A mid-tick penetration deeper than that entry is not reported; the
  contact record carries the contact. Reported to `SEPARATION_RESOLUTION_M`
  (1e-6 m).
- **Candidate set**: a `SweptBroadPhase` over both bodies grown by half
  `INTERACTION_RANGE_M` (20 m) — the same slice-A/B machinery the safety pass
  uses, with the wider margin, so the safety pass's near-miss pair set is a
  subset of this one. Pairs are ascending `(AgentId, AgentId)` with
  `first < second`, the canonical pair spelling the collision records use. A pair
  that stays outside the range for a whole run has no recorded separation and no
  recorded time to collision.
- **Conflict-region occupancy intervals and PET**: occupancy edges are read from
  the existing `Event::Entry`/`Event::Exit` state for every `RegionKey` (crossing
  regions and authored conflict regions), so the module adds no second occupancy
  predicate and cannot disagree with `safety.rs` about when a body is in a
  region. An occupancy's boundaries are the ends of the ticks that reported them,
  so PET is tick-quantized and converges with the step; a despawn closes an open
  occupancy at its tick, because `Event::Despawned` closes the agent's stream. A
  `PostEncroachment` is recorded between two successive recorded occupancies of
  one region when they are by different bodies and do not overlap in time;
  overlapping occupancies have no post-encroachment time (the bodies were in the
  region together), and a re-entry is an ordinary occupancy that forms a
  succession with the occupancy before and after it. Occupancies are stored in
  close order — tick order, then the safety pass's ascending-`AgentId` order
  within a tick — so the successions, their records, and the minimum are
  deterministic. Every running minimum keeps the first value it saw on a tie.

### Public surface

`tangle_sim::{InteractionMetrics, MetricMinimum, ModePair, PostEncroachment,
RegionOccupancy, INTERACTION_RANGE_M, TTC_HORIZON_S, TTC_TIME_TOLERANCE_S,
SEPARATION_RESOLUTION_M, time_to_collision, tick_minimum_clearance_m}` and
`Simulation::interaction_metrics()`. `InteractionMetrics` exposes the tick and
run minima of time to collision and separation, the per-mode-pair and per-pair
separation minima (cross-mode is `ModePair::VehiclePedestrian`), the recorded
post-encroachments and the minimum PET, and one region's completed occupancies.
The pass itself and the bisection helpers it shares with `crate::swept` stay
crate-internal; `SweptBroadPhase::first_fraction` and `clearance_rate` became
`pub(crate)` so the metric reuses one definition of "still closing" and one
bisection instead of restating them. No event variant was added, so
`EVENT_VERSION` stays 2.

### Schema impact

None. No scenario-source or record schema changed, so schema version 1 is
unchanged and no schema regeneration or drift-test update was needed.

### Evidence

- Unit tests in `metrics.rs`: the analytic circle crossing (`3 - 1/sqrt(2)` s)
  and its invariance under a finer step; touching and overlapping pairs reported
  as zero; parallel, perpendicular, receding, passing-clear, and beyond-horizon
  pairs reported as not applicable; the located closest approach of a
  hand-computed pass-by and the endpoint cases of a monotone pair; the candidate
  grid covering an in-range pair and excluding a pair outside the range; and PET
  successions by hand — different bodies, same-body re-entry, an overlapping
  succession that records nothing, and a despawn closing an occupancy.
- `crates/tangle-sim/tests/metrics.rs` — six differential tests against
  references built from the observed frames:
  - a constant-speed lane (the walking skeleton) reports exactly the 15.5 m gap
    with no TTC, no PET, and no cross-mode record, identically at steps 0.05,
    0.0125, and 0.003125, and its per-pair and per-mode-pair records hold the
    same value;
  - 610 pedestrian-pair observations on `mixed_interaction_v1` at three steps
    match the closed-form quadratic first root of two moving circles to
    `TTC_TIME_TOLERANCE_S`, including agreement on applicability;
  - every recorded separation equals the all-pairs minimum over the observed
    frames to a nanometre, per mode pair as well as overall, whenever the closest
    pair is inside the range (step 0.05: 2.2454 m cross-mode among mode-pair
    minima 6.5214 / 2.2454 / 59.5511 m; step 0.0125: 14.6728 m cross-mode);
  - 175 post-encroachment records on `perpendicular_conflict_v1` over three steps
    and three seeds match an independently detected continuous occupancy
    (64 / 59 / 52 records per step), where both sides measure occupancy with the
    safety layer's conservative body radius: the worst deviation is 0.0925 s at
    step 0.05, 0.0233 s at 0.0125, and 0.0059 s at 0.003125 — proportional to the
    step and inside the declared 4-step bound at every step, so the metric
    converges as the step does;
  - the per-pair separation record equals the least of that pair's per-tick
    sweeps over 800 ticks of the conflict benchmark;
  - a run whose metrics are read after every tick is byte-identical in event
    stream and frame fingerprints to one whose metrics are never read.
- All five gates pass on the final tree: `cargo test --workspace --all-features`
  (including the canonical trace, its hash, the Phase 1 baseline, and the scene
  goldens, all unchanged), `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

### Measured cost

The pass is always on. On `mixed_interaction_v1` (about 15 candidate pairs per
tick) it adds roughly 22 µs per tick in a release build against roughly 7 µs for
the rest of the tick, so a run remains about 1750x faster than wall-clock; in an
unoptimized test build the same work is about 8x more expensive and the
benchmark-heavy suites are the ones that notice (the mixed-interaction suite goes
from about 7 s to about 23 s).

### Deferred

- Optimizing the pass: profiling candidates are the TTC bisections (the dominant
  cost), a per-pair clearance cache that would let the rate reuse the endpoint
  clearances it recomputes, and a coarser declared resolution. Increment 5's
  "profiler captures before optimization" owns that work.
- Whether the pass should be switchable off for throughput-sensitive runs is a
  coordinator decision: it would change `RunConfig`/provenance, and slice E does
  not need it.
- The minimum separation of a pair that contacts reports the contact entry rather
  than the deepest mid-tick penetration; a consumer that needs penetration depth
  should read the contact record's `clearance_m` or ask for an exact penetration
  query.
- The relevance range (20 m) and the TTC horizon (5 s) are declared reporting
  constants like the near-miss threshold, not calibrated measures.
- Region occupancy still uses the slice-C swept bounding circle, so a metric
  occupancy boundary can precede the exact body-versus-ring crossing by up to the
  sweep margin; the PET differential test measures that bound.
- PET is only defined for successive occupancies that do not overlap, so a region
  two bodies occupy together reports occupancy but no PET.
- Throughput/level-of-service metrics, metric definition versions, and
  disaggregation by movement are Increment 5 and 6 work, not this slice.

## Slice E — viewer overlays, inspector links, and F5 mode projection

Slice E closes the increment's presentation work: the typed safety records the
kernel already emits become the overlays a renderer draws, and the agent mode
reaches the projected body (F5). It adds no simulation behavior and does not
rewire the tick, so the canonical trace, its hash, the Phase 1 baseline, and the
renderer cell and Kitty goldens are unchanged.

### F5: the mode in the projected body

`SceneBody::project` now carries `mode: AgentMode` from the sample's
`MotionSample::mode` (the same seam the body dimensions come from), so a backend
styles a vehicle and a pedestrian distinctly from one frame. The previous
`kind: BodyKind` field was a hard-coded `Vehicle`, so every pedestrian projected
as a car; `BodyKind` is removed rather than kept as a second spelling of the
same concept, and a snapshot at `SnapshotDetail::Position` carries no mode and
keeps the vehicle fallback its dimensions use. Consumers updated: the Bevy
viewer spawns a shared disc mesh and pedestrian material for a pedestrian body
and a shared box and vehicle material for a vehicle (an agent's mode is fixed
for its slot, so an entity never changes either), the terminal inspector names
the mode, and the terminal rasterizers colour emphasized bodies.

### Overlay projection

`crates/tangle-present/src/safety.rs` folds the typed safety records into the
data a renderer draws. `PresentationController::observe_events(tick, events)` is
called once per completed step by every host; the fold keeps
`MARKER_LIFETIME_SECONDS` (2.0 s) of records converted to whole ticks at the
run's step, plus the open states the edges open and close: region occupancy
(`Entry`/`Exit`, and a despawn, which the safety pass closes without an exit
record), standstill (`Queue`), and controller state (`ControlTransition`).
`project` windows that state to the frame's own tick, so a frame carries
exactly the records inside its window and a backend reads nothing else.

Derived per frame, all from the frame alone:

- `safety_markers()` — one marker per retained record, in record order, anchored
  on its region's ring centre or on the midpoint of the participants alive in
  the frame; a record whose bodies have all left and which names no region draws
  no marker.
- `body_emphasis()` — one style per emphasized body, ascending by agent id, with
  precedence collision, near miss, violation, queue, control transition, so a
  body in contact is not also reported as queued. Contact, near-miss, and
  violation emphasis fade with the marker window; queue and controller emphasis
  last exactly as long as their state.
- `occupied_regions()` — regions bodies currently occupy, ascending by
  `RegionKey`, occupants ascending.
- `events_involving(agent)` and `EventParticipants::of(event)` — the inspector
  link: the records a body took part in, in window order, each with its
  ascending participant agent ids and its `RegionKey` (a yield carries
  `RegionKey::Crossing`, so one key spelling covers every region-bearing
  record). `event_summary(record)` is the shared wording both inspectors print.
- `SceneGeometry::region_points` / `region_center(RegionKey)` — the authored ring
  and its vertex mean, so an occupancy overlay covers the region the safety
  layer crossed and keeps the crossing and conflict-region id spaces apart.

Every derivation is pure and deterministic: no time, randomness, or hidden
state, no hash-map iteration, and ascending order with the kernel's own record
order as the tie-breaker, so replaying a run projects byte-identical overlays.
`Overlays` gains `safety` (default on) and `Overlay::Safety`; the Bevy viewer
binds `B`, the terminal binds `b`.

### Viewer wiring

The Bevy viewer draws the occupied-region ring, a ring per emphasized body in
the style's colour, a link ring on the other participants of the selected
body's records, and a marker per record, and its inspector names the mode and
lists the selected body's records with their participants. The terminal backend
rasterizes the same overlays: an emphasis colour per body, the occupied ring,
and one glyph per marker kind (in both the cell and Kitty pixel paths), and the
one-line footer names the mode and the body's most recent record.

### Public surface

`tangle_present::{FrameEvent, EventParticipants, BodyEmphasis, OccupiedRegion,
SafetyMarker, SafetyOverlay, event_summary, is_safety_record,
MARKER_LIFETIME_SECONDS}`, `SceneBody::mode`, `SceneBody::project` (unchanged
signature), `SceneFrame::safety` with `safety_markers`, `body_emphasis`,
`occupied_regions`, `events_involving`, `region_points`,
`SceneGeometry::{region_points, region_center}`, `Overlay::Safety`,
`Overlays::safety`, and `PresentationController::{observe_events, safety}`.
`BodyKind` is removed. No dependency changed: `tangle-present` already depended
on `tangle-model` and `tangle-sim`, no Bevy type entered the kernel or the
presentation layer, and `f64`/`glam::DVec2` are unchanged.

### Golden impact (deliberate)

`tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt` is regenerated
because the projected frame's shape changed; the whole diff is:

- each of the six bodies: `kind: Vehicle` → `mode: Vehicle`;
- `overlays` gains `safety: true`;
- the frame gains `safety: SafetyOverlay { events: [], marker_lifetime_ticks:
  40, occupied: [], queued: [], controlling: [] }` (40 ticks is 2.0 s at the
  0.05 s standard step).

No other golden changed: the trace golden and hash, the Phase 1 baseline, the
cell grid, and the Kitty byte fixtures all pass without regeneration, and the
view-command golden is unchanged because it prints named view fields rather than
the frame.

### Evidence

- `crates/tangle-present/src/scene.rs`: `SceneBody::project` projects
  `AgentMode::Vehicle` and `AgentMode::Pedestrian` from a sampled `MotionSample`
  and the vehicle fallback without motion detail; a `RegionKey` resolves to its
  own ring in its own id space and `region_center` is the ring's vertex mean;
  `Overlay::Safety` toggles independently.
- `crates/tangle-present/src/safety.rs`: the participants of every record kind
  (spawned, despawned, yield, contact, near miss, violation, entry, exit, queue,
  control transition), including a yield's crossing key and a conflict-region
  key; `others` as the jump targets; the fold opening and closing occupancy,
  queue, and controller states, including a despawn clearing every state without
  a closing record; the marker window dropping records older than the lifetime
  and `windowed_at` filtering without a newer observation; markers anchored on a
  region centre, on a live participant, and on the survivor of a departed pair,
  with a departed body drawing none; one precedence-ordered emphasis per body;
  and `event_summary` wording for every record kind.
- `crates/tangle-present/src/controller.rs`: observed records reach the projected
  frame, projection is a pure read, a record past the window stops drawing a
  marker while its open queue state persists, and a restart clears the run's
  records.
- `crates/tangle-present/tests/safety_overlays.rs` over the checked-in
  `mixed_interaction_v1` benchmark at seed 3 for 800 ticks: two runs project
  byte-identical overlays per tick; every marker is backed by a record the frame
  carries and anchored on its region centre or inside its live participants;
  emphasis names live bodies, ascending, at most once each; occupancy names
  geometry regions and live occupants, ascending; every inspector link resolves
  to a record and to bodies the frame draws or records; both modes project
  through the same frames.
- Real-stream evidence over that run: 7 yield, 15 near-miss, 14 entry, 11 exit,
  115 queue, and 13 control-transition records fold into 6180 markers across 589
  frames, occupancy in 545 frames, and 2986 body-emphasis entries (2054 control
  transition, 849 near miss, 83 queue) with both modes present; the largest
  retained window is 37 records, inside the 40-tick lifetime.
- Falsification: three deliberate mutations each fail a named test — forcing
  the projected mode to `Vehicle` fails the mode test, a marker window with no
  lifetime fails the window test and the restart test, and a marker anchor that
  ignores the region centre fails the anchor test and the real-stream overlay
  test.
- All five gates pass on the final tree: `cargo test --workspace --all-features`
  (all 40 test binaries, including the regenerated scene golden, the trace
  golden and hash, the Phase 1 baseline, and the renderer goldens),
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check nodes`.

### Deferred

- Event selection is a body-first UI: an inspector picks a record through a body
  that participates in it (or through a marker, which exposes the same
  `EventParticipants`). A pointer-picks-an-event interaction is not implemented;
  the projection it would consume is.
- Overlays show the event and occupancy side of the online metrics; the numeric
  TTC, minimum-separation, and PET values slice D records are not yet drawn in a
  viewer panel.
- A marker is drawn at full strength for its lifetime and then disappears; a
  fade or a per-kind lifetime is a presentation nicety with no consumer yet.
- The Bevy viewer draws markers as circles and the terminal as one glyph per
  kind, so the two backends agree on position, colour family, and kind but not
  on the exact silhouette; a character cell affords only a glyph.

## Slice G — review-finding closeout

Slice G closes the three P2 test-quality findings of the independent read-only
slice-F verification. No simulation behavior, schema, public presentation
surface, or golden changed. The one non-test change is the terminal emphasis
gate recorded under F3, which the new test found and which aligns the terminal
with `Overlay::Safety`'s own recorded contract.

### F1 — unfalsifiable candidate assertion in `geometry_queries.rs`

`candidate_pairs_never_exceed_the_enclosing_box_test` asserted
`bounds_gap_m(&first_bounds, &second_bounds) <= QUERY_TOLERANCE_M` after already
asserting the two boxes overlap, and `bounds_gap_m` clamped each axis gap at
zero, so the value was always exactly `0.0` and the assertion could not fail.
The finding's preferred replacement — bounding a rejected candidate's signed
clearance by the contact band — is false, and was measured false before being
implemented: a broad phase is an enclosing-box filter, not a near-contact
filter, and on the fixed world sweep seed 2 has rejected candidate `(1, 2)` at
`0.3362705201374694 m` of real clearance. Slice G instead deletes the vacuous
assertion and the now-unused `bounds_gap_m` helper, corrects the doc comment to
state the true invariant and to record explicitly that a rejected candidate
carries no clearance bound, and replaces the deleted line with a completeness
check against the independent brute-force predicate: every pair
`reference_intersects` accepts must be in the candidate set. The test now
asserts the sandwich property — exact intersections are a subset of the
candidates, and the candidates are a subset of the box-overlap relation — so
every assertion in it is falsifiable.

Falsification (each mutation run against the fixed test, then reverted):

- returning only non-overlapping pairs from `BroadPhase::candidate_pairs` fails
  the box-overlap assertion: `candidate (0, 1) must have overlapping enclosing
  boxes: seed 1`;
- returning no pairs at all fails the completeness assertion: `intersecting pair
  (0, 12) must be a candidate: seed 1`.

### F2 — always-true disjunction in `safety_events.rs`

`pair_records_match_an_independent_swept_query` asserted `!near_this_tick ||
contact_this_tick` inside `if contacting`, where `contact_this_tick` had just
been asserted equal to the true `contacting` flag, so the disjunction was always
true. Slice G removes it, records each pair's *begin* keys in two sets
(`contact_begins`, `near_miss_begins`), and asserts after the tick loop that the
sets are disjoint: no pair may begin a contact and a near miss in the same tick.
Reading only the emitted stream makes the check order-independent, strictly
stronger than the in-loop order-dependent near-miss/contact cross-check it
replaced. The per-tick band-predicate assertions that carry the emission rule
are unchanged.

Falsification: a mutation that pushes a `NearMiss { entering: true }` from the
contact branch fails the unchanged band-predicate assertion, and with that
assertion temporarily masked the new structural check fires on its own:
`pair(s) began a contact and a near miss in the same tick: [(15, 16, 1222)]`.

### F3 — no app-layer overlay test, and the wiring defect it caught

The finding's second half was the real gap. The pure overlay derivation the apps
consume was already covered in `tangle-present` — `src/safety.rs` unit tests
over synthetic records, `src/controller.rs` tests through the controller, and
`tests/safety_overlays.rs` over a real `mixed_interaction_v1` stream — and the
terminal inspector link was already covered by `hud.rs`'s
`the_inspector_reports_the_latest_decision_reason`, which reads
`events_involving` through `Hud::describe`. What no test under `apps/` exercised
was the mapping from projected overlays to draws. Slice G adds it at all three
backends:

- **Viewer.** `draw_safety_overlays` is split so the overlay-to-shape mapping is
  a pure `safety_overlay_shapes(&SceneFrame) -> Vec<SafetyOverlayShape>`
  (`Ring`/`Circle` in world metres) that the Bevy system then renders. The drawn
  picture is unchanged: the occupied ring, one emphasis ring per body, one link
  ring per record participant of the selected body, and markers last, all
  suppressed while `Overlays::safety` is off. Three tests over a two-vehicle
  crossing frame assert the exact shape plan (order, palette, radii), the
  overlay switch, and the link ring for either selected body.
- **Terminal cell backend.** `raster.rs` gains two tests that rasterize such a
  frame and assert the occupied-region ring, the contact and standstill marker
  glyphs in their own colours, and an emphasized body in its emphasis colour,
  and that `Overlays::safety = false` removes them while the body keeps drawing.
- **Terminal pixel backend.** `pixel.rs` gains the same pair of tests over a
  synthetic two-body frame, so the Kitty path's parallel `draw_safety` is
  covered too.

The cell-backend test found a real wiring defect. Both terminal backends
computed `SceneFrame::body_emphasis` outside the `if frame.overlays.safety`
guard, so `b` removed the markers and the occupied ring but left body-emphasis
colours on, while the Bevy viewer and `Overlay::Safety`'s recorded contract
("Safety markers, body emphasis, and region occupancy") gate all three. Slice G
gates the terminal emphasis map on `overlays.safety` in both backends. Nothing
changes while the overlay is on, and no golden exercises safety-off, so no
golden was regenerated.

Falsification: reverting the emphasis gate fails
`the_safety_overlay_can_be_disabled`, and giving `BodyEmphasis::Collision`
another colour fails the viewer plan test. The Bevy ECS wiring that consumes
`safety_overlay_shapes` — the `Gizmos` calls and the system registration — stays
source-verified-only: it holds no mapping a test could falsify without a live
Bevy context.

### Evidence

- `cargo test --workspace --all-features`: all 40 test binaries pass, including
  the canonical trace and its hash, the Phase 1 baseline, the scene, cell, and
  Kitty goldens, all without regeneration.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check nodes` pass on the final tree.
- Dependency direction is unchanged: no crate gained a dependency, `tangle-model`
  and `tangle-sim` still carry no Bevy type, and `f64`/`glam::DVec2` are
  unchanged.

# Final verification

Every `# Done when` criterion is met on the integrated tree (HEAD `9074df3`,
verified after the slice-G closeout). The orchestrator reran the five gates on
the final tree: `cargo test --workspace --all-features` exit 0 with no failed
test binary; `cargo clippy --workspace --all-targets --all-features -- -D
warnings` exit 0; `cargo fmt --all --check` clean;
`./scripts/check-dependency-direction.sh` reports `dependency direction OK`; and
`braintree check nodes` passes (54 nodes). An independent read-only review
(slice F) verified each criterion against the source and tests and raised no
merge blocker; slice G closed its three P2 test-quality findings (two
unfalsifiable assertions replaced, and app-layer overlay tests added, which also
caught and fixed a terminal emphasis-gating defect).

Per criterion:

1. **Property tests compare indexed candidates with an all-pairs reference on
   randomized small worlds.** `crates/tangle-sim/tests/geometry_queries.rs`:
   `broad_phase_candidates_cover_every_overlapping_pair` (12 fixed seeds x
   16-body random worlds) compares the broad phase with the independent all-pairs
   BTreeSet reference `reference_overlapping_bounds`, and
   `exact_queries_match_the_brute_force_reference` covers the narrow phase. Slice
   G made every assertion falsifiable: the test now asserts the sandwich (exact
   intersections are a subset of the candidates, and the candidates are a subset
   of the box-overlap relation), with two recorded mutations that fail it.
   Evidence: `crates/tangle-sim/src/query.rs` and `src/index.rs` unit tests.
2. **Swept fixtures detect crossing bodies that do not overlap at either tick
   endpoint.** `crates/tangle-sim/tests/swept_queries.rs` covers a fast box
   through a thin wall (`!bodies_intersect` at both endpoints, time of impact
   0.29), a circle crossing at 1/3, a diagonal crossing against its analytic time
   and normal, and 20 narrow-window crossings, all against
   `crates/tangle-sim/src/swept.rs`; `src/index.rs` has a unit test that the
   static phase pairs nothing while the swept phase pairs the crossing body.
3. **Event pairs have deterministic ordering and are emitted once according to a
   documented lifecycle.** The lifecycle is documented at
   `crates/tangle-sim/src/event.rs:32`; `EventKind` (`event.rs:81`) and
   `order_key` (`event.rs:391`) give the stable order, sorted once per step at
   `sim.rs:752`; emission predicates live in `src/safety.rs` (`:13`, `:41`).
   Tests: `crates/tangle-sim/tests/safety_events.rs` tie-breaker, per-tick order,
   once-per-transition, and stream-wide tests (`:217`, `:287`, `:377`, `:486`,
   `:528`), plus the 24-seed x 4000-tick mixed stream test in
   `mixed_interaction.rs`.
4. **Fine-step differential tests agree with analytic/simple fixtures within
   declared tolerances.** `crates/tangle-sim/tests/metrics.rs` checks the
   analytic 15.5 m lane at steps 0.05/0.0125/0.003125 (`:322`), a closed-form
   quadratic circle TTC (`:372`), and PET against an independently detected
   continuous occupancy over three steps and three seeds (`:526`, worst
   deviation inside the declared four-step bound). Tolerances are declared in
   `crates/tangle-sim/src/metrics.rs`.
5. **`EVENT_VERSION` describes the full event union (F4), and the agent mode is
   carried through `Event::Spawned` and `SceneBody::project` (F5).** F4:
   `EVENT_VERSION = 2` (`event.rs:66`) names the full 10-variant union
   (`event.rs:210`), which `kind()`/`agent()`/`order_key()` cover exhaustively
   with no wildcard. F5: `Event::Spawned { mode, .. }` (`event.rs:213`) is in the
   trace (`apps/tangle-cli/src/trace.rs:68,240`) and `SceneBody::project`
   (`crates/tangle-present/src/scene.rs:679-681`) with mode tests at
   `scene.rs:932`. The walking trace golden, its hash, the scene golden, and the
   Phase 1 baseline were deliberately regenerated for this change; no unrelated
   golden moved.
6. **Viewer overlays and inspector links connect an event to its participants.**
   `crates/tangle-present/src/safety.rs` exposes `EventParticipants::of`,
   `others`, `events_involving`, `safety_markers`, `body_emphasis`,
   `occupied_regions`, and `event_summary`; the Bevy viewer
   (`apps/tangle-viewer/src/main.rs`) draws the occupancy ring, emphasis and link
   rings, and markers and lists a body's records; the terminal backends draw the
   same overlays (`b`). Evidence: `crates/tangle-present/tests/safety_overlays.rs`
   over the real 800-tick mixed stream, the slice-G viewer shape-plan tests, and
   the new terminal-backend tests.
7. **Online TTC and minimum-separation tracking, and conflict-region occupancy
   intervals for PET, have evidence.** `crates/tangle-sim/src/metrics.rs` owns
   `time_to_collision`, `tick_minimum_clearance_m`, and the occupancy/PET state
   (`:316`, `:414`, `:720`), wired into the tick at `sim.rs:743` and readable via
   `Simulation::interaction_metrics()`. Evidence: the `metrics.rs` unit tests and
   the six differential tests in `tests/metrics.rs`.
8. **Five gates on the final tree.** Rerun by the orchestrator; all pass (see
   above).

# Limitations

All are accepted scope boundaries or recorded residuals, not failures of the
criteria above:

- **Swept queries are a capability, not yet a tick consumer.**
  `SweptBroadPhase` and `time_of_impact` are exercised by the safety and metrics
  passes and their fixtures, but the tick still rebuilds the static
  `SpatialIndex` (`sim.rs:718`) and no trajectory (yielding or the anti-overlap
  caps) consumes a cast. Tunneling protection is available to consumers; wiring
  it into motion would change behavior and the goldens and is left to a later
  increment.
- **Geometry is dependency-free on `glam`.** `query.rs` and `swept.rs` implement
  box/box, circle/circle, box/circle distance, intersection, and casts directly
  on `f64`/`glam::DVec2`. `PHASE_1_PLAN.md` names `parry2d-f64` as an option, but
  the f64/glam constraint was preferred and no new dependency was added.
- **Region occupancy uses a swept bounding circle.** A metric occupancy boundary
  can precede the exact body-versus-ring crossing by up to the sweep margin; the
  PET differential test measures that bound rather than eliminating it.
- **PET is defined only for non-overlapping successions.** Two bodies that
  occupy a conflict region together report occupancy but no post-encroachment
  time.
- **Contacting-pair minimum separation is the contact entry.** A pair that
  contacts reports its contact-entry clearance, not the deepest mid-tick
  penetration; a consumer that needs penetration depth reads the contact
  record's `clearance_m`.
- **The online metrics pass is always on.** It costs roughly 22 microseconds per
  tick in a release build on `mixed_interaction_v1` (about 8x more in an
  unoptimized test build). Whether it should be switchable is deferred to
  Increment 5, which owns profiling before optimization.
- **Reporting constants are declared, not calibrated.** The 20 m relevance
  range, 5 s TTC horizon, 1 m near-miss threshold, 1e-3 m/s queue stop speed,
  and 2 s marker lifetime are documented choices, not measured measures.
- **Event selection is body-first.** An inspector reaches a record through a
  participating body or a marker; pointer-picks-an-event is not implemented.
  Overlays show events and occupancy, not the numeric TTC/separation/PET values.
- **Bevy ECS wiring is source-verified only.** The pure overlay-to-shape plan is
  tested, but the `Gizmos` calls and system registration hold no mapping a
  headless test can falsify.
- **Event-record version.** `EVENT_VERSION` was deliberately bumped 1 -> 2 so it
  describes the union; consumers keying on version 1 must migrate. The walking
  trace golden, its hash, the scene golden, and the Phase 1 baseline were
  regenerated deliberately; the cell and Kitty goldens legitimately stayed
  identical because the walking run emits no safety record.
- **TAS-029 red-queue emergency-cap residual** remains a separate P2 node under
  `IDX-001`; it does not gate this increment.
