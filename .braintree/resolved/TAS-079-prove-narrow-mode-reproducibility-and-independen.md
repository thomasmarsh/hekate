---
context_rev: 1
updated: 2026-09-13T21:05:00Z
summary: Prove narrow-mode reproducibility and independence from passenger-car dimensions and defaults.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

The Increment 1 verification gates hold as checked evidence: repeated seeded runs
of the narrow fixtures reproduce demand, profiles, decisions, events, and trace
hashes, and each narrow mode passes its independent fixtures without relying on
car-specific dimensions or controller defaults. This leaf adds the reproducible
batch/replay check and the negative guard that fails if shared code substitutes a
car dimension or a car controller default for a sampled narrow value.

# Done when

- A determinism test runs each `narrow_isolated_*` fixture twice at a fixed seed
  and asserts an identical trace hash, and a batch over a declared seed bank
  reproduces the per-seed hashes and event streams.
- A stream-isolation / no-defaults check fails when a narrow agent's body or
  limit comes from a Phase 1 passenger-car constant rather than its compiled
  template, proven with a falsification probe as in
  [[TAS-070-synthetic-template-gate]].
- `cargo test --workspace` passes and every narrow fixture's recorded trace hash
  is stable across the checked-in presets tested.
- The increment's four gate criteria are each traced to a passing test in the
  node result.

# Context

Gated on [[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]],
[[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]], and
[[TAS-078-check-in-narrow-mode-isolated-fixtures-and-tests]]. Per
`PHASE_2_PLAN.md` *Increment 1* gate ("repeated seeded runs reproduce...; each mode
passes independent fixtures without relying on car-specific dimensions or
controller defaults") and *Validation strategy* (deterministic tests). Read
[[TAS-070-synthetic-template-gate]] and [[TAS-059-benchmark-matrix-and-tolerances]].
Owns the determinism and no-car-defaults tests in `crates/tangle-sim/tests/` and
any replay/batch wiring in `apps/tangle-cli` they require.

# Result

All four `Done when` bullets hold. Three new files: the CLI determinism +
seed-bank-batch suite, the kernel no-car-defaults suite with its falsification
probe, and the declared seed bank. No production code, schema, baseline, golden
trace, or other node changed.

## Determinism and the seed-bank batch (`apps/tangle-cli/tests/narrow_determinism.rs`)

- `each_narrow_fixture_reproduces_its_trace_hash_at_every_preset` runs each
  checked-in `narrow_isolated_*` fixture twice at the fixed seed `20_260_913` at
  each matrix preset `F`/`S`/`f` (0.1/0.05/0.02 s) and asserts an identical
  canonical trace hash, asserts each run spawns at least one agent, and asserts a
  second seed diverges, so the stability is not a constant. Observed `S`-preset
  hashes (seed `20260913`, 600 ticks):
  `straight 3ac3feeb…7742b`, `curve 5c4ebbbb…ab5f`,
  `braking 46ab374c…d67e`, `following b1b11900…ce5b`,
  `signal 9cc53c2b…7611`, `crossing fc22c56a…2e7c`. The `F`/`f` presets record
  stable-but-distinct hashes (for example straight
  `F 38b798ab…6d52d`, `f fe66b2d0…64e76`).
- `the_narrow_seed_bank_batch_reproduces_every_per_seed_hash_and_event_stream`
  reads the declared bank `scenarios/phase2/inc1/narrow_isolated_seed_bank.json`
  through `read_seed_bank`, runs `run_batch` twice per fixture over the bank's
  ordered seeds, and asserts per seed that the two batches' `trace_sha256` and
  decompressed `events.jsonl.gz` are equal and that each run manifest's
  `stream.uncompressed_sha256` is that hash. Per-seed hashes recorded for every
  fixture (seed 20260913 equals the `S`-preset hash above):
  - straight `…914 750f0ea6…c2c1`, `…915 8d0c3ab8…1565`
  - curve `…914 91277fb7…5ab02`, `…915 11eb52ff…3f702`
  - braking `…914 c82593ef…62b8d`, `…915 afcfe154…482e`
  - following `…914 b2574300…9a96`, `…915 aaecc70d…78c`
  - signal `…914 9731e39e…4882`, `…915 36aa1f03…d37`
  - crossing `…914 8cc2a7a5…19d6`, `…915 c3a9890c…5ba7`
- `the_declared_seed_bank_runs_the_same_traces_as_an_explicit_seed_list` proves
  the bank adds pairing without changing the runs.
- The bank's recorded content hash is `3c700e76…73c9`; a manual CLI batch
  reproduces it and the same per-seed hashes (command under Acceptance).

## No-car-defaults check and falsification probe (`crates/tangle-sim/tests/narrow_no_car_defaults.rs`)

`car_default_violations` is a behavioral check, not a source grep: for every live
agent on a narrow fixture's facility it compares the values the kernel realized —
capsule body length and width, desired speed, max acceleration, comfortable
braking, steering rate, and lateral clearance — to the facility's compiled mode
template, and reports a violation when the realized value is not the template's
(a narrow profile is absent, or the facility's mode is not the
`WheeledCapsule` family the narrow path dispatches on).

- `narrow_fixtures_realize_their_compiled_template_with_no_passenger_car_defaults`
  holds all six fixtures to an empty violation list while observing live agents.
- `the_no_car_defaults_check_flags_a_narrow_facility_whose_agent_uses_the_car_profile`
  is the falsification probe. It constructs the failure through the kernel: a
  facility whose mode template compiles to a box (the `WheeledBox` family, like
  the Increment 0 passenger car) cannot take the narrow path, so the kernel
  samples the Phase-1 passenger-car profile (`scenario.profiles()`, default body
  4.0–5.2 m, speed 9.0–15.0 m/s) for its agent — the agent's body and limits come
  from the Phase-1 passenger-car constants rather than the authored 1.8 m
  template. The probe asserts the realized body really is the car band and not
  the authored template, and that `car_default_violations` reports it (naming the
  body substitution and the missing narrow profile). Detection is therefore
  proven by constructing the failure, as [[TAS-070-synthetic-template-gate]] does
  for its no-branch guard, rather than by a source-text assertion.

## The increment's four gate criteria, each traced to a passing test

1. **Path/world round trips stay within tolerance on straight and curved
   fixtures.** `narrow_isolated::the_narrow_isolated_straight_fixture_holds_trt_tdim_tenv_and_tspd_at_every_preset`
   and `…curve_fixture_holds_trt_tdim_tenv_and_tspd_at_every_preset` (`T-RT`,
   `<= 1e-9 m`; the curve test measures the declared analytic
   `CompiledReferencePath::arc`). Compiled facility/reference geometry is
   [[TAS-074-compile-version-2-facility-and-connector-shapes]], validated by
   [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]].
