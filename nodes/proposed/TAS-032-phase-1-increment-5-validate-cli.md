---
context_rev: 1
priority: P1
updated: 2026-09-12T23:56:54Z
summary: Slice A1 of Phase 1 Increment 5 adds a first-class `validate` CLI command that checks a scenario source (and compiled scenario) and reports actionable diagnostics with a documented contract and non-zero exit on failure.
next: Discover the existing `apps/tangle-cli` command surface and scenario-loading path, then implement `validate` with a documented contract and tests.
---

# Outcome

Add a `validate` command to the Tangle CLI alongside the existing `run`
command. `validate` loads a scenario source, checks it against the schema and
the semantic invariants the loader enforces, and reports diagnostics to the
user without running the simulation. It exits non-zero on any invalid input and
zero on a clean source, so it can be used in CI and in pre-batch checks. It does
not write any run artifact.

The command's contract is documented where the user finds the other command
help, and it has tests for: a valid source, a syntactically malformed source, a
schema-invalid source, and a semantically invalid source the loader rejects.
Reuse the existing scenario-source parsing/validation path rather than adding a
second parser; if a reusable validation entry point does not yet exist, add the
minimal seam inside the CLI layer or the existing parse path in the same write
set.

Constraint: no new dependency in `tangle-model` or `tangle-sim`; validate uses
the existing loader. Any schema change stays additive to schema version 1.

# Done when

- `validate` is reachable from the CLI with a documented contract (usage,
  accepted inputs, exit codes).
- Valid, malformed, schema-invalid, and loader-invalid sources are covered by
  tests, and each invalid case exits non-zero with an actionable message.
- No run artifact is written and no simulation tick executes.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice A of Increment 5 asks for `validate`, `run`, `batch`, and `replay`.
`validate` is split out as the first child because it is a bounded,
independently resumable command that does not depend on the run-directory
format the later `batch`/`replay` children consume.

# Result

Pending.
