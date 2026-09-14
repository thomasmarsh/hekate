---
context_rev: 1
updated: 2026-09-14T11:18:59Z
summary: Compile the adjacency shared-boundary lateral coordinate.
next: Extend CompiledFacilityAdjacency with a compile-time shared-boundary lateral coordinate, expose it, state it in docs/schema-v2-contract.md, and test it.
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
