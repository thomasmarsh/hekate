---
context_rev: 1
status: resolved
updated: 2026-09-15T23:33:11Z
summary: Segment-level broad/narrow-phase collision for articulated chains.
---

Parent [[tas-5vc5c3cbttnvcztafvns26bcs4-add-hitch-integration-runtime-dispatch-and]].

# Context

Depends on the segment-pose chain the hitch-integration slice adds to the per-agent runtime state; today collision and broad-phase code (`crates/hekate-sim/src/swept.rs` and its broad-phase caller) assume one shape per agent.

# Outcome

Broad phase indexes one proxy per chain or one per segment for an `ArticulatedWheeled` agent; narrow phase reports the specific contacting segment pair (agent, segment index) rather than the whole chain, so a contact event names which segment touched.

# Done when

- Broad phase never misses a contact a single enclosing proxy would have caught, and narrow phase resolves the specific segment pair for an articulated-chain contact.
- A fixture/test proves a contact is detected on a non-lead segment (e.g. the trailer, not the tractor) that a single-proxy check would miss or misreport.
- Existing box/circle/capsule collision fixtures and event shapes are unchanged.
- The five gates pass.

# Result

`crates/hekate-sim/src/safety.rs::SafetyMonitor` no longer indexes or scans a
chain agent by its lead segment alone. Three new private helpers do the work:
`agent_segments` (every segment's exact `BodyShape`, segment 0 first, reusing
`query::agent_body` for the lead and `ArticulatedState::segments()`/
`trailers()` for each trailer); `agent_broad_phase_shape` (the indexing proxy
`begin_tick`/`index_bodies` now build from, in place of the old bare
`query::agent_body` call: identity for a non-chain agent, one circle
enclosing every segment's AABB for a chain, so the swept broad phase can
never miss a contact any segment could make); and `min_segment_clearance`
(the minimum exact clearance, and the two segment indices that achieve it,
between every segment of one side and every segment of the other).

`scan_pair`/`close_stale_pairs` now take `agents: &AgentStore` and branch: a
pair where neither side is an `ArticulatedWheeled` chain keeps the exact
pre-existing swept clearance and `time_of_impact`/`band_entry` cast,
byte-identical to before this change, so every pre-existing box/circle/
capsule fixture is unaffected. A pair where either side is a chain instead
resolves through `agent_segments`/`min_segment_clearance` using only exact,
tick-end (static) geometry, with `contact = clearance_m <= 0.0` and
`near = !contact && clearance_m <= NEAR_MISS_THRESHOLD_M`, no swept cast.
This is the deliberate, temporary precision gap the node scoped out: sub-tick
swept precision for an individual chain segment is `tas-78ney4232et57z60w345873x43`
("swept collision queries for multi-segment bodies"), the next sibling node,
not this one. On the contact edge, a chain-involved pair pushes the new
`Event::ArticulatedSegmentContact { agent, agent_segment, other,
other_segment, clearance_m, contacting }` instead of `Event::Collision`;
`agent_segment`/`other_segment` are `Some(index)` for a chain side (segment
`0` = lead) and `None` for a non-chain side. `Event::NearMiss` is unchanged
in shape and still emitted for a chain pair's near-miss edge, only now with
an accurate clearance instead of a silently lead-only one.

`crates/hekate-sim/src/articulated.rs::ArticulatedState` gained one read-only
accessor, `segments()`, returning `&[ArticulatedSegmentGeometry]` (mirroring
the existing `trailers()`). Per the repo's coordination convention, this is
an internal, non-behavioral reuse addition in a resolved sibling node's
module (`tas-12mx01cfsxskm0pjzq13hvcm2g`) and is authored here without
escalation: it changes no artifact byte, no public API beyond one new
`pub(crate)` getter, and no behavior.

`crates/hekate-sim/src/event.rs` gained `Event::ArticulatedSegmentContact`
and `EventKind::ArticulatedSegmentContact`, appended last in both enums
(after `ArticulationLimitExceeded`), with `agent()`/`kind()`/`order_key()`
arms mirroring `Collision`'s pair-key shape. It lands under the same
unbumped `EVENT_VERSION` 3, for the same reason `ArticulationLimitExceeded`
did: a purely additive variant no scenario compiled before
`ArticulatedWheeled` existed can ever emit.

The required proof is `crates/hekate-sim/src/safety.rs`'s new unit test
`a_trailer_segment_contact_is_detected_and_named`: it spawns a
`tractor_semitrailer` chain (6.0 m/2.5 m tractor, 13.6 m/2.55 m trailer,
1.2 m kingpin setback, matching `articulated.rs`'s own fixture), recomputes
the trailer's spawn position from the live `ArticulatedState` rather than
trusting hand arithmetic, places a 2 m x 2 m obstacle at (-10, 0) that
overlaps only the trailer's box, asserts the old lead-only proxy
(`query::agent_body` + `query::bodies_intersect`) misses the contact
entirely, then runs the real `begin_tick`/`observe` pass and asserts it
emits `Event::ArticulatedSegmentContact { contacting: true, agent_segment:
Some(1), .. }` naming the chain and the obstacle, with no plain
`Event::Collision` for that pair.

Every exhaustive `match`/`matches!` over `hekate_sim::Event`/`EventKind`
across the workspace needed the new arm, found by `cargo build --workspace
--all-features` (compiler-forced, in the write set per the repo's
coordination convention): `apps/hekate-cli/src/run_metrics.rs`
(`counted_family`, `event_agent`, plus a non-forced parity addition to the
`pending_conflicts` push alongside `Collision`/`NearMiss`), `apps/hekate-cli/
src/trace.rs` (`EventRecord` gained `agent_segment`/`other_segment` fields
appended last, a `From`-style match arm, and a pinned serialization test
case), `apps/hekate-cli/tests/scenarios.rs`, `apps/hekate-tui/src/raster.rs`
(`marker_glyph`/`marker_color`, same glyph/color as `Collision`),
`apps/hekate-tui/src/session.rs`, `apps/hekate-viewer/src/main.rs` (event
loop join, plus `marker_color`'s `EventKind` match, same color as
`Collision`), `crates/hekate-present/src/safety.rs` (`is_safety_record`,
`EventParticipants::of`, `event_summary`, `SafetyOverlay::observe`,
`body_emphasis`), `crates/hekate-present/src/tactical.rs`, and
`crates/hekate-sim/tests/{pedestrian_flow,vehicle_flow}.rs`. No compiler-
forced site required changing another node's own behavior, only an
additive passthrough arm.

Final gates on the resolved tree: `cargo test --workspace --all-features`
green, 0 failed (including the new `safety::tests::
a_trailer_segment_contact_is_detected_and_named`); `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean; `cargo fmt --all
--check` clean; `./scripts/check-dependency-direction.sh` OK; `tangle check`
clean. `index.rs` (`BroadPhase`/`SweptBroadPhase`) and its other consumers
(`close_pass.rs`, `metrics.rs`, `prediction.rs`, `sim.rs`'s `SpatialIndex`)
were not touched, as scoped: they stay keyed one-shape-per-`AgentId`.
