---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice C of Phase 1 Increment 6 runs the two variants into immutable run directories and produces the concise comparison report over throughput, delay, queues, violations, collision/contact events, TTC, PET, and minimum separation by mode and movement, adding the F6 event-family aggregation and per-metric convergence tolerance that the report depends on.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Consumes the variant scenarios and experiment spec from
[[TAS-047-increment-6-scenario-variants-and-experiment-spec]], the v2 metrics
from [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]], and the
Increment 5 `batch`, `aggregate`, `compare`, and `converge` surfaces.

Two Increment 5 residuals are folded into this slice because it owns the
comparison report:

- **F6:** the batch-level mode slice carried only `minimum_separation_m`, and the
  run artifact's event-family mode/movement counts were not aggregated. The gate
  requires collision/contact events, violations, and queue counts by mode and
  movement, so aggregation must carry them.
- **Per-metric convergence tolerance:** convergence used one global 5% relative
  tolerance, so count metrics (0 → 1 is unbounded) commonly flagged as
  materially sensitive. A per-metric tolerance must let the report distinguish
  material sensitivity from noise.

# Outcome

- Both variants are run into immutable run directories from the checked-in spec
  and seed bank.
- A concise comparison report reports throughput, delay, queues, violations,
  collision/contact events, TTC, PET, and minimum separation by mode and
  movement.
- `aggregate` carries the full mode and movement slices, including event-family
  counts (F6 closed).
- `converge` applies a per-metric relative tolerance, and the comparison report
  reports material sensitivity rather than hiding it.

# Done when

- Immutable run directories exist for both variants across the specified seeds,
  with manifests, summaries, event streams, and sampled trajectories consistent
  with the declared sampling policy.
- The comparison report reports throughput, delay, queues, violations,
  collision/contact events, TTC, PET, and minimum separation by mode and
  movement, and every number links to its manifest(s) and its
  `metric_definition_version` value (2 for the new metrics).
- Aggregation carries the full mode and movement slices including event-family
  counts (F6 closed), with tests.
- Convergence uses a per-metric relative tolerance and the report states material
  sensitivity per metric; tests cover the tolerance rule.
- Parallel and serial batch execution produce identical per-run trace hashes.
- No existing golden or baseline is broken without a deliberate, reported
  regeneration.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `tangle check`.

# Result

Landed in one session; nothing was deferred. Claimed by `worker` at base hash
`7ccf8bc6bfc6e50297c6d252f6a4654c152cf81ce1775bcca5a76576e0db03f3`
(`tangle hash TAS-048`, bare-ID form, FBK-002) with `--lease-seconds 14400`,
and moved to `.tangle/active/` with `git mv` plus `git add` of the
destination (FBK-013). Write set held: `apps/hekate-cli/src/**`,
`apps/hekate-cli/tests/**`, `experiments/increment6_signal_timing_v1/**`, and
this node's own status move. No file under `crates/`,
`schemas/`, `baselines/`, or `tests/golden/` changed, and no dependency was
added.

## What landed

Three commits, each green on its own:

- `3b34e84` **F6: the mode and movement event slices.** The aggregation carries
two new slice families — `mode_event_slices` keyed by one `AgentMode` label and
`agent_movement_slices` keyed by one movement key — holding the counted event
families (and, for a movement, the ten operational metrics of that movement),
and the paired comparison now pairs both families. A family a mode or movement
recorded no record for is a reported `0`, because the run artifact's own
`by_family` set always holds all ten; a movement bucket a run does not carry at
all stays `not_observed`, like every other sparse slice. Records:
`the_event_slices_disaggregate_the_counted_families_by_mode_and_movement`,
`the_event_families_pair_by_mode_and_by_agent_movement`, the extended link and
ordering walks, and the extended real-batch slice test.
- `575bca4` **per-metric convergence tolerance.** `MetricTolerance` replaces the
one global number: material when
`|fine - coarse| > absolute + relative x |coarse|`, with a continuous metric
(seconds, metres, agents per second) at 5% and no absolute part and a countable
metric (records, agents) at 5% plus one whole unit. `0 -> 1` records is now
noise rather than an unbounded relative change, `0 -> 3` is still material, and
every metric record carries the class, relative part, and absolute part it was
read against. `tolerance_is_per_metric_because_a_count_has_an_absolute_part`
plus the rewritten unit test cover the rule; the engineered-change test now
reads `0 -> 0 -> 1` as converged and the tolerance block is asserted field by
field.
- `0f6616b` **the experiment run and the checked-in comparison report.**
`apps/hekate-cli/src/experiment.rs` adds `run_experiment` (read the spec, run
each variant through the batch machinery, aggregate each, pair them, build the
report) and the `experiment` command, and
`experiments/increment6_signal_timing_v1/comparison_report.json` is its output
for the checked-in spec.

