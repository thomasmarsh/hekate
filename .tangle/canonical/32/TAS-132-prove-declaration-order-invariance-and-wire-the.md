---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T06:06:00Z
summary: Prove declaration-order invariance and wire the matrix.
---

Parent [[TAS-107-check-in-contextual-wrong-way-fixtures]].

# Outcome

Reversing facility and reference declarations preserves the physical wrong-way
outcome, and the fixtures replace only their planned benchmark-matrix entries.

# Done when

- Reversing facility and reference declarations does not change the physical
  outcome after stable IDs are accounted for.
- Fixture paths replace only their planned entries in both benchmark-matrix
  representations; no interaction disposition or tolerance is widened.
- CLI validate and run plus cargo test --workspace pass at required presets.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Extends [[TAS-107-check-in-contextual-wrong-way-fixtures]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns declaration
invariance and the matrix wiring; the core fixtures are the sibling slice.

# Result

Reversing the checked-in fixture's facility and reference declarations preserves
every physical quantity, and the wrong-way fixture's own matrix entry is wired;
no other matrix cell was touched and no tolerance or disposition was widened or
narrowed.

- `crates/hekate-sim/tests/wrong_way.rs` gains
`reversing_facility_and_reference_declarations_preserves_the_wrong_way_outcome`,
a test-level variant of the TAS-131 scenario: it parses
`scenarios/phase2/inc2/narrow_wrong_way_v2.json5` itself and reverses only its
`paths` and `facilities` arrays, leaving demand order (which fixes agent-id
allocation) and every random stream (keyed by demand-source index or agent id)
unchanged. Both compiles are driven 6000 steps by the identical corridor-keyed
wrong-way driver and their outcomes compared after mapping every compiled index
to its stable name.
- Bit-identical under the reversal: agent-id allocation, all spawn and despawn
ticks, facility handoffs, the collision scan's contacts and near misses, the
entered/refused decisions on all four corridors, and every agent's world pose and
route-relative state at all 6000 steps. The comparison is not vacuous: all four
corridors reach the decision and the occupied corridor reaches its contact.
- **One thing the reversal does move, and it is explicitly not treated as a
physical difference:** the within-tick *emission order* of two same-agent
`Event::OpposingTraversal` records. At tick 1083 agent 9's `a` close and `a_left`
open swap places; at tick 1170 agent 10's `b` close and `b_left` open swap
places. The cause is `crates/hekate-sim/src/event.rs`'s `order_key`, which sorts
same-agent same-kind records by `facility_key` — the optional facility's dense
array index plus one, which the scenario's declaration order legitimately
controls. The ticks' record *sets* are unchanged, so the test canonicalizes each
tick's boundaries by stable name before comparing; `event.rs` and every other
resolved seam are untouched. Confirmed with the parent as a dense-index artifact
rather than a physical outcome.
- Matrix wiring only: the `narrow_wrong_way_v2` slug moved from the `### 7.3
Planned fixtures` table into `### 7.2 Checked-in fixtures` in
`docs/benchmark-matrix.md`, and its path moved from `fixtures.planned_patterns`
into `fixtures.checked_in_increment_2` in `docs/benchmark-matrix.json`. No other
row, cell, disposition, or tolerance changed.

Verified: `cargo fmt --all` clean; `cargo clippy -p hekate-sim --all-targets
--all-features -- -D warnings` clean; the fixture CLI-validates and runs at seed
0 with trace hash `fab298dccfdff4a91f2a0f9ad318b654724c711c97f3ee55c5a84affcfe5fb6b`;
`cargo test -p hekate-sim wrong_way` 3 passed; `cargo test -p hekate-cli --test
migration_regression` 4 passed; `./scripts/check-dependency-direction.sh` reports
`dependency direction OK`.

Recorded as delegated and not claimed: the `# Done when` clause requiring
`cargo test --workspace` and the required presets was not executed here, because
this slice's brief scopes verification to the focused commands above. Per-preset
reproduction is [[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]]'s
scope; no workspace or preset coverage is claimed.

## Gate evidence

Fresh `gate132` run from the repository root on `86b9d08`:

| gate | command | exit | wall s | result |
| --- | --- | --- | --- | --- |
| format | `cargo fmt --all --check` | 0 | 1.0 | clean |
| lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1.0 | clean |
| test | `cargo test --workspace --all-features` | 0 | 184.0 | 1005 passed, 0 failed, 1 ignored |
| dependency direction | `./scripts/check-dependency-direction.sh` | 0 | 0.0 | `dependency direction OK` |
| graph | `tangle check` | 0 | 0.0 | `graph check: passed (194 nodes)` |

The gate's workspace run executes the new
`reversing_facility_and_reference_declarations_preserves_the_wrong_way_outcome`
and it passes (`tests/wrong_way.rs`, 20 passed). No fix was needed for any gate.

Preset clause, recorded as delegated and not claimed: the required-preset half
of the `# Done when` clause ("CLI validate and run plus cargo test --workspace
pass at required presets") is still not executed here. The workspace half is
covered by the gate's `cargo test --workspace --all-features` above; per-preset
reproduction is delivered by
[[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] (child of
[[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]), exactly as
TAS-130's preset clause was delegated. No preset coverage is claimed here.
