---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:54Z
summary: Same-tick event order follows the dense facility index, so declaration order moves it.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for Increment 2 declaration-order invariance and the
event stream (TAS-132). Verified at commit 98ba8d3.

# Finding

`Event::order_key` (`crates/hekate-sim/src/event.rs:679`) carries a facility
component derived by `facility_key` (`event.rs:813`), which is the optional
facility's dense array index plus one. Two records of the same agent and the
same kind within one tick are therefore ordered by that index, and the dense
index follows the scenario's declaration order, so reversing two facilities'
declarations changes the within-tick emission order of those records.

# Evidence

TAS-132's
`reversing_facility_and_reference_declarations_preserves_the_wrong_way_outcome`
(`crates/hekate-sim/tests/wrong_way.rs`) reverse-compiles
`scenarios/phase2/inc2/narrow_wrong_way_v2.json5` and finds every physical
quantity bit-identical — agent-id allocation, span and despawn ticks, facility
handoffs, contacts and near misses, entered/refused decisions, and every pose
and route-relative state — while exactly one thing moves: the within-tick
emission order of two same-agent `Event::OpposingTraversal` records (agent 9's
`a` close and `a_left` open swap at tick 1083; agent 10's at tick 1170). The
tick's record *set* is unchanged, so the test canonicalizes boundaries by
stable name and `event.rs` is left untouched.

# Consequence

The order is deterministic for a fixed scenario but declaration-dependent and
non-physical: either declaration order is admissible, so a consumer must not
treat the within-tick order of same-agent same-kind records as a physical
relation. A declaration-order invariance test over a tick must compare record
sets or canonicalize by stable name.

# Seam

`crates/hekate-sim/src/event.rs` (`Event::order_key`, `facility_key`),
`crates/hekate-sim/tests/wrong_way.rs`.
