---
context_rev: 1
priority: P1
updated: 2026-09-13T03:05:35Z
summary: Slice G of Phase 1 Increment 5 closes the independent slice-F review findings: the P1 comparison.json unpaired-ordering defect, the DEF-004 summary.json version requirement, the unbacked perf within-pass split, and the two P3 notes.
next: Fix the P1 `unpaired` ordering in compare.rs with a regression test, then close G2-G5 and rerun the five gates.
---

# Outcome

Close the findings of the independent read-only slice-F verification. The
reviewer returned "no merge blocker" and raised one P1 defect, two P2 items, and
three P3 notes:

- **F1 (P1) — `comparison.json` `unpaired` is not ascending for sparse movement
  slices.** `PairedAccumulator::finish` never sorts the seed list that
  `fill_missing` appends to, so when a bucket first appears at seed index `k`
  with one side missing, the result is `[k, 0, 1, …]`, contradicting the field's
  documented ascending contract. Fix it and add the missing regression test
  (a bucket carried by one side only at seed 1, absent at seed 0, asserting the
  unpaired seeds ascend) — the existing test uses a bucket present at seed 0, so
  it never exercises the defect.
- **F2 (P2) — DEF-004's `summary.json` requirement is unimplemented and
  uncorrected.** DEF-004's "# Where the version appears" requires `summary.json`
  to emit `metric_definition_version`; `RunSummary` has no such field. Add the
  additive field (preferred, so the settled definition holds as written) or
  amend DEF-004's follow-up to record the re-scoping; if you amend the resolved
  definition, record the correction and leave its `context_rev` unless a
  consumer assumption changes.
- **F3 (P2) — the perf within-pass split has no artifact backing.**
  `perf/README.md` states candidate-query and TTC-bisection shares that
  `scripts/profile-symbols.py` does not emit and `perf/profiles/*.summary.txt`
  does not contain. Extend the fold/script to emit the within-pass section (or
  add the summation to the artifact) so a reader can reproduce the numbers.
- **F4 (P3) — `run` lacks the `long_about` contract** its sibling commands have.
  Add it, with a help-content test like `validate --help`'s.
- **F5 (P3) — a truncated `manifest.json` fails the batch closed.** A crash
  during the marker write leaves a seed directory `batch` refuses instead of
  clearing and re-running, unlike the missing-marker partial path. Make the
  marker write atomic (temp + rename) or treat an unparseable marker as partial;
  add a test.
- **F6 (P3) — mode/movement event-family counts are not aggregated.** Accepted:
  record it as a limitation rather than expanding scope in this slice.

Constraint: no behavior change beyond these fixes; the canonical trace, trace
golden, trace hash golden, Phase 1 baseline, and all goldens stay byte-identical;
`EVENT_VERSION` stays 2; no dependency change in `tangle-model` or `tangle-sim`
(no kernel change is expected in this slice).

If the slice outgrows one session, land F1 with its regression test first,
commit with `Refs TAS-043`, leave the node `active` with a concrete `next`, and
report.

# Done when

- F1 is fixed with a regression test that fails against the old behaviour.
- F2 is closed by the additive `summary.json` field (or a recorded DEF-004
  amendment).
- F3's numbers are reproducible from the checked-in script/artifact.
- F4 and F5 are fixed with tests.
- F6 is recorded as an accepted limitation.
- The canonical trace, goldens, and Phase 1 baseline are unchanged;
  `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

The slice-F reviewer (`2ac861c5`) verified all eleven `Done when` criteria for
the increment and reported "no merge blocker" with the findings above. This
child closes them before the coordinating node is resolved. The coordinator has
already independently confirmed the diff-level preservation claims (only
`crates/tangle-sim/src/metrics.rs` changed in `crates/`, +91 additive lines;
goldens, baselines, scenarios, and schemas untouched; `.braintree/` tracked).

# Result

Pending.
