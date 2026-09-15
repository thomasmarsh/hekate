---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T03:44:29Z
summary: Prove deterministic simultaneous-claim resolution.
next: Run the five-gate validation and resolve.
---

Parent [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]].

# Outcome

Adversarial tests prove simultaneous corridor claims resolve by the documented
stable key regardless of declaration, discovery, or insertion order.

# Done when

- Two and three-agent claims on the same corridor select the same winner after
  reversing source declaration, candidate discovery, and insertion order.
- Fixed-seed repetition is identical and the result does not depend on an
  unordered collection or incidental random draw.

# Result

Test-only slice; no production logic in `crates/hekate-sim/src/` changed.

In-crate unit tests extend `stage::tests`; a new integration file
`crates/hekate-sim/tests/claims.rs` covers the public seam.

- `three_conflicting_claims_resolve_identically_in_every_insertion_order`: three
  claims on one corridor in all six insertion orders produce byte-identical
  `arbitrate_claims` output; the nearest claimant wins.
- `a_three_way_exact_tie_is_won_by_the_lowest_agent_id_in_every_permutation`: an
  exact tie on the committed clause and the entry distance is decided by the
  lower `AgentId` in every permutation, asserted as the stable-key property.
- `a_committed_claim_keeps_its_corridor_against_two_preparing_claims`: the
  committed claimant outranks two nearer preparing claims in every order.
- `a_three_agent_claim_resolves_to_the_same_winner_under_reversed_request_order`:
  three riders contest one corridor through `request_lateral_maneuver`; the
  nearest commits, both others abort with `ClaimRejected`, and the reversed
  request order yields identical transitions and states.
- `a_claim_run_is_identical_across_fixed_seed_repetitions`: two runs at
  `RunConfig::new(0)` produce identical transition and `Event::Maneuver` streams
  over 40 steps.

Verified: `cargo test -p hekate-sim stage::tests` (13 passed),
`cargo test -p hekate-sim --test claims` (2 passed),
`cargo test -p hekate-sim --test maneuver_lifecycle` (6 passed),
`cargo clippy -p hekate-sim --all-targets` and `cargo fmt --all` clean. No
permutation changed a winner and no fixed-seed repetition differed, so no seam
defect was found.

# Context

Approved reading of "same winner": for asymmetric claimants it is the same
physical winner under a reversed request/insertion order; for an exact tie it is
the lowest `AgentId`, the documented stable key, asserted as a key property
rather than a fixed identity. A declaration reversal permutes ids and moves each
source's keyed random stream, so an assertion that the same `AgentId` wins
across a declaration reversal is out of contract and is not written.
Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Extends [[TAS-105-prove-deterministic-claims-and-unsafe-commit-policy]]; reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the claim
determinism proof; the hazard responses are the sibling slice.
