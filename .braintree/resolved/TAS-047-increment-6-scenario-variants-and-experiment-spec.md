---
context_rev: 1
priority: P1
updated: 2026-09-13T12:35:14Z
summary: Slice B of Phase 1 Increment 6 authors the two freely described variants of one small intersection that differ only through scenario data, the checked-in experiment spec and seed bank, and the evidence that a geometrically different benchmark scenario can be added without changing simulator logic.
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

# Result

Claimed by `worker` at base hash
`c2fa601191d1ff935fc2fe1179ecc625b728efc69d5f4b65f90d7706b0a857f3`
(`braintree hash TAS-047`, bare-ID form, FBK-002) with `--lease-seconds 14400`,
and moved to `.braintree/active/` with `git mv` plus `git add` of the
destination (FBK-013). Write set held: `scenarios/**`,
`experiments/increment6_signal_timing_v1/**`, `apps/tangle-cli/tests/**`, and
this node. No file under `crates/`, `schemas/`, `baselines/`, or `tests/golden/`
changed.

## What landed

- `scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5` (variant A)
  and `scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5`
  (variant B): one four-leg car/pedestrian intersection each, authored from the
  version 1 primitives. An east-west and a north-south through movement crossing
  in one conflict region, one fixed-time two-phase signal with a head per
  movement, two crosswalks on the approach legs (west leg over the east-west
  road, south leg over the north-south road) each with its own fixed-time
  pedestrian signal, three kerb waiting areas, four pedestrian routes, demand
  for both vehicle movements, and demand and profiles for both modes. Each
  movement's stop line sits upstream of the crosswalk its movement crosses
  (38 m from its portal), so a queue waits clear of the crosswalk; both
  movements also carry the `yield` rule from `mixed_interaction_v1`, so a
  vehicle does not enter a crosswalk a pedestrian occupies while the signal
  controls who may proceed.
- `scenarios/benchmarks/offset_junction_v1.json5`: the geometrically different
  benchmark (below).
- `experiments/increment6_signal_timing_v1/experiment.json`: the checked-in
  experiment spec.
- `experiments/increment6_signal_timing_v1/seed_bank.json`: the checked-in
  common-random-number seed bank, written by `tangle-cli seed-bank --start 1
  --count 10`, so it is the byte-reproducible Increment 5 artifact
  (`seed_bank_version` 1, seeds 1..10 ascending).
- `apps/tangle-cli/tests/experiment_spec.rs`: the contract suite (5 tests).
- `apps/tangle-cli/tests/scenarios.rs`: `offset_junction_v1` added to both
  structural benchmark lists, so the existing layout and demand contracts cover
  it; `every_checked_in_scenario_loads` already walks `scenarios/` recursively
  and therefore covers all three new scenario sources.

## The independent variable

The variants are byte-identical except for the scenario `id` line and six
`duration_s` values, and the suite proves it twice (line-for-line over the raw
bytes, then over the parsed `ScenarioSource` with the `id` and every phase
duration normalized). The differing scenario data is exactly the fixed-time
signal plan:

| Field | Variant A | Variant B |
| --- | --- | --- |
| `signals[0].phases[0].duration_s` (east-west green) | 30.0 | 20.0 |
| `signals[0].phases[2].duration_s` (north-south green) | 20.0 | 30.0 |
| `crossings[west_crossing].pedestrian_signal.phases` durations | 34.0, 24.0 | 24.0, 34.0 |
| `crossings[south_crossing].pedestrian_signal.phases` durations | 34.0, 24.0 | 24.0, 34.0 |

Both variants keep the same heads, the same four phases (green 30/20, yellow
4, green 20/30, yellow 4) and therefore the same 58 s cycle, and each crosswalk
walks for a period that sums with its don't-walk period to that same cycle; only
the green split and the walk intervals coordinated to it move. Geometry, paths,
portals, regions, conflict region, rules, demand, pedestrian routes and demand,
waiting areas, and both profile sets are controlled data and identical, which
is the stated equivalence: **the variants are one authored intersection whose
only differing scenario data is the fixed-time signal timing plan.**

