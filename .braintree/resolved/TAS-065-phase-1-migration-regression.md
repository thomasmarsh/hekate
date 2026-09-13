---
context_rev: 1
priority: P1
updated: 2026-09-13T15:42:00Z
summary: Prove every Phase 1 acceptance scenario migrates and reproduces its frozen baseline.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

The Increment 0 gate over schema provenance: every Phase 1 acceptance scenario
runs through either its original reader or the explicit migration path and
reproduces the frozen baseline, and any intentional baseline change carries a
minimal regression fixture and a versioned explanation.

# Done when

- Every checked-in Phase 1 acceptance scenario is exercised through its supported reader or the migration path.
- Migrated runs reproduce the frozen Phase 1 baseline hashes except for declared, versioned corrections.
- Any intentional baseline change has a minimal regression fixture and a versioned explanation recorded with it.
- `cargo test` and `baselines/phase1` checks pass.

# Context

Gated on [[TAS-063-v1-to-v2-migration-and-cli]].
Gated on [[TAS-064-migration-provenance]].

# Result

The Increment 0 provenance gate passes: every checked-in version-1 scenario
migrates and reproduces the original reader's canonical trace *body*
byte-for-byte, and the three frozen goldens reproduce through the migration path.

`apps/tangle-cli/tests/migration_regression.rs` (new) drives the gate. For every
`scenarios/**/*.json5` (11 files: walking, 8 benchmarks, 2 experiments) it runs
two traces at seed 7, 1200 Standard steps:

- the **original reader**, `load_scenario` (the version-1 direct compile path the
  `run` command uses); and
- the **migration path**, `parse_scenario_document` → `migrate_v1_to_v2` →
  `validate_v2` (asserted empty) → `CompiledScenario::compile_v2`.

It asserts the event body (every line after the run header) is byte-identical and
that the two headers differ in exactly one field. Four tests:

- `every_checked_in_scenario_is_enumerated` – recursively enumerates
  `scenarios/**/*.json5` and asserts the frozen set is inside it, so a scenario
  added later that does not migrate and reproduce cannot escape the gate.
- `every_scenario_migrates_and_preserves_the_original_trace_body` – the
  differential proof over all 11 scenarios.
- `migrated_frozen_runs_reproduce_their_golden_bodies_and_pinned_hashes` – the
  three frozen goldens.
- `migrated_walking_run_matches_the_frozen_standard_preset_baseline` – the
  walking scenario against `baselines/phase1/baseline.json`.

## Declared, versioned correction

An intentional correction **was** required, but **no frozen baseline hash was
altered**. A migrated run consumes the normalized version-2 document, so
`CompiledScenario::compile_v2` records `schema_version: 2` on the compiled
scenario, and the trace header — the field naming the schema version the scenario
was authored against — records `2` where the version-1 direct reader records `1`.
That one header field is the only difference; the event body is byte-identical
for every scenario. The change is not silent: `ScenarioProvenance` still records
the authored `schema_version` (`1`), the `normalized_sha256` of the version-2
bytes the run consumed, and the `migration_version` (`1`), so it is attributable
to the migration rather than to the kernel.

The versioned explanation is recorded in the suite's module docs and here. The
minimal regression fixture is the three pinned migrated full-trace hashes under
`apps/tangle-cli/tests/golden/<id>.migrated.trace.sha256` (placed beside the suite
rather than under the repository's `tests/golden/` so a migration fixture cannot
be mistaken for a Phase 1 golden):

- `walking_guide_v1` (seed 0, 250 ticks) = `cb06c97a075ec2c75d793760a6ad4d46b683b8a8479e5c8192dbfd7fe61b416c`
- `four_leg_pedestrian_ew_priority_v1` (seed 1, 1200 ticks) = `b8df1255f0a832676f3e91458b7143e3502a02bc6daf05611efec0689f552836`
- `four_leg_pedestrian_ns_priority_v1` (seed 1, 1200 ticks) = `7c5fcdb0f44f9a5eb1e83c07b483cf3ee6661d48ed4be523dc8ddf49f442b889`

The suite reproduces each pinned hash exactly, so a future change to the declared
correction is caught.

No file under `baselines/phase1/**` and no `tests/golden/<id>.trace.*` golden is
changed: the `run` command still uses the version-1 direct reader, so the frozen
Phase 1 evidence is untouched and `checked_in_baseline_matches_a_fresh_capture`
and `standard_preset_hash_matches_the_golden_trace` still pass.

## Differential result

At seed 7, 1200 Standard steps the migrated and original traces have identical
event bodies for all 11 scenarios; their full-trace hashes differ only by the
declared header field. For the three frozen goldens the migrated body equals the
golden body byte-for-byte, the migrated header equals the golden header except
`schema_version`, and the original reader still reproduces the frozen Standard
preset baseline hash (`60bd030f7b830d63292f8198dc57b2db2a9bdcab8b315e82251c12ff53195cba`).

Evidence: `cargo test` (workspace) passes; `cargo test -p tangle-cli` passes,
including the two `baselines/phase1` checks and the new four-test suite;
`cargo clippy -p tangle-cli --all-targets` is clean; `cargo fmt --all -- --check`
is clean; `scripts/check-dependency-direction.sh` reports `dependency direction OK`.

Handoff (outside this leaf's write set): once this node is `resolved/`, the parent
[[TAS-057-schema-v2-migration-and-provenance]] still names it in `next`, which
`braintree check` reports as one expected `next-resolved-node` diagnostic. The
coordinator owns advancing that parent's `next`.