2. **Bicycle and scooter motion respects dimensions, speed, acceleration,
   braking, steering, and facility boundaries.**
   `narrow_spawn::narrow_modes_spawn_from_their_compiled_template_with_their_body_and_profile`
   and `narrow_longitudinal::a_narrow_agent_accelerates_follows_holds_yields_and_completes_a_compiled_facility_route`
   ([[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]]), plus the
   `T-DIM`/`T-ENV`/`T-SPD` bounds every `narrow_isolated` test holds each sample
   to ([[TAS-078-check-in-narrow-mode-isolated-fixtures-and-tests]]).
3. **Repeated seeded runs reproduce demand, profiles, decisions, events, and
   trace hashes.** This leaf:
   `narrow_determinism::each_narrow_fixture_reproduces_its_trace_hash_at_every_preset`
   (twice per fixture at each preset, identical hash; a second seed diverges) and
   `narrow_determinism::the_narrow_seed_bank_batch_reproduces_every_per_seed_hash_and_event_stream`
   (two batches over the declared seed bank agree on every per-seed hash and
   event stream).
4. **Each mode passes independent fixtures without relying on car-specific
   dimensions or controller defaults.** [[TAS-078-check-in-narrow-mode-isolated-fixtures-and-tests]]
   runs every `narrow_isolated_*` fixture for both modes with no car-specific
   branch (`narrow_mode_no_branch`), and this leaf's
   `narrow_fixtures_realize_their_compiled_template_with_no_passenger_car_defaults`
   plus its falsification probe
   `the_no_car_defaults_check_flags_a_narrow_facility_whose_agent_uses_the_car_profile`
   prove no realized narrow dimension or limit comes from the Phase-1
   passenger-car profile.

## Files changed

New: `apps/tangle-cli/tests/narrow_determinism.rs`,
`crates/tangle-sim/tests/narrow_no_car_defaults.rs`,
`scenarios/phase2/inc1/narrow_isolated_seed_bank.json`, and this node's move to
`resolved/`. No production module in `tangle-sim`, `tangle-model`,
`tangle-present`, or `apps/tangle-cli/src` changed; no `baselines/**`,
`schemas/**`, `docs/**`, golden trace, Phase 1 fixture, Phase 1 baseline, or
other node file changed. No additive `tangle-model` accessor was needed.

## Acceptance

- `cargo test -p tangle-sim` and `cargo test -p tangle-cli` — pass, including the
  new `narrow_no_car_defaults` (2 tests) and `narrow_determinism` (4 tests)
  suites.
- `cargo test --workspace` — passes; Phase 1 golden traces and baselines
  unchanged (no file under `baselines/**`, `tests/golden/**`, or
  `apps/tangle-cli/tests/golden/**` was touched).
- Batch over the declared seed bank, twice, reproduces per-seed hashes and event
  streams:
  `cargo run -q -p tangle-cli -- batch scenarios/phase2/inc1/narrow_isolated_straight_v2.json5 --seed-bank scenarios/phase2/inc1/narrow_isolated_seed_bank.json --out-root /tmp/narrow-bank-a --ticks 600`
  (and `…b` for the second root) records seed `20260913 3ac3feeb…7742b`,
  `20260914 750f0ea6…c2c1`, `20260915 8d0c3ab8…1565` in both, with identical
  `events.jsonl.gz` bytes per seed.
- `cargo fmt --all -- --check` clean;
  `RUSTFLAGS="-D warnings" cargo clippy -p tangle-sim -p tangle-cli --all-targets --all-features`
  clean.
- `scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `braintree check` — `graph check: passed (121 nodes)` while this node was
  still `proposed`.

## Note for the coordinator

`PHASE_2_PLAN.md` *Increment 1* names four gate criteria; they are traced to
tests above. The declared seed bank lives under `scenarios/phase2/inc1/` as plain
JSON (not `.json5`) so the version-1/version-2 scenario gate that enumerates
`*.json5` does not treat it as a scenario. Post-move, `braintree check`
transiently reports `next-resolved-node` because TAS-019's `next` still names
this node; the coordinator advances it. No parent or sibling node was edited and
no child node was created.