## Experiment spec and seed bank

`experiments/increment6_signal_timing_v1/experiment.json` names both variants by
repository-relative path, the fidelity and step (`standard`, 0.05 s, from
`PRESETS`), the horizon (`ticks` 6000 = `duration_s` 300.0, so other fidelities
derive their ticks from it), the seed bank path, and the sampling policy. The
sampling block is the Increment 5 `SamplingPolicy` shape verbatim
(`policy_version` 1, `events: all`, `trajectories: sampled` with `stride_ticks`
10 and `max_samples` 100000), and the test asserts it equals
`SamplingPolicy::default()`, so output stays bounded and full trajectories remain
an opt-in no experiment here takes. `seed_bank.json` is the Increment 5 artifact
itself: both sides run its ten ordered seeds, so the per-seed runs are paired and
the paired comparison cancels the between-seed variance both sides carry.

There was no checked-in Increment 5 *experiment spec* artifact to copy: the only
Increment 5 specification surfaces are the fields `batch.json` records in
`BatchSpec` (`scenario`, `ticks`, `fidelity`, `step_s`, `event_version`,
`model_version`, `build_revision`, `sampling`) and the per-run `SamplingPolicy`.
The spec therefore reuses that vocabulary rather than inventing a new one: it
carries the subset a runner supplies (fidelity, step, ticks, sampling), names the
scenarios and the bank the batches are launched from, and leaves the derived and
recorded fields to `batch.json`. The format choice is recorded as friction.

## Geometric-difference proof

