---
context_rev: 1
priority: P1
updated: 2026-09-13T02:23:09Z
summary: Slice D of Phase 1 Increment 5 adds a Fast/Standard/Fine convergence runner that evaluates a scenario at three fidelities over one seed bank and writes a machine-readable sensitivity report, disaggregated by mode and movement and linked to every run manifest and metric_definition_version 1.
next: Resolve the node once the five gates pass on the committed tree, recording per-criterion evidence in `# Resolution`.
---

# Outcome

Add a convergence runner that evaluates one scenario at the Fast, Standard, and
Fine fidelities over a shared seed bank and writes a machine-readable
sensitivity report. For every reported metric the report gives the value at each
fidelity, the change as the step refines (Fast to Standard to Fine), and a
convergence verdict against a declared tolerance: a metric whose Standard-to-Fine
change exceeds the tolerance is reported as materially sensitive rather than
hidden or averaged away.

Requirements:

- Reuse the existing fidelity presets (the same Fast/Standard/Fine steps the
  baseline already uses) and the seed-bank, batch, aggregation, and paired
  machinery rather than duplicating step constants or statistics.
- The report is deterministic: declared ordering of metrics, fidelities, and
  slices; no wall time or hash-map order in the artifact (timing, if recorded,
  is clearly separated from the reproducible content).
- Disaggregated by mode (`ModePair`) and movement (the DEF-004 key) wherever the
  metric supports it.
- Every reported metric links to the run manifest(s) at each fidelity and to
  `metric_definition_version: 1`, satisfying the increment gate.
- Not-applicable/not-observed values are reported as such, never as zero.
- A `converge` CLI command with a documented contract writes the report to a
  named path or the default and mutates no run artifact.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens stay byte-identical; `EVENT_VERSION` stays 2; no dependency change
  in `tangle-model` or `tangle-sim`.

If the slice outgrows one session, land the first bounded increment (the runner
plus run-level metrics and the convergence verdict), commit with `Refs TAS-041`,
leave the node `active` with a concrete `next` naming the disaggregation, and
report.

# Done when

- `converge` runs one scenario at Fast, Standard, and Fine over a shared seed
  bank and writes a machine-readable sensitivity report.
- The report gives per-metric values per fidelity, the refinement change, and a
  convergence verdict against a declared tolerance, flagging material
  sensitivity.
- The report is disaggregated by mode and by movement.
- Every reported metric links to its manifests at each fidelity and to
  `metric_definition_version: 1`.
- The ordering is deterministic, covered by a test.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

The seed bank (TAS-039), aggregation (TAS-038), paired comparison (TAS-040), and
per-run `metrics.json` (TAS-037) are in place; [[DEF-004-metric-definition-v1]]
is the versioned definition this report cites. Increment 6's gate "selected
findings remain directionally stable at Fine fidelity; material sensitivity is
reported rather than hidden" builds directly on this report. The release-mode
benchmarks and profiler captures are the next direct child (E).

# Result

