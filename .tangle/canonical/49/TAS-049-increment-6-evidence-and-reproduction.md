---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice D of Phase 1 Increment 6 lands the reproduction and honesty evidence - golden traces, Fast/Standard/Fine convergence evidence with material sensitivity reported, a known-limitations document, and a one-command reproduction of the comparison.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Consumes the comparison report and per-metric convergence tolerance from
[[TAS-048-increment-6-run-and-comparison-report]]. The Increment 5 runner
(`TAS-041`) already produces a machine-readable sensitivity report; this slice
records the Fast/Standard/Fine evidence for the Increment 6 comparison and makes
the material-sensitivity outcome explicit. Golden traces follow the Increment 4/5
determinism contract exercised by `replay --verify`.

# Outcome

- Golden traces for the canonical event stream (and, where already supported, the
  sampled trajectory or scene golden), regenerated deliberately if and only if a
  reported change requires it.
- Fast/Standard/Fine convergence evidence for the comparison, recording which
  selected findings are directionally stable at Fine fidelity and which are
  materially sensitive, per metric.
- A known-limitations document naming unvalidated claims, model limitations, and
  accepted boundaries (including the Increment 5 residuals dispositioned by
  [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]]).
- A one-command reproduction of the comparison from the checked-in spec and seed
  bank.

# Result

Claimed by `worker` at base hash
`b2f3f8c069ec70dfe9a28f5f133066c16ab2bb4c4b88e876b3154aa2fc04eecd`
(`tangle hash TAS-049`, bare-ID form, FBK-002) with `--lease-seconds 14400`,
and moved to `.tangle/active/` with `git mv` plus `git add` of the destination
(FBK-013). Write set held: `apps/hekate-cli/src/**`, `apps/hekate-cli/tests/**`,
`tests/golden/**`, `experiments/increment6_signal_timing_v1/**`,
`docs/known_limitations.md`, `scripts/reproduce-increment6.sh`, and this node.
No file under `crates/`, `schemas/`, or `baselines/` changed, no dependency was
added, and `EVENT_VERSION` stays 2. `Cargo.toml` and `Cargo.lock` are unchanged.

## What landed

Six commits, each green on its own:

- `252f885` **the two missing convergence slice families.** The convergence
  report mirrored the whole batch, the mode pair, and the movement pair only, so
  the F6 families slice C reported — a mode's counted event families and one
  agent movement's operational values — could be materially sensitive in the
  comparison and silent in the evidence. `ConvergenceReport` now carries
  `mode_event_slices` and `agent_movement_slices` beside the three it had, each
  with its own per-metric tolerance and verdict, and `sensitivities()` walks all
  five families in the declared order (`SliceFamily`, `SLICE_FAMILIES`,
  `SLICE_KEY_RUN`, `SliceSensitivities`, `converge.rs`). The three single-key
  families share one walk (`single_key_slices`) instead of one copy each.
- `68b7990` **the Increment 6 convergence evidence and its summary.**
  `experiment --convergence <PATH> --summary <PATH>` runs each variant's Fast and
  Fine batches beside its Standard one — the Standard batch is the variant's own
  run root, the batch the comparison report read, so the two artifacts cannot
  disagree about it — and writes `convergence_evidence.json`
  (`ExperimentConvergence`) plus `convergence_summary.md`
  (`render_convergence_summary`). Every metric of every slice family is judged
  per variant, and the projection states the seed bank and per-fidelity seed
  tables once per variant rather than repeating the per-seed pairing per metric
  (`project_variant`, `MetricConvergence`, `SliceConvergence`,
  `VariantConvergence`). The spec gains an additive, optional
  `selected_findings` block (`SelectedFinding`); a finding no variant reports is
  refused rather than reported as an absent value (`ExperimentError::Finding`),
  and a spec that declares another fidelity than the Standard one is refused
  (`ExperimentError::ConvergenceFidelity`).
- `aefc096` **the Fast/Fine defect.** See below; it also carries the regenerated
  evidence the fix corrects.
- `07543c7` **the golden traces and their test.**
  `tests/golden/four_leg_pedestrian_ew_priority_v1.trace.{jsonl,sha256}` and the
  `ns_priority` pair, at the declared seed 1 over 1200 steps of the 50 ms preset,
  pinned by `apps/hekate-cli/tests/increment6_trace.rs`.
