---
context_rev: 1
priority: P1
updated: 2026-09-13T00:02:34Z
summary: Slice A1 of Phase 1 Increment 5 adds a first-class `validate` CLI command that checks a scenario source (and compiled scenario) and reports actionable diagnostics with a documented contract and non-zero exit on failure.
next: Hand this slice to the TAS-031 coordinator for integration, slice F verification, and closeout.
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

Slice A1 landed the `validate` command on the existing loader. No schema
change, no new dependency, no golden or baseline change, and no simulation tick:
`validate` stops after the loader's compilation step.

## What landed

- `apps/tangle-cli/src/validate.rs` (new) is the CLI-layer seam.
  `validate_scenario(&Path) -> Result<ValidationSummary, LoadError>` reads,
  parses, semantically validates, and compiles a source through the same
  `load_scenario` entry point `run` and `baseline` use — one parser and one
  validation path — and reports `source_path`, `scenario_id`, and
  `schema_version`. `render_validation_failure` turns a `LoadError` into a
  report with one indented diagnostic per line, so every finding is visible
  instead of folded into `Display`'s `; `-joined single line.
  `apps/tangle-cli/src/lib.rs` re-exports both.
- `apps/tangle-cli/src/main.rs` adds `validate <SCENARIO>` next to `run` and
  `baseline`. Its contract (usage, inputs, output, exit codes 0/1/2) is the
  subcommand's `long_about`, shown by `validate --help`, and the module docs
  carry the same contract. A clean source prints one report line to stdout and
  exits 0; an unreadable, malformed, schema-invalid, or semantically invalid
  source exits 1 through the existing `fail` path with the diagnostics on
  stderr.
- `apps/tangle-cli/tests/validate.rs` (new) is the contract suite.

## Evidence

`cargo test -p tangle-cli --test validate` (binary `target/debug/deps/validate-*`),
7 passed:

- `valid_source_exits_zero_and_reports_the_scenario` — exit 0, the report line
  `valid.json5: valid scenario 'validate_valid_v1' (schema version 1)`, and an
  empty stderr.
- `checked_in_walking_scenario_validates_by_absolute_path` — the checked-in
  walking scenario validates by absolute path.
- `malformed_source_exits_non_zero_with_a_positioned_message` — truncated JSON5
  exits 1 with the position (`4:1`).
- `schema_invalid_source_exits_non_zero_naming_the_offending_field` — a
  well-formed document carrying an undeclared `speed_limit_mps` field exits 1
  and the message names that field.
- `loader_invalid_source_exits_non_zero_with_every_diagnostic` — a structurally
  valid source with a duplicate portal identifier and a portal on an undeclared
  path exits 1 with both diagnostics, one per line (`E_ID_DUPLICATE`,
  `E_PORTAL_UNKNOWN_PATH`).
- `validate_writes_no_run_artifact` — after validating a valid and an invalid
  source in a scratch working directory, the directory holds exactly the two
  source files and neither output reports a `trace hash`.
- `help_documents_usage_inputs_and_exit_codes` — `validate --help` carries the
  usage line, the input, and the exit-code contract.

Gates on this tree: `cargo test --workspace --all-features` passed,
`cargo clippy --workspace --all-targets --all-features -- -D warnings` passed
clean, `cargo fmt --all --check` passed after `cargo fmt --all`,
`./scripts/check-dependency-direction.sh` printed `dependency direction OK`, and
`braintree check nodes` passed.

## Claim and write set

Base hash `945fe712c0ddf659c4fb3e28b382d2b98aefe03fd783389f0e19f8c46121f442`
from `braintree hash TAS-032`, claimed by `worker` for 14400 s. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`, and this node. No path
outside the set was touched and `apps/tangle-cli/Cargo.toml` is unchanged, so no
new dependency was added.

## Deferrals

- `validate` reports loader acceptance, not a run: it constructs no
  `Simulation` and takes no seed or tick count, which stay `run`'s arguments.
- `run`, `batch`, `replay`, and the run-directory format stay with the later
  slices of TAS-031; this node introduces no seam for them.