`scenarios/benchmarks/offset_junction_v1.json5` is a six-arm staggered junction:
a wider main road (9 m portals against the variants' 7 m) crossed at `x = -18`
and `x = +18` by two separate north-south arms, each crossing carrying its own
authored conflict region, with the main road `free` and both arms `yield` at
priority 1, and no signal controller, crossing, or pedestrian infrastructure at
all. It validates and runs through the unmodified CLI, and it is structurally
different from the variants (3 movements against 2, 2 conflict regions against
1, 0 signals against 1).

The commit that adds it (`7396058`) touches one file under `scenarios/`:

```
git diff --stat 7396058^ 7396058 -- crates/tangle-model crates/tangle-sim
(empty)
```

No line of `crates/tangle-model` or `crates/tangle-sim` changed for the whole
slice, which is what makes the geometry data-only.

## Validation

`apps/tangle-cli/tests/experiment_spec.rs` (5 tests, 1.95 s):

- `the_variants_differ_only_in_the_signal_timing_plan` — the byte-level line
diff admits only the `id` and `duration_s` lines, the parsed documents agree on
heads, rules, and cycle length while the first green differs, and the two are
equal once the id and every phase duration are normalized.
- `both_variants_validate_and_run_through_the_cli` — `tangle-cli validate`
  reports each variant valid and `tangle-cli run --seed 1 --ticks 600` produces a
  non-empty canonical trace for each, and the two traces differ at that seed, so
  the independent variable is observable.
- `the_experiment_spec_and_bank_name_the_checked_in_inputs` — the spec parses,
  its fidelity and step are a Phase 1 preset, its ticks and duration agree, its
  sampling equals the bounded default, its two variant paths are the checked-in
  files and load, and the bank it names parses as a `seed_bank_version` 1 bank of
  seeds 1..10 with the content hash a comparison proves both sides share.
- `the_checked_in_experiment_runs_paired_from_the_seed_bank` — `batch
  --seed-bank` over the checked-in bank runs all ten seeds for both variants
  (recording the bank path and hash) and `compare` pairs them, reporting 48
  metrics over 10 paired seeds with `metric_definition_version` 2.
- `a_geometrically_different_benchmark_needs_no_simulator_change` — the offset
  junction validates and runs through the CLI, is structurally different, and
  admits routed demand on the evaluated kernel.

Ad-hoc observability check (not a committed artifact, and TAS-048 owns the
report): running the checked-in experiment as the spec declares it — both
variants, the checked-in bank, Standard at 6000 ticks, `--jobs 8` — and pairing
with `compare` shows the plan does move the outcome: mean control delay per
movement flips with the split (variant A: east-west 14.18 s against north-south
26.53 s; variant B: east-west 24.44 s against north-south 13.49 s), while the
aggregate stays within about a second per vehicle. No direction is claimed here;
the paired comparison report and its convergence evidence are
[[TAS-048-increment-6-run-and-comparison-report]]'s outcome.

No schema change was required: every new scenario field already exists in
schema version 1, so `schemas/scenario-source.schema.json` is unchanged,
`SUPPORTED_SCHEMA_VERSION` stays 1, and the drift test
`checked_in_schema_matches_the_generated_schema` passes untouched. No schema
version 2 was introduced.

## Gates

All five green on the committed tree (`a9ee7ef`, `7396058`, `b85cf20`):
`cargo test --workspace --all-features` (564 passed, 0 failed, 1 ignored);
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
`cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh`
printed `dependency direction OK`; `braintree check` printed
`graph check: passed (87 nodes)`.

## Closeout

Every `# Done when` criterion is met, so the node resolves. The coordinator
advanced TAS-045's `next` from this node to
[[TAS-048-increment-6-run-and-comparison-report]] (commit `7632768`,
`braintree check` green), which is what unblocked the move; the `Closes TAS-047`
commit moves the node to `.braintree/resolved/` with its `next` removed. The
`next` this node carried until then spelled no wikilink, because a checker reads
a wikilink in a `next` as the route form and this node has no direct child
(FBK-019). No pinned consumer exists:
`rg -n -F 'Depends on [[TAS-047' .braintree` returns nothing, and no other node
changed, so `context_rev` stays 1. The experiment-spec format gap is recorded as
`FBK-020`.

# Resolution

Every `# Done when` criterion holds on the committed tree, with the evidence in
`# Result`:

1. **Both variants validate and run through the existing CLI without source
   changes.** `both_variants_validate_and_run_through_the_cli` invokes
   `tangle-cli validate` and `tangle-cli run --seed 1 --ticks 600` on each
   checked-in variant through the unmodified binary and requires a non-empty
   canonical trace from each; the variants' traces differ at that seed.
2. **The variants are equivalent except for the independent variable, and the
   equivalence is stated.** The suite proves it twice — only the `id` line and
   the six `duration_s` lines differ byte-for-byte, and the parsed
   `ScenarioSource` documents are equal once the id and every phase duration are
   normalized — and `# Result` names the differing fields and the controlled
   data explicitly.
3. **The experiment spec and seed bank are checked in.**
   `experiments/increment6_signal_timing_v1/experiment.json` and
   `seed_bank.json` are committed,
   `the_experiment_spec_and_bank_name_the_checked_in_inputs` parses both and
   checks the variants, the Phase 1 preset, the ticks/duration, the bounded
   default sampling policy, and the ordered seeds, and
   `the_checked_in_experiment_runs_paired_from_the_seed_bank` runs both variants
   from that bank through `batch` and pairs them with `compare`.
4. **A geometrically different benchmark scenario exists, is validated, and is
   shown to need no change to `tangle-model` or `tangle-sim`.**
   `scenarios/benchmarks/offset_junction_v1.json5` is a six-arm staggered
   junction that validates and runs through the CLI, is structurally different
   from the variants (3 movements, 2 conflict regions, no signals), and the
   commit that adds it (`7396058`) touches only that scenario file:
   `git diff --stat 7396058^ 7396058 -- crates/tangle-model crates/tangle-sim`
   is empty (the whole slice's range is empty there too).
5. **Schema validation, the regenerated JSON Schema, and the drift test are
   green; no schema version changes.** No schema change was needed — every field
   the new scenarios use already exists in version 1 — so
   `schemas/scenario-source.schema.json` is unchanged,
   `SUPPORTED_SCHEMA_VERSION` stays 1, and
   `checked_in_schema_matches_the_generated_schema` passes untouched.
6. **The five gates pass.** Rerun on the committed tree: 564 tests passed, 0
   failed; clippy clean with `-D warnings`; `cargo fmt --all --check` clean;
   `dependency direction OK`; `braintree check` passed (88 nodes).
