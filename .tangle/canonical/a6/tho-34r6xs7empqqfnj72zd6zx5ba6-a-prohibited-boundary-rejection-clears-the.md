---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:53Z
summary: A prohibited-boundary rejection clears the intent but emits no maneuver event.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 maneuver event surface (TAS-099,
TAS-129, TAS-133). Verified at commit 98ba8d3.

# Finding

`attempt_facility_transition` returns `Attempt::BoundaryForbidden`
(`crates/hekate-sim/src/sim.rs:2572`) when the destination traversal is not
permitted by the mode's policy. The maneuver decision loop handles it by
clearing the intent and recording
`maneuver_reason: Some(ManeuverReason::BoundaryForbidden)` with
`transition: None` (sim.rs:1384-1391).

`Event::Maneuver` is emitted only per recorded transition edge
(`sim.rs:1703`, `fn maneuver_event`), and `ManeuverEdge`
(`crates/hekate-sim/src/stage.rs:176`) has no rejection variant, so this path
emits no event: the recorded stream shows the maneuver as never attempted. The
reason survives only in the live route state, reachable through
`Simulation::lateral_maneuver_reason` (sim.rs:874) and
`RouteState::maneuver_reason` (`crates/hekate-sim/src/agent.rs:189`), which no
trace, golden, or replay reads.

# Evidence

`apps/hekate-cli/tests/inc2_trace.rs:268-271` states it in the golden suite's
own words: "The prohibited-boundary variant's rejection is an inspectable
route-state reason, not a stream record ... `Attempt::BoundaryForbidden`
discards the intent and records the reason with no transition", so the golden
proves the prevented crossing only "by the absence of any maneuver edge and any
handoff". `crates/hekate-sim/tests/inc2_passing_fixtures.rs:689-698` asserts
the same in-process (`trace.edges.is_empty()`).

# Consequence

A prohibited-boundary rejection is invisible in the recorded event stream, so a
trace consumer cannot distinguish "no maneuver was ever wanted" from "a
maneuver was rejected because the boundary is forbidden". Proving the
rejection from a trace requires the negative assertion, not a record.

# Seam

`crates/hekate-sim/src/sim.rs` (`Attempt`,
`attempt_facility_transition`, the `Attempt::BoundaryForbidden` arm of the
maneuver decision loop, `maneuver_event`,
`Simulation::lateral_maneuver_reason`),
`crates/hekate-sim/src/agent.rs` (`RouteState::maneuver_reason`),
`crates/hekate-sim/src/stage.rs` (`ManeuverEdge`),
`crates/hekate-sim/src/event.rs` (`ManeuverReasonCode::BoundaryForbidden`),
`apps/hekate-cli/tests/inc2_trace.rs`,
`crates/hekate-sim/tests/inc2_passing_fixtures.rs`.
