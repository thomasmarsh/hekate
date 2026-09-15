---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T04:19:02Z
summary: Prove deterministic simultaneous claims and every unsafe committed-maneuver response.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Adversarial tests prove that simultaneous claims resolve by the documented
stable key and that a committed maneuver responds to each newly unsafe corridor
with the documented bounded brake, abort, hold, or return action.

# Done when

- Two and three-agent claims on the same corridor select the same winner after
  reversing source declaration, candidate discovery, and insertion order.
- Fixed-seed repetition is identical and the result does not depend on an
  unordered collection or incidental random draw.
- Front intrusion, rear intrusion, disappearing connector, narrowing corridor,
  and blocked return each reach the required state and bounded motion response.
- A falsification probe changes the tie-break key or removes one hazard response
  and demonstrates that the suite fails for the intended reason.
- No case teleports, overlaps silently, exceeds a motion limit, or crosses a
  forbidden boundary without the TAS-100 event fact.

# Result

Both slices resolved and integrated, so this parent's Done-when is met.

- [[TAS-127-prove-deterministic-simultaneous-claim-resolutio]] (resolved,
  `a151b4a`) proves two- and three-agent claim determinism under reversed
  declaration, candidate discovery, and insertion order, and fixed-seed
  repetition identity: the parent's first two clauses.
- [[TAS-128-prove-each-unsafe-commit-hazard-response-with-a]] (resolved,
  `78c0d4b`) proves the front, rear, side-narrowing, lost-connector, and
  blocked-return hazard responses and the response-removal falsification probe:
  the parent's last three clauses (the probe clause is disjunctive, satisfied by
  the removed hazard response).

Verified by the TAS-128 resolution gate on `78c0d4b` from the repo root:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, `cargo test --workspace --all-features` (993 passed, 0 failed,
1 ignored across 81 test binaries and six doctest blocks),
`./scripts/check-dependency-direction.sh`, and `tangle check`
(`graph check: passed (194 nodes)`) all green. No slice added a broad scenario
fixture, metric, or presentation, and no seam defect was exposed, so no
production line changed.

# Context

Gated on [[TAS-091-resolve-gap-claims-and-maneuver-transitions]],
[[TAS-095-complete-lane-transitions-and-safe-aborts]], and
[[TAS-100-version-the-maneuver-event-and-trace-surface]]. Owns one focused
adversarial integration suite and minimal defects it exposes. Do not add broad
scenario fixtures, metrics, or presentation.

Gates my test artifact enters: Cargo's hekate-sim integration-test discovery and
the workspace warnings-as-errors test gate.

# Slices

- [[TAS-127-prove-deterministic-simultaneous-claim-resolutio]] Deterministic simultaneous-claim proof.
- [[TAS-128-prove-each-unsafe-commit-hazard-response-with-a]] Unsafe-commit hazard responses and probe.