- `5ae5338` **the known-limitations document and the reproduction script.**
  `docs/known_limitations.md` and `scripts/reproduce-increment6.sh`.

## The Fast/Fine defect (found here, fixed here, and reported)

`run_batch` recorded `BatchRequest::step_s` in every run manifest and in
`batch.json`, but ran the kernel with the default step:

```
manifest: experiments/increment6_signal_timing_v1/runs/convergence/ew_priority/fast/seed-1/manifest.json
  "step_s": 0.1, "ticks": 3000
event stream's own run header:
  {"kind":"run",...,"step_s":0.05,"ticks":3000}
summary elapsed_s: 150.0 (3000 x 0.05, not 3000 x 0.1)
replay --verify: exit 1, "the reproduced canonical event stream does not equal the recorded stream,
  first differing byte at offset 122 (recorded 122911 bytes, reproduced 192826 bytes)"
```

The consequences were not cosmetic. Fast and Fine batches ran at the Standard
step, so `converge` was a horizon sweep at one step rather than a fidelity
refinement, the committed convergence evidence of `68b7990` was not fidelity
evidence at all, and `replay --verify` failed on every batch run directory whose
declared step was not the default — which is how
`scripts/reproduce-increment6.sh` found it. `run_batch` resume cannot detect the
stale runs either, because a pre-fix manifest is self-consistent: it records the
step it was asked for while holding a stream at another step.

`execute_run` now applies the requested step, as `baseline::capture`
(`baseline.rs:241`) and `replay` (`replay.rs:195`) already did. The regression
test `apps/hekate-cli/tests/batch.rs::a_batch_runs_at_its_declared_step_and_replays_at_it`
holds the three records against each other — the manifest, the stream header,
and the summary's elapsed time — and replays the recorded manifest; it fails on
the old behavior at the stream-header assertion. The convergence runs under
`runs/convergence/` were removed and rebuilt, and the checked-in comparison
report is byte-identical (its Standard batch already ran at the default step).
`apps/hekate-cli/tests/converge.rs`'s expectation that the benchmark's
refinement "must flag a material sensitivity" only held because Fast and Fine
ran at the wrong step; it now asserts the declared rule instead (each fidelity's
runs cover one simulated duration at its own step, read from the run summaries)
and keeps the status-handling expectation. The resolved TAS-041 node (which owns
the convergence runner) is unaffected in its body and is now more true than it
was: its three fidelities really do run at their own steps. Recorded as
`FBK-022`.

## The convergence evidence, per metric

`convergence_evidence.json` (`evidence_version` 1, `metric_definition_version` 2,
1,238,501 bytes) and `convergence_summary.md` (81,772 bytes) judge 254 metrics
per variant across the five declared families, at Fast 0.1 s / 3000 steps,
Standard 0.05 s / 6000 steps, and Fine 0.02 s / 15000 steps — each covering the
spec's one 300 s duration — over the ten bank seeds, with the per-metric
tolerance the increment settled (relative 0.05 plus one whole unit for a
countable metric, verdict on the standard-to-fine step).

- **Material sensitivity is reported per metric and not hidden.** For
  `ew_priority` the evidence reaches 168 of 254 metrics materially sensitive, 68
  converged, and 18 inconclusive; for `ns_priority`, 174 materially sensitive, 62
  converged, and 18 inconclusive. The summary lists every metric of every family
  of both variants, and the test asserts that each (slice, metric) is exactly one
  row, so nothing can be dropped from the human-readable form.
- **All three selected findings are directionally stable at Fine.** `side_a -
  side_b` at Fast / Standard / Fine, in seconds:

  | Finding | Fast | Standard | Fine | Direction |
  | --- | --- | --- | --- | --- |
  | East-west through control delay | -9.9577 | -10.2585 | -5.7776 | stable |
  | North-south through control delay | +10.2093 | +13.0397 | +9.9396 | stable |
  | Whole-run mean control delay | +0.2012 | +0.9093 | +0.9945 | stable |

  The movement-level effect is an order of magnitude larger than the run-level
  one at every fidelity, and it shrinks at Fine (about -10 s to -5.8 s for the
  east-west movement) rather than vanishing.
