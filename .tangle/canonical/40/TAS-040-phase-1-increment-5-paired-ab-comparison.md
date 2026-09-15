---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice C2b of Phase 1 Increment 5 adds a `compare` command that pairs two batches run from one common-random-number seed bank and reports per-metric mean differences with a paired confidence interval, disaggregated by mode and movement and linked to both manifests and metric_definition_version 1.
---

# Outcome

Add a `compare` command to the Hekate CLI that performs the A/B comparison the
CRN seed bank exists for. It reads two completed batches (`--a` and `--b`) and
the seed bank they were run from, proves both sides used the same bank (same
content hash) and the same seed order, and then pairs the runs by seed.

For every reported metric it computes the per-seed paired difference
`d_i = value_A(seed_i) - value_B(seed_i)` and reports the mean difference with a
paired confidence interval (Student-t on the differences, consistent with
TAS-038's method). Results are disaggregated by mode (`ModePair`) and by
movement (the DEF-004 movement key), ordered deterministically, and each
comparison links to both run manifests and to `metric_definition_version: 1`.

Requirements:

- A missing seed on either side, a bank-hash mismatch, a different seed order, a
  duplicate seed, or a metric one side reports and the other does not is refused
  with an actionable diagnostic rather than silently unpaired.
- Not-applicable/not-observed pairs are excluded from the paired statistic and
  counted, so a metric whose pair has no value for some seeds reports its paired
  `n` rather than a fabricated zero.
- The command writes a machine-readable comparison artifact (for example
  `comparison.json`) to a named path or the default, and mutates no run artifact.
- The paired comparison is the CRN payoff: state in the node result how pairing
  differs from the unpaired per-side interval (the variance the two variants
  share is removed).

If the slice outgrows one session, land the paired engine and run-level metrics
first, commit with `Refs TAS-040`, leave the node `active` with a concrete
`next` naming the disaggregation, and report.

# Done when

- `compare` pairs two batches from one seed bank and writes a machine-readable
  per-metric paired comparison with mean difference and a documented paired
  confidence interval.
- The comparison is disaggregated by mode and by movement.
- Every comparison links to both run manifests and to
  `metric_definition_version: 1`.
- Unpaired or mismatched inputs (missing seed, bank-hash mismatch, duplicate
  seed, differing metric availability) are refused, covered by tests.
- The ordering is deterministic, covered by a test.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `tangle check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

TAS-039 (C2a) added the versioned seed bank, the `seed-bank` command, and
`batch --seed-bank` with the bank identity in the batch manifest. TAS-038 (C1b)
added across-seed aggregation with a documented Student-t interval. This child
combines them into the paired A/B comparison the plan asks for. The Fast/
Standard/Fine convergence runner and sensitivity report (D) will consume this.

# Result

Claimed by `worker` at base hash
`f84e0a94ab8f7c48747069f26eafc7f4dad1f41c1e2cdae67c4b54c22dbea26e`
(`tangle hash TAS-040`, bare-ID form) with `--lease-seconds 14400`. Write set:
`apps/hekate-cli/src/**`, `apps/hekate-cli/tests/**`, and this node's own frontier
transition (`nodes/proposed/` → `nodes/active/`). Every changed, created, and
moved path stayed inside that set. `apps/hekate-cli/Cargo.toml` and `Cargo.lock`
are unchanged: no dependency was added. No kernel file was edited and no file
under `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/` changed.

## What landed

`apps/hekate-cli/src/compare.rs` (new) owns the comparison.

`compare_batches(a_root, b_root, bank_path)` reads the seed bank through
TAS-039's `read_seed_bank`, then both `batch.json` manifests, then every paired
run's `manifest.json` and `metrics.json`, and returns `comparison.json`
(`COMPARISON_VERSION` 1)
without writing anything. Before a single value is paired it proves the inputs
match:

- **one bank**: each batch manifest must name a bank, the two must record the
  same `content_sha256`, and the bank file the command was given must hash to
  that value, so a comparison cannot be assembled from two banks that happen to
  hold the same seeds;
- **one order**: each manifest's `seeds` must be the bank's ordered seed list —
  same length, same seed at every position — so pairing is by position without
  re-sorting either side;
- **every seed paired**: a repeated seed, a bank seed with no run, and a run the
  manifest's own seed list does not hold are each refused rather than skipped;
- **every run unchanged**: a run manifest whose bytes do not hash to what the
  batch recorded, a metrics artifact linking to another manifest, and an
  artifact from another metric definition revision are refused, exactly as
  `aggregate` refuses them;
- **one metric set**: the first seed read fixes the comparison's metric set, and
  any run that reports or omits a metric the others do not is refused with
  `MetricAvailability`, so a metric one side reports and the other does not can
  never be silently unpaired.

The artifact holds the documented method (`PairedMethod`: method
`paired_student_t`, confidence level 0.95, denominator `n - 1`, the paired
difference `a - b`, the critical-value table range and the normal fallback, and
the least seeds an interval needs), the seed bank identity
(`SeedBankReference`), both batch links (`ComparedBatch`: manifest path, SHA-256,
format version, and the bank path and hash that batch recorded), the paired run
table (`pairs`, in the bank's order, each side's directory, `manifest_sha256`,
and `metrics_sha256`), and three slice families:

- `metrics` — the whole-batch metrics keyed by the run artifact's own metric
  names (`minimum_ttc_s`, `minimum_separation_m`,
  `minimum_post_encroachment_s`, `event_counts.total`,
  `event_counts.by_family.<family>`, and
  `event_counts.by_family_kind.<family>.<kind>`), read through `aggregate`'s own
  `run_level_readings`, so the metric set and the units stay one spelling;
- `mode_pair_slices` — keyed by the `ModePair` label, holding that pair's
  `minimum_separation_m`;
- `movement_slices` — keyed by the DEF-004 movement key (the pair's two
  movement keys sorted and joined with `|`), holding the bucket's
  `minimum_separation_m`, `minimum_ttc_s`, and `minimum_post_encroachment_s`.

Every distribution is `PairedDistribution`: `metric_definition_version: 1`, the
unit, the paired `count`, `mean_difference`, the `spread` of the differences,
the paired `confidence_interval`, the `paired_seeds` and its side-A and side-B
`manifests` in the same order, and the `unpaired` seeds with each side's
`MetricStatus`. A pair enters the statistic only when both sides report a value:
not-applicable and not-observed sides are counted per seed, so a metric one side
never reports reads `count: 0` and `mean_difference: null`, never a zero
difference diluted over fabricated zeros. A sparse slice covers the whole
comparison — the seeds a bucket does not reach are counted unobserved on both
sides, mirroring `aggregate`'s `fill_not_observed` — so every distribution's
`count + unpaired` equals the comparison's pair count.

`apps/hekate-cli/src/main.rs` adds the `compare` subcommand
(`hekate-cli compare --a <BATCH_ROOT> --b <BATCH_ROOT> --seed-bank <FILE>
[--output <PATH>]`, default `comparison.json` in the current directory, `-` for
stdout) with its `--help` contract, and `lib.rs` re-exports the new surface. The
command reads only: it opens the bank, both manifests, and each paired run's two
artifacts, and writes exactly one derived file, so no run artifact of either
batch is created, changed, or removed.

`apps/hekate-cli/src/aggregate.rs` gained a small, behavior-preserving
visibility widening so the comparison reuses one spelling of the aggregation's
seam instead of duplicating it: `run_level_readings`, `movement_readings`,
`Reading`, `MODE_PAIR_METRIC`, `METRES`, `Spread::of`, `sample_mean`, and
`sample_variance` became `pub(crate)` (no behavior, error, or artifact-shape
change), and `ConfidenceInterval` gained
`labelled(method, values, mean, variance)`, with the existing `of` delegating to
it, so the paired interval is the same critical-value table and arithmetic under
the name `paired_student_t` rather than a second implementation. The aggregation
artifact, its ordering, and its error variants are unchanged, and its tests pass
untouched.

## How the paired interval differs from the unpaired per-side interval

`aggregate` reports each side alone: `mean ± t(0.975, n - 1) * s / sqrt(n)` over
one batch's per-seed values, so that standard error carries the whole
between-seed spread of that side. Comparing two such intervals compares two
means whose standard errors both contain the seed-to-seed variation both
variants share — the part that makes seed 3 slower than seed 0 for either
variant.

`compare` computes the interval on the paired differences instead:
`mean_difference ± t(0.975, n - 1) * s_d / sqrt(n)`, where `s_d` is the `n - 1`
sample standard deviation of `d_i = A_i - B_i`. The shared seed-to-seed
component cancels inside each `d_i`, so it does not appear in `s_d`: what remains
is the spread of the paired effect, which is why the paired interval is narrower
and a paired comparison is more sensitive than reading two unpaired intervals
side by side. The test fixture `A = [1, 2, 3, 4]`, `B = [0.5, 1, 2.5, 4]` makes
it concrete: the paired standard error is `sqrt(0.5/3)/2 = 0.204`, side A's
unpaired across-seed standard error is `0.645`, side B's is `0.791`, and the
unpaired difference-of-means standard error is `1.021`, so the paired half-width
is about a third of side A's. The two only agree when the sides' per-seed values
are uncorrelated, which is exactly the case the CRN seed bank exists to avoid.

## Evidence

`apps/hekate-cli/tests/compare.rs` (12 tests) plus the module's 2 unit tests,
over two synthetic batches written with hand-chosen `metrics.json` values and a
shared bank:

- `hand_computed_paired_mean_difference_and_interval` (tests/compare.rs:499)
  pins the mean difference (`0.5` over differences `0.5, 1, 0.5, 0`), the spread
  (minimum `0`, maximum `1`, variance `0.5/3`), and the interval against
  `t(3) = 3.182446305 * sqrt(0.5/3)/2`, the documented method block, both batch
  and bank links against the files on disk, the pair table's directories and
  artifact hashes, the per-seed manifest links, and the collision-count
  difference;
- `pairing_removes_the_between_seed_variance_the_sides_share`
  (tests/compare.rs:624) recomputes the two sides' unpaired standard errors by
  hand and shows the paired one is below each side's own, which is the CRN
  payoff stated in the node outcome;
- `the_mode_and_movement_slices_pair_by_mode_and_movement`
  (tests/compare.rs:676) covers all three mode pairs (including one neither side
  observes, reported as `count: 0` with no mean), the movement bucket key and its
  three metrics, and a bucket carried at one seed only, whose other seed is
  counted unobserved on both sides;
- `unpaired_values_are_counted_not_zeroed` (tests/compare.rs:816) shows a
  not-applicable and a not-observed pair counted with both sides' statuses
  (`count: 2`, mean over the two reported pairs), a metric no side ever reports
  (`count: 0`, `mean_difference: null`, `confidence_interval: null`), and the
  one-paired-seed rule (`count: 1`, no interval);
- `every_comparison_links_to_both_manifests_and_the_definition_version`
  (tests/compare.rs:897) walks every distribution in the artifact — metrics,
  mode slices, and movement slices — and checks the definition version, the
  unit, `count == paired_seeds.len()`, both sides' manifest lists against the
  pair table, ascending seed order, and that the statuses account for every pair,
  plus the batch and bank links against the files on disk;
- `the_ordering_is_deterministic` (tests/compare.rs:984) compares two batches
  whose manifests list the same runs in opposite order and finds every part but
  the batch hash identical, checks the key and seed lists ascend, and shows the
  command writes exactly the library's bytes;
- `a_disagreeing_seed_bank_is_refused`, `a_different_seed_order_is_refused`,
  `unpaired_runs_and_missing_artifacts_are_refused`,
  `a_metric_available_on_one_side_only_is_refused`, and
  `a_run_that_disagrees_with_its_batch_is_refused` (tests/compare.rs:1064,
  :1130, :1186, :1270, :1310) cover every refusal: a side without a bank, two
  different banks, a bank file the batches did not record, a seed list in
  another order, a shorter seed list, a duplicated seed, a missing run, a stray
  run, a missing metrics artifact, a changed run manifest, a metrics artifact
  linking elsewhere, another definition revision, a claimed value with none, and
  a metric one side does not report (naming
  `event_counts.by_family.yields`);
- `the_compare_command_pairs_real_batches_and_mutates_no_run_artifact`
  (tests/compare.rs:1394) runs `seed-bank`, two `batch --seed-bank` runs over two
  different scenarios (the walking skeleton and the car-following benchmark) and
  the command itself: it checks the default `comparison.json` path, the seed
  order, the bank and batch links, that every distribution accounts for both
  pairs, that at least one slice is sparse, that both batch roots are
  byte-identical before and after (and after a second run and a `--output -`
  run), and that a mismatched bank exits 1 and writes no artifact.

The module's unit tests pin the accumulator's pairing (both sides must report for
a pair to enter the statistic) and the paired interval arithmetic.

## Gates

All five passed on the implementation tree (`b43745b`), rerun after the commit:
`cargo test --workspace --all-features` (537 tests, 0 failures),
`cargo clippy --workspace --all-targets --all-features -- -D warnings` (clean),
`cargo fmt --all --check` (clean), `./scripts/check-dependency-direction.sh`
(`dependency direction OK`), and `tangle check nodes` (`graph check: passed
(70 nodes)`). No file under `tests/golden/`, `baselines/`, `scenarios/`, or
`schemas/` changed, and `EVENT_VERSION` stays 2.

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`b43745b`), verified by rerunning the five gates on that tree:

1. **`compare` pairs two batches from one seed bank and writes a
   machine-readable per-metric paired comparison with mean difference and a
   documented paired confidence interval.** `compare_batches`
   (`compare.rs:529`) reads the bank and both batches;
   `PairedDistribution` (`compare.rs:459`) carries `count`,
   `mean_difference`, `spread`, and `confidence_interval`, computed by
   `PairedAccumulator::finish` (`compare.rs:1200`) from the per-seed
   differences; the interval is
   `ConfidenceInterval::labelled(PAIRED_INTERVAL_METHOD, ..)`
   (`compare.rs:409`, `aggregate.rs:409`), the two-sided Student-t interval of
   the differences, and `PairedMethod::v1` records the method, level, `n - 1`
   denominator, `a - b` difference, table range, and least-seeds rule in the
   artifact. `main.rs:114` declares the subcommand, `main.rs:648` its arguments,
   and `main.rs:672` writes the file (default `comparison.json`, `-` for
   stdout). Asserted by `tests/compare.rs:499` (hand-computed mean difference and
   interval) and `tests/compare.rs:1394` through the binary.
2. **The comparison is disaggregated by mode and by movement.**
   `Comparison.mode_pair_slices` (`compare.rs:714`) is keyed by the `ModePair`
   label and holds that pair's `minimum_separation_m`; the movement loop
   (`compare.rs:659`) keys each bucket by the DEF-004 movement key and
   `ComparedMovementSlice` (`compare.rs:486`) holds the bucket's three
   interaction metrics with its sorted `movement_keys`. Asserted by
   `tests/compare.rs:676` (all three mode pairs and the movement bucket key,
   metrics, and one-seed-only bucket) and by `tests/compare.rs:1394` on two real
   scenarios with different movements.
3. **Every comparison links to both run manifests and to
   `metric_definition_version: 1`.** `ComparedBatch` (`compare.rs:367`) records
   each side's manifest path, SHA-256, format version, and the bank it recorded;
   `PairedDistribution` carries `metric_definition_version` and both
   `a_manifests` and `b_manifests` in `paired_seeds` order, and `ComparedPair`
   (`compare.rs:394`) records each side's run directory, `manifest_sha256`, and
   `metrics_sha256`. `check_run` (`compare.rs:869`) refuses a run whose bytes
   changed, whose metrics link elsewhere, or whose revision differs. Asserted by
   `tests/compare.rs:897`, which walks every distribution in the artifact and
   checks the definition version, the unit, both manifest lists against the pair
   table, and the batch and bank links against the files on disk.
4. **Unpaired or mismatched inputs (missing seed, bank-hash mismatch, duplicate
   seed, differing metric availability) are refused, covered by tests.** The
   bank proof is at `compare.rs:545` (no bank, two banks, a bank file the
   batches did not record), the order proof at `check_seed_order`
   (`compare.rs:792`), the run index at `index_runs` (`compare.rs:820`: duplicate
   seed, missing run, stray run), the artifact guards at `check_run`
   (`compare.rs:869`), and the metric-set proof at `check_metric_set`
   (`compare.rs:1045`). Covered by `tests/compare.rs:1064`, `:1130`, `:1186`,
   `:1270`, and `:1310`.
5. **The ordering is deterministic, covered by a test.** The pairs follow the
   seed bank's order, every map is a `BTreeMap`, and `fill_missing`
   (`compare.rs:1178`) walks the seeds in bank order, so the seed lists are
   ascending. Asserted by `tests/compare.rs:984`, which compares two batches
   whose manifests list the same runs in opposite order, checks every key and
   seed list ascends, and shows the command writes exactly the library's bytes.
6. **The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
   all goldens are unchanged; `EVENT_VERSION` stays 2.** `b43745b` touches only
   `apps/hekate-cli/src/{aggregate.rs,compare.rs,lib.rs,main.rs}`,
   `apps/hekate-cli/tests/compare.rs`, and this node; no file under
   `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/` changed,
   `EVENT_VERSION` stays 2 (`crates/hekate-sim/src/event.rs:66`),
   `Cargo.toml`/`Cargo.lock` are unchanged, so no dependency was added, and the
   `metrics.json`, `aggregation.json`, and `batch.json` shapes are read, not
   modified. The golden and baseline guards in gate 7 passed.
7. **The five gates pass on the final tree** (rerun on `b43745b`, 2026-09-13):
   `cargo test --workspace --all-features` passed with 537 tests and 0 failures;
   `cargo clippy --workspace --all-targets --all-features -- -D warnings` was
   clean; `cargo fmt --all --check` was clean;
   `./scripts/check-dependency-direction.sh` printed `dependency direction OK`;
   and `tangle check nodes` printed `graph check: passed (70 nodes)`.

Outcome complete: the paired engine, its documented paired interval, the mode
and movement disaggregation, the manifest and definition-version links, the
counted unpaired seeds, the deterministic ordering, and the refusal of every
mis-pairing input are on the committed tree.

## Handoff note

`TAS-031`'s `next` already names this node and its exclusive write set excludes
the parent, so this worker did not edit it; it was advanced past this node by the
coordinator before the handoff, so no pointer change is owed. One friction note
about reusing a resolved sibling's seam by widening visibility (rather than
duplicating it) is recorded as an `FBK` node.
