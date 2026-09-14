---
context_rev: 1
updated: 2026-09-14T11:34:53Z
summary: Compile the adjacency shared-boundary lateral coordinate.
---

Parent [[TAS-092-passing-and-lane-transition-behavior]].

# Outcome

`CompiledFacilityAdjacency` exposes the compiled cross-band offset (the shared
boundary's lateral coordinate between its two bands), so a lateral handoff, its
predicted corridor, and its committed hazard response read one compiled
geometric datum instead of the source band's half-width runtime approximation.

# Done when

- The compiled adjacency carries the shared boundary between its two facility
  regions, derived at compile time from the regions' longest collinear shared
  segment, and exposes it (for each band and travel frame) so a consumer can
  express the destination band's usable interval in the source band's frame.
- The derivation reuses one implementation in `crates/tangle-model` rather than
  duplicating the validation helper, and an adjacency whose bands do not touch
  along a shared boundary stays dropped/uncompiled exactly as validation already
  rejects it.
- `docs/schema-v2-contract.md`'s "Transition targets and the geometric handoff"
  names the compiled datum that realizes the lateral handoff point; no authored
  source field is added, so `schemas/scenario-source.schema.json` and the source
  shapes are unchanged.
- Focused model-crate tests cover a straight parallel pair, a curved pair, and a
  corner-only touch that yields no coordinate, and the whole workspace builds.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features
  -- -D warnings`, and `scripts/check-dependency-direction.sh` pass.

# Context

Gated on [[TAS-085-compile-increment-2-policy-and-traversal-semantics]] and
[[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]]. Owns the compiled
adjacency datum, its accessor, the shared boundary-derivation helper, the
contract clause naming it, and focused model tests. Do not change the authored
schema, emit events, or implement the sim-side corridor/abort behavior
([[TAS-095-complete-lane-transitions-and-safe-aborts]] consumes this).

# Result

The compiled adjacency now carries the shared boundary between its two facility
regions:

- `CompiledFacilityAdjacency::shared_boundary_midpoint()` returns the world
  midpoint of the longest collinear segment the two regions share, derived in
  `compile_facility_adjacencies` from the compiled region polygons that
  `compile_regions` builds before the adjacencies read them.
- `CompiledFacilityAdjacency::shared_boundary_offset(facility, direction)`
  returns that boundary's signed lateral offset from `facility`'s reference path
  in its own travel frame (positive to the left of travel), or `None` when
  `facility` is not one of the two bands. Its sign agrees with the compiled
  `LateralTransition::side` in the same frame, so a consumer expresses the
  destination band's usable interval in the source band's frame from one
  compiled geometric datum.
- `crates/tangle-model/src/validate.rs`'s `shared_boundary_midpoint` is the one
  implementation; it is now `pub(crate)` over `DVec2` rings and both validation
  and compilation call it. Validation diagnostics and behaviour are unchanged.
  An adjacency whose regions touch at a corner alone shares no segment, so
  `compile_facility_adjacencies` drops it exactly as validation rejects it with
  `E_FACILITY_ADJACENCY_DISJOINT`.
- `docs/schema-v2-contract.md`'s *Transition targets and the geometric handoff*
  names both accessors; no authored field or schema JSON changed.

Focused tests: `compiles_the_shared_boundary_of_a_straight_parallel_pair`,
`compiles_the_shared_boundary_of_a_curved_pair`, and
`a_corner_only_touch_yields_no_boundary_and_is_rejected` in
`crates/tangle-model/tests/increment2_compiled.rs`, plus a
`shared_boundary_midpoint` unit test in `validate.rs`.

Evidence: `cargo test -p tangle-model` (all green), `cargo test --workspace`
(exit 0), `cargo clippy --workspace --all-targets --all-features -- -D warnings`
(exit 0), and `scripts/check-dependency-direction.sh` (OK). The pending
advance of the parent `TAS-092`'s `next` off this resolved node is a coordinator
action: this node's write set excludes the parent.