- **Absolute values are materially sensitive to the step in both directions.**
  The whole-run mean control delay rises from 14.73 s at Standard to 21.13 s at
  Fine (+43%), the counted event total from 1804 to 2736 records (+52%), while
  the collision count falls from 5.9 to 2.6. A value read at one fidelity must
  not be compared with a value read at another; `docs/known_limitations.md`
  states this.

## Golden traces and replay

- `tests/golden/four_leg_pedestrian_ew_priority_v1.trace.jsonl` (35,443 bytes,
  SHA-256 `a9666e0a5bfca56df071954b07a7e6fdd64d9009c27f6dd41d47c887b78f6def`)
  and `tests/golden/four_leg_pedestrian_ns_priority_v1.trace.jsonl` (39,376
  bytes, SHA-256
  `35221e91d3631bed9caedd0fe60eafdef7387d7a1b4b49469862e3013ea6cf79`), each with
  its `.sha256` file. The declared run is seed 1 over 1200 steps of the Standard
  50 ms preset — one 58 s signal cycle, so the trace spans both green phases and
  both walk intervals.
- **No existing golden or baseline was regenerated.** The walking trace, the
  trace-hash golden, the Phase 1 baseline, and the scene/cell/Kitty fixtures are
  byte-identical and their tests pass unchanged; the new goldens are new files.
  The Increment 6 variants are not part of the scene golden, which covers the
  walking scenario alone.
- `apps/hekate-cli/tests/increment6_trace.rs` (2 tests) runs the `run` command at
  the declared parameters, checks the bytes and hash against the goldens, checks
  the manifest, and replays **that same manifest** with `--verify`, comparing the
  reproduced stream with the golden. `replay --verify` also passes on all 60 run
  directories the reproduction writes (20 comparison runs and 40 convergence
  runs), which the script asserts.

## One-command reproduction

```sh
scripts/reproduce-increment6.sh
```

It builds the release CLI (or takes `HEKATE_CLI`), runs `experiment` with
`--convergence` and `--summary` into the checked-in paths, replays every recorded
manifest with `--verify`, and diffs both golden traces against a fresh run. Run
from a state with **no** `runs/` directory and no generated artifacts, it
reproduced every artifact byte-for-byte (`git status` empty afterwards) in 51 s,
verified 60 run directories, and matched both goldens. The underlying command is

```sh
hekate-cli experiment experiments/increment6_signal_timing_v1/experiment.json \
  --run-root experiments/increment6_signal_timing_v1/runs --jobs 8 \
  --output experiments/increment6_signal_timing_v1/comparison_report.json \
  --convergence experiments/increment6_signal_timing_v1/convergence_evidence.json \
  --summary experiments/increment6_signal_timing_v1/convergence_summary.md
```

## Known limitations

`docs/known_limitations.md` names what is **not** claimed: no calibration to
observed traffic, no safety prediction from the surrogate metrics, no design
conclusion beyond this model, no cross-environment determinism claim, no
statistical power claim on ten seeds, no performance claim while the always-on
interaction-metrics pass costs 18–33 µs per tick (54–75%, owned by TAS-051), and
no level of service. It names the model's limits (two modes; fixed-time control
with no actuation or preemption; longitudinal motion under IDM with three safety
caps and no reported emergency-cap counter; interaction readings bounded by the
20 m range, the 5 s TTC horizon, and the 1 m near-miss threshold; delay defined
by recorded state with no free-flow reference; agent-count queues; PET needing
two region occupancies), the Increment 5 residuals TAS-045 dispositioned, and the
boundaries of the evidence itself.

## Preservation and write set

`crates/hekate-sim` and `crates/hekate-model` are untouched: the whole diff is
`apps/hekate-cli` plus the checked-in artifacts, the goldens, the document, and
the script. No scenario, schema, or baseline changed, `SUPPORTED_SCHEMA_VERSION`
stays 1, `EVENT_VERSION` stays 2, and no dependency was added. The checked-in
`comparison_report.json` is byte-identical to `68b7990`'s except where the spec
hash moved in that commit; the convergence artifacts were regenerated by
`aefc096` because the fix changed the runs they read.

