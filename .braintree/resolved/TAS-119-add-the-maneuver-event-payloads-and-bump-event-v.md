---
context_rev: 1
priority: P1
updated: 2026-09-14T17:03:21Z
summary: Add the maneuver event payloads and bump EVENT_VERSION once.
---

Parent [[TAS-100-version-the-maneuver-event-and-trace-surface]].

# Outcome

The typed event union and canonical serialization carry every increment-2 maneuver
and rule record under one documented version.

# Done when

- Payloads match TAS-083 and name agent, partner when applicable, source and
  target facility or movement, side, reason, state edge, perceived rule, and
  decision reason without copying high-volume trajectory samples.
- EVENT_VERSION is bumped exactly once for the additive union, and manifests,
  JSONL serialization, replay, summaries, and inspectors recognize the same
  version.
- Round-trip tests pass and required Phase 1 goldens change only with the
  versioned rationale.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Extends [[TAS-100-version-the-maneuver-event-and-trace-surface]]; reads the seams
and gates in [[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns
the payloads and the version bump; do not change emission timing.

# Result

Complete. `Event` gains the three payloads the contract fixes — `Maneuver`,
`FacilityTransition`, `OpposingTraversal` — appended to `EventKind` after
`ControlTransition`, so every existing order value is unchanged (Spawned 0 …
ControlTransition 9, Maneuver 10, FacilityTransition 11, OpposingTraversal 12).
No event is emitted yet; [[TAS-120-emit-maneuver-events-edge-triggered-with-stable]]
owns emission timing, and `ClosePass` belongs to a later sibling of the same
version.

`EVENT_VERSION` 2 → 3 (`crates/tangle-sim/src/event.rs`), the union's single
bump; the rationale is recorded on the constant (no existing variant gains,
loses, or reorders a field) and a unit test pins it with the kind order.

Codes and keys:

- `Maneuver.reason: ManeuverReasonCode`, one closed event set that both source
  enums map into (`From<ManeuverReason>`, `From<ManeuverAbortReason>`) plus
  `settled`; every label reuses the source enum's own spelling. Decided by the
  coordinator in this session: `stage::ManeuverReason` alone cannot carry the
  contract's termination codes, and widening it after the bump would be a second
  bump of the same constant.
- `order_key` widens to `(u32, u8, u8, u32, u32, u32, u32, u32)` — agent, kind
  order, key-space tag, then up to five key components in the contract's order
  ending with the edge flag. Slots 0–3 keep their meaning for every existing
  variant; an absent optional component is `0` and a present one its dense index
  plus one. New keys: `Maneuver` (tactic, target facility, from, to, edge),
  `FacilityTransition` (from, to facility), `OpposingTraversal` (facility,
  movement, entering).
- `apps/tangle-cli/src/trace.rs (EventRecord)`: 24 variant-specific optional
  fields appended after `region`; a version-2 line changes only in the header.

Closed-outside-the-named-set paths (closure additions, reported):
`crates/tangle-model/src/source.rs` and `components.rs` gain `label()` on
`TacticKind`, `MovementDirection`, `PermissionEffect`, and `NominalDirection`
(the code spellings the trace and inspector need; same pattern as
`SignalColor::label`, no schema change); `apps/tangle-cli/src/run_metrics.rs
(counted_family)` returns `None` for the new kinds, because the increment-2
families are [[TAS-101-measure-close-passes-with-exact-clearance-evidence]],
[[TAS-122-close-the-close-pass-observation-and-report-clea]], and
[[TAS-124-add-disaggregated-wrong-way-metrics-and-version]]; presenter record
membership is deliberately unchanged, since `is_safety_record` is an allow-list
and [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]] owns the
overlays.

Goldens regenerated with the documented commands (header version only, no new
event line, byte counts unchanged): `tests/golden/walking_guide_v1.trace.*`
(hash `60bd030f…` → `004a3d70…`), the two `four_leg_pedestrian_*` trace goldens,
the three `apps/tangle-cli/tests/golden/*.migrated.trace.sha256` pins, and
`baselines/phase1/baseline.json` (`event_version` and the three preset
`trace_sha256` values).

Gates: `cargo test --workspace` 84 targets, 0 failed; `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, and
`scripts/check-dependency-direction.sh` clean; round-trip evidence is
`trace.rs (each_record_shape_serializes_its_own_fields_in_order)` grown from 10
to 13 variants (construct, serialize, parse, assert each field in order, no
`null`) plus the `event.rs` order-key tests.

Pending advance (coordinator): this node is `# Done when`-complete and
[[TAS-100-version-the-maneuver-event-and-trace-surface]]'s `next` still names it,
so its advance to
[[TAS-120-emit-maneuver-events-edge-triggered-with-stable]] is owed; verify with
`braintree check --allow-pending-advance TAS-100`. `DEF-004-metric-definition-v1`
also cites the old `EVENT_VERSION 2, event.rs:66`, which is now stale — a
coordinator-owned correction.
