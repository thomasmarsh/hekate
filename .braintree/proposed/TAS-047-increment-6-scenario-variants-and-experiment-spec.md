---
context_rev: 1
priority: P1
updated: 2026-09-13T11:48:58Z
summary: Slice B of Phase 1 Increment 6 authors the two freely described variants of one small intersection that differ only through scenario data, the checked-in experiment spec and seed bank, and the evidence that a geometrically different benchmark scenario can be added without changing simulator logic.
next: Author the two scenario-only variants of the same small intersection plus the checked-in experiment spec and seed bank, and prove a geometrically different benchmark scenario adds without simulator-logic changes.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

The variants must differ through scenario data, not code. `PHASE_1_PLAN.md`'s
kickoff recommendation names two fixed-time signal/control variants of one
four-leg car/pedestrian intersection as the Phase 1 comparison case; a roundabout
would force yielding and lateral-path complexity too early. Reuse the existing
version-1 scenario source, schema, and compiled scenario; any schema change must
be additive to schema version 1, regenerate
`schemas/scenario-source.schema.json`, and keep the drift test. No schema
version 2.

# Outcome

- Two scenario-source variants of the same small intersection, differing only in
  scenario data (for example signal timing, control policy data, or demand; not
  geometry or code).
- A checked-in experiment spec and a common-random-number seed bank that name
  the variants, fidelity, seeds, and sampling policy.
- A demonstrated geometrically different benchmark scenario that is added
  through scenario data alone, with no simulator-logic change.

# Done when

- Both variants validate and run through the existing CLI without source
  changes.
- The variants are equivalent except for the scenario-data differences that are
  the experiment's independent variable, and that equivalence is stated.
- The experiment spec and seed bank are checked in to the repository.
- A geometrically different benchmark scenario exists, is validated, and is
  shown to need no change to `tangle-model` or `tangle-sim`.
- Schema validation, the regenerated JSON Schema, and the drift test are green;
  no schema version changes.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.