## The run directories

The report was produced with

```sh
hekate-cli experiment experiments/increment6_signal_timing_v1/experiment.json \
  --run-root experiments/increment6_signal_timing_v1/runs \
  --jobs 8 \
  --output experiments/increment6_signal_timing_v1/comparison_report.json
```

which runs both variants at the spec's Standard fidelity (0.05 s, 6000 ticks,
300.0 s) over all ten bank seeds, into
`experiments/increment6_signal_timing_v1/runs/<variant>/seed-<n>/`, each holding
`manifest.json`, `summary.json`, `events.jsonl.gz`, `trajectories.parquet`, and
`metrics.json` under the spec's declared sampling policy (`events: all`,
trajectories sampled at stride 10 with a 100000-sample cap; 244 KB per run,
4.9 MB for the 20 runs). Those directories are large and regenerable, so they
are not checked in: the nested
`experiments/increment6_signal_timing_v1/.gitignore` ignores `runs/`. Slice D
regenerates them byte-for-byte from the checked-in spec by re-running the
command above — the batch machinery's resume path means a completed run
directory is never rewritten, and the report names each variant's root, so the
run tables resolve to `<root>/<seed-n>/manifest.json`.

## The report

`comparison_report.json` (`report_version` 1, `metric_definition_version` 2)
holds 254 metric records in 33 slices across the ten declared gate sections, for
two variants over ten seeds. Every record carries both variants' across-seed
distributions and, where the comparison pairs the slice, the paired difference
`side_a - side_b` (side A is `ew_priority`).

- **By mode**: `vehicle` and `pedestrian`, each reporting throughput, travel
time, stopped delay, control delay, queue length and duration, queue events,
violations, collisions, near misses, and every other counted family.
- **By movement**: `movement:ew_through`, `movement:ns_through`, and the four
pedestrian routes, each reporting that movement's ten operational metrics and
its counted families; plus the movement-pair slices holding the pair minima.
- **Minimum separation** by `ModePair` (`vehicle_vehicle`,
`vehicle_pedestrian`, `pedestrian_pedestrian`) and by movement pair; **time to
collision** and **post-encroachment time** by movement pair and over the run.

Every number links to its manifest(s): each variant's `runs` table names the
seed, run directory, `manifest_sha256`, and `metrics_sha256`, each record's
`reported_seeds` name the seeds behind its numbers, `ReportLinks.manifests`
states the resolution rule, and a missing value is `not_applicable_seeds` or
`not_observed_seeds`, never a zero. The test re-derives a run-level operational
value, one mode's collision count, one movement's queue events, and one
movement's throughput from the run artifacts on disk and checks the hashes of
every run table entry.

The headline result is the one the increment's plan predicted and TAS-047 saw ad
hoc: the east-west movement's mean control delay is 14.18 s under `ew_priority`
and 24.44 s under `ns_priority` (paired mean difference -10.26 s), the two
movements swap the longer green, and the whole-intersection aggregates stay
within about a second per vehicle.

## Parallel equals serial

```sh
hekate-cli experiment experiments/increment6_signal_timing_v1/experiment.json \
  --run-root /tmp/t48serial --jobs 1  --output /tmp/t48serial-report.json
hekate-cli experiment experiments/increment6_signal_timing_v1/experiment.json \
  --run-root /tmp/t48par    --jobs 8  --output /tmp/t48par-report.json
cmp /tmp/t48serial/ew_priority/batch.json /tmp/t48par/ew_priority/batch.json
cmp /tmp/t48serial/ns_priority/batch.json /tmp/t48par/ns_priority/batch.json
```

Both `batch.json` files are byte-identical, so every one of the 20 per-run
`trace_sha256` and `manifest_sha256` values is identical between serial and
parallel execution (both variants' seed-1 trace hashes are
`31bc569efeaa...` and `c62604b17210...` respectively). The same property is
pinned by `a_parallel_experiment_matches_a_serial_one_and_rewrites_nothing`,
which runs the command at `--jobs 1` and `--jobs 8`, compares the batch manifest
bytes and the reports field for field, and then shows a re-run leaving every run
artifact byte-identical and the report unchanged.

## Preservation

No golden or baseline was touched or regenerated: the walking trace, trace
hash, scene golden, Phase 1 baseline, and cell/Kitty goldens all pass unchanged,
and `tests/golden/`, `baselines/`, and `schemas/` are unmodified.
`SUPPORTED_SCHEMA_VERSION` stays 1, no scenario or schema changed, and no
scenario-source schema regeneration was needed. `EVENT_VERSION` stays 2. No
kernel file changed: the whole diff is `apps/hekate-cli` plus the checked-in
experiment artifacts and the FBK node.

## Gates

All five green on the code tree (`3b34e84`, `575bca4`, `0f6616b`), and `tangle
check` re-run green on the tree this Result lands in:

- `cargo test --workspace --all-features`: 576 passed, 0 failed, 1 ignored
  (the opt-in release benchmark), across 53 test binaries. Twelve tests are new:
  five in `apps/hekate-cli/tests/experiment.rs`, one each in the aggregate,
  compare, and converge suites, one `run_metrics` unit test, and the rewritten
  tolerance tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt --all --check`: clean.
- `./scripts/check-dependency-direction.sh`: `dependency direction OK`.
- `tangle check`: `graph check: passed (89 nodes)`.

## Friction

Recorded as `FBK-021`: DEF-005 is resolved and is what a consumer reads for
metric semantics, but it deferred the aggregation slice shape to this node, so
the F6 slice vocabulary (two new families, the
count-`0`-versus-`not_observed` rule, and the per-slice metric-key spelling) has
no versioned definition coverage, and an additive slice does not bump
`metric_definition_version`. The same note records that the run artifact spells
its sparse slices in opposite directions (`by_mode`/`by_movement` subject-first,
`by_family_mode`/`by_family_movement` family-first), which read alike and fail
silently.

## Closeout

The node stayed `active` until the coordinator advanced TAS-045's `next` from
this node to the evidence-and-reproduction slice (commit `bf0db10`,
`tangle check` green), because moving it to resolved while that route still
named it fails `tangle check` (FBK-011/012/017). The `Closes TAS-048` commit
moves this node to `.tangle/resolved/` with its `next` removed.

# Resolution

Every `# Done when` criterion holds on the committed tree, with the evidence in
`# Result`:

1. **Immutable run directories exist for both variants across the specified
   seeds, with the declared artifacts and sampling policy.** Both variants ran
   all ten bank seeds at the spec's Standard fidelity into
   `experiments/increment6_signal_timing_v1/runs/<variant>/seed-<n>/` through
   the batch machinery, whose completed run directories are never rewritten (the
   parallel test re-runs a completed experiment and shows every artifact
   byte-identical). The policy is the spec's declared bounded default, and the
   report records it.
2. **The comparison report reports throughput, delay, queues, violations,
   collision/contact events, TTC, PET, and minimum separation by mode and
   movement, and every number links to its manifest(s) and to
   `metric_definition_version` 2.** `comparison_report.json` holds 254 records
   across ten sections and 33 slices, including the two modes, the four
   pedestrian routes, and the two vehicle movements; every record states version
   2 and its unit, each variant's `runs` table resolves a seed to its
   `manifest_sha256`, and
   `the_checked_in_report_matches_the_checked_in_inputs` and
   `the_experiment_runs_both_variants_over_one_bank_and_reports_by_mode_and_movement`
   check the coverage, the attribution, and four re-derived numbers.
3. **Aggregation carries the full mode and movement slices including
   event-family counts (F6 closed), with tests.** `mode_event_slices` and
   `agent_movement_slices` in `aggregate.rs`, the paired equivalents in
   `compare.rs`, and the tests named in `# Result`.
4. **Convergence uses a per-metric relative tolerance and the report states
   material sensitivity per metric; tests cover the tolerance rule.**
   `MetricTolerance` per metric, carried by every `MetricSensitivity` and
   declared in the report's tolerance block, with the rewritten unit test and
   `the_tolerance_is_per_metric_because_a_count_has_an_absolute_part`.
5. **Parallel and serial batch execution produce identical per-run trace
   hashes.** Both checked-in variants' `batch.json` files are byte-identical
   between `--jobs 1` and `--jobs 8`, and the property is pinned by the parallel
   test.
6. **No existing golden or baseline is broken without a deliberate, reported
   regeneration.** All of `tests/golden/`, `baselines/`, and `schemas/` are
   unmodified and their tests pass; no regeneration was needed or performed.
7. **The five gates pass.** `cargo test --workspace --all-features` 576 passed,
   0 failed, 1 ignored; clippy clean with `-D warnings`; `cargo fmt --all
   --check` clean; `dependency direction OK`; `tangle check` passed (89
   nodes) on the tree this Resolution lands in.

- Immutable run directories exist for both variants across the specified seeds,
  with manifests, summaries, event streams, and sampled trajectories consistent
  with the declared sampling policy.
- The comparison report reports throughput, delay, queues, violations,
  collision/contact events, TTC, PET, and minimum separation by mode and
  movement, and every number links to its manifest(s) and its
  `metric_definition_version` value (2 for the new metrics).
- Aggregation carries the full mode and movement slices including event-family
  counts (F6 closed), with tests.
- Convergence uses a per-metric relative tolerance and the report states material
  sensitivity per metric; tests cover the tolerance rule.
- Parallel and serial batch execution produce identical per-run trace hashes.
- No existing golden or baseline is broken without a deliberate, reported
  regeneration.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `tangle check`.