## Gates (on the final tree)

- `cargo test --workspace --all-features`: 584 passed, 0 failed, 1 ignored (the
  opt-in release benchmark), across 48 test binaries. Eight tests are new:
  `the_mode_event_and_agent_movement_slices_are_reported_per_metric`
  (`tests/converge.rs`); `the_checked_in_convergence_evidence_matches_the_checked_in_inputs`,
  `the_experiment_command_runs_the_convergence_evidence_and_summary`,
  `convergence_evidence_refuses_another_fidelity_or_an_absent_finding`, and
  `the_summary_flag_requires_the_convergence_flag` (`tests/experiment.rs`);
  `a_batch_runs_at_its_declared_step_and_replays_at_it` (`tests/batch.rs`); and
  the two in `tests/increment6_trace.rs`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt --all --check`: clean.
- `./scripts/check-dependency-direction.sh`: `dependency direction OK`.
- `tangle check`: `graph check: passed (89 nodes)`.

## Friction

Recorded as `FBK-022`: the skill's write-set and ownership rules cover reversing
an outcome in its own node and superseding one that moves, but not a later slice
that must change code a resolved slice owns to make its own deliverable true —
which here regenerated a resolved slice's derived artifact and updated a test it
wrote. The fix belongs in this node's Result; TAS-035 and TAS-041 stay read-only.

# Resolution

Every `# Done when` criterion holds on the committed tree, with the evidence in
`# Result`:

1. **Golden traces exist and are checked in; any regeneration is deliberate and
   reported, and `replay --verify` passes on the same manifest.** The two
   `tests/golden/*.trace.jsonl` and `.sha256` files are new, no existing golden or
   baseline was regenerated, and `apps/hekate-cli/tests/increment6_trace.rs`
   replays the manifest each golden was recorded from with `--verify`, comparing
   the reproduced stream with the golden. `scripts/reproduce-increment6.sh` also
   verifies all 60 run directories the reproduction writes.
2. **Convergence evidence for Fast/Standard/Fine exists as a machine-readable
   artifact plus a human-readable summary, with material sensitivity reported
   per metric rather than hidden.** `convergence_evidence.json` and
   `convergence_summary.md` are checked in, generated by
   `experiment --convergence --summary`; the summary lists every metric of every
   family of both variants as exactly one row (asserted), and the per-metric
   counts are 168 of 254 and 174 of 254 materially sensitive.
3. **The known-limitations document names the unvalidated claims and the
   residual limitations carried from Increments 4–5.** `docs/known_limitations.md`
   names seven explicit non-claims, the model's limits, the three Increment 5
   residuals TAS-045 dispositioned, the still-deferred level of service and
   free-flow reference, and the boundaries of this increment's evidence.
4. **One documented command (or checked-in script) reproduces the comparison
   from the checked-in spec and seed bank.** `scripts/reproduce-increment6.sh`,
   run from a state with no run directory and no generated artifact: every
   artifact byte-identical (empty `git status`), 60 manifests verified, both
   goldens matched.
5. **The five gates pass.** `cargo test --workspace --all-features` 584 passed,
   0 failed, 1 ignored; clippy clean with `-D warnings`; `cargo fmt --all
   --check` clean; `dependency direction OK`; `tangle check` passed (90
   nodes) on the tree this Resolution lands in.

Outcome complete, with one defect found and repaired on the way: `run_batch`
recorded a requested fixed step and ran the kernel at the default one, so the
Fast and Fine fidelities were not refinements at all and `replay --verify`
failed on those runs. The fix and its regression test are in `aefc096`, the
evidence is regenerated, and `FBK-022` records the ownership gap the repair
raised. `TAS-035` and `TAS-041` were left read-only.

# Done when

- Golden traces exist and are checked in; any regeneration is deliberate and
  reported, and `replay --verify` passes on the same manifest.
- Convergence evidence for Fast/Standard/Fine exists as a machine-readable
  artifact plus a human-readable summary; material sensitivity is reported per
  metric rather than hidden.
- The known-limitations document names the unvalidated claims and the residual
  limitations carried from Increments 4–5.
- One documented command (or checked-in script) reproduces the comparison from
  the checked-in spec and seed bank.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `tangle check`.