Claimed by `worker` at base hash
`d0fc48dfcb51f7314e746a9973ebc1331ff735148f65c78f09ea41f6459f52b4`
(`braintree hash TAS-041`, bare-ID form) with `--lease-seconds 14400`. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`, `apps/tangle-cli/Cargo.toml`
and `Cargo.lock` (both unused: no dependency was added), and this node's own
status move (`proposed/` → `active/` → `resolved/`). Every changed, created, and
moved path stayed inside that set. No kernel file was edited, and no file under
`tests/golden/`, `baselines/`, `scenarios/`, or `schemas/` changed.

## What landed

`apps/tangle-cli/src/converge.rs` (new) owns the runner and the report.

- **`converge_batches(fast_root, standard_root, fine_root, bank_path, tolerance)`**
  reads three completed fidelity batches through the landed machinery and
  returns `convergence.json` without writing anything. Each fidelity is read by
  `aggregate_batch` (TAS-038) and each of the two refinement steps is paired by
  `compare_batches` (TAS-040), so the report holds the aggregation's own
  `MetricDistribution` per metric per fidelity and the comparison's own
  `PairedDistribution` per refinement step. The two paired steps are also what
  proves all three batches ran the one bank the command was given, in one seed
  order; the batch manifests additionally must name one scenario
  (`content_sha256`), and each batch must run its fidelity's declared step
  (`PRESETS`) at the tick count the declared fidelity rule gives it.
- **The declared tolerance**: `CONVERGENCE_TOLERANCE = 0.05` (5%) on the
  **relative change** `|mean(fine) - mean(coarse)| / |mean(coarse)|` of a
  refinement step, where the change is the paired mean difference `fine - coarse`
  over the shared bank and the reference is the coarser fidelity's across-seed
  mean. A metric whose **standard-to-fine** relative change *exceeds* the
  tolerance (strictly greater) is `materially_sensitive`; the step and the metric
  verdict both record it rather than averaging it away. A zero coarser value has
  no relative scale: a zero change is converged and any other change is an
  unbounded relative change and material (`relative_change: null`). A step no
  seed reports at both fidelities is `inconclusive`, never a fabricated zero. The
  tolerance block is recorded in the artifact (`tolerance`), documented in the
  module docs, and printed by `converge --help`.
- **Report shape**: `convergence_version` 1, `metric_definition_version` 1, the
  `tolerance` block, the one `scenario` and `seed_bank`, `fidelities` (Fast,
  Standard, Fine in declared order, each with its `step_s`, `ticks`, batch root,
  batch manifest link, and per-seed run directory plus `manifest_sha256` and
  `metrics_sha256`), `metrics`, `mode_pair_slices`, and `movement_slices`. Each
  metric (`MetricSensitivity`) carries its `unit`, its three `fidelities`
  (`FidelityValue`: the across-seed `MetricDistribution`, or `null` when that
  fidelity's aggregation carries no such slice at all), its two `refinements`
  (`RefinementStep`: the paired `PairedDistribution`, the `relative_change`, and
  the `materially_sensitive` flag), and the `verdict`.
- **Disaggregation**: `mode_pair_slices` is keyed by the `ModePair` label and
  `movement_slices` by the DEF-004 movement-key pair, both reusing the
  aggregation's and the comparison's own slice keys and metric names; a bucket
  one fidelity does not carry is a `null` value there and a paired distribution
  that counts every seed as no observation, never a zero.
- **Links and statuses**: every distribution carries
  `metric_definition_version: 1`, the aggregation's `manifests` per reported
  seed, the comparison's `a_manifests`/`b_manifests` per paired seed, and the
  seeds behind each reporting status (`not_applicable_seeds`,
  `not_observed_seeds`, `unpaired`), exactly as `aggregate` and `compare`
  report them.
- **Determinism**: the fidelities and refinement steps are in the declared
  Fast → Standard → Fine order, every map is a `BTreeMap`, and every seed list is
  ascending, so the report reads no clock, no randomness, and no hash-map order.

`apps/tangle-cli/src/main.rs` adds the `converge` command
(`tangle-cli converge <SCENARIO> --seed-bank <FILE> --out-root <DIR>
[--ticks <N>] [--output <PATH>]`, default `convergence.json` in the current
directory, `-` for stdout) with its `--help` contract. `--ticks` is the Standard
fidelity's fixed step count (default 250, the 12.5 s the golden trace covers)
and `fidelity_ticks(step_s, standard_ticks)` derives the Fast and Fine counts
from it, because the presets differ in step size and a fixed tick count would
compare different simulated horizons. The command runs one batch per preset into
`out-root/fast`, `out-root/standard`, and `out-root/fine` through `run_batch`
(TAS-035) with the one seed bank, then builds the report from the roots. The
batch machinery's resume path means a completed run directory is never
rewritten: re-invoking the command changes no run artifact and rewrites the same
report bytes.

`apps/tangle-cli/src/lib.rs` re-exports the new surface. No dependency was
added: `apps/tangle-cli/Cargo.toml` and `Cargo.lock` are unchanged.

## Evidence

`apps/tangle-cli/tests/converge.rs` (7 tests) over three synthetic fidelity
batches built with hand-written `metrics.json` files (so every value is exact)
and one command-level run over a real scenario:

- `an_engineered_refinement_change_above_the_tolerance_is_flagged` (:517)
  engineers separation 0.5 → 1.0 → 1.5 and shows both refinement steps over the
  tolerance being flagged materially sensitive, TTC 1.0 → 1.0 → 1.01 staying
  converged at 1%, and collisions 0 → 0 → 1 reading the zero-reference rule, with
  the across-seed means and paired differences asserted exactly.
- `a_status_is_reported_not_zeroed_and_an_unmeasurable_change_is_inconclusive`
  (:629) shows a not-applicable and a not-observed metric reported as statuses
  with no mean, no interval, and `unpaired` entries carrying both statuses, and
  a zero change on a defined reference converging.
- `the_report_is_disaggregated_by_mode_and_by_movement` (:690) covers the
  `ModePair` slices (a label no run observed stays inconclusive), a movement
  bucket the Fast fidelity does not carry (`value: null` there, every seed
  counted no-observation in the paired step), and a bucket only the coarsest
  fidelity carries, whose second refinement has no paired distribution at all.
- `every_metric_links_to_its_manifests_and_the_definition_version` (:849) walks
  every metric, mode slice, and movement slice and checks the definition
  version, the unit, the declared fidelity and step order, every aggregation
  manifest against that fidelity's seed table, every paired manifest against the
  right side of the step, and that paired plus unpaired seeds account for every
  bank seed.
- `the_ordering_is_deterministic` (:1045) builds the report twice (equal), checks
  the declared order and ascending keys and seed lists, and shows a batch whose
  runs are listed in reverse changing nothing but its own manifest hash.
- `batches_that_do_not_share_one_step_duration_or_scenario_are_refused` (:1127)
  covers a batch at another step, at another simulated duration, naming another
  scenario, and naming another bank.
- `the_converge_command_runs_three_fidelities_over_one_bank_and_mutates_nothing`
  (:1254) runs the real command over `car_following_v1` with a two-seed bank and
  60 Standard ticks: it checks the three fidelities' steps and tick counts (30 /
  60 / 150), the one scenario and one bank, the mode and movement slices, that at
  least one metric is materially sensitive and at least one inconclusive, that
  every verdict is its standard-to-fine reading, that a missing value is never a
  zero, that a second invocation leaves every file under all three batch roots
  byte-identical and the report byte-identical, that `--output -` writes the same
  bytes, and that an incompatible out-root exits 1 writing no report.

The module's own unit tests (:738, :750, :762) pin the declared fidelity order
and the Standard step the rule reads, the Fast/Standard/Fine tick counts for two
durations, and the tolerance rule itself: a change of exactly the tolerance is
converged, a negative change is measured by magnitude, a zero reference is
bounded only by the zero rule, and no measurable change is not a converged one.

## Preservation

Only `apps/tangle-cli/src/{converge.rs,lib.rs,main.rs}` and
`apps/tangle-cli/tests/converge.rs` changed; `Cargo.toml`, `Cargo.lock`, every
kernel crate, and every file under `tests/golden/`, `baselines/`, `scenarios/`,
and `schemas/` are untouched, so the canonical trace, the trace golden, the
trace hash golden, the Phase 1 baseline, and all goldens stay byte-identical and
`EVENT_VERSION` stays 2. No existing artifact shape changed: `converge` reads
`batch.json`, `manifest.json`, and `metrics.json` and writes one new derived
file.
