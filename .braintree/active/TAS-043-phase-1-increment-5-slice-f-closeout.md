---
context_rev: 1
priority: P1
updated: 2026-09-13T03:19:00Z
summary: Slice G of Phase 1 Increment 5 closes the independent slice-F review findings: the P1 comparison.json unpaired-ordering defect, the DEF-004 summary.json version requirement, the unbacked perf within-pass split, and the two P3 notes.
next: Resolve TAS-043 once the five gates pass on the final tree.
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

Claimed at `content_hash 6eda867b791d2283a5ee50b000dba72d2943d8116f11c5f6ff4df3aa3a85139c`
(the bare digest `braintree hash TAS-043` printed) with
`braintree claim TAS-043 worker --base-hash <hash> --lease-seconds 14400`, and
released with that same base hash before handoff.

Write set as handed: `apps/tangle-cli/src/{batch,compare,lib,main,run_dir}.rs`,
`apps/tangle-cli/tests/{batch,compare,run_directory,run_metrics}.rs`,
`scripts/profile-symbols.py`,
`perf/profiles/mixed_interaction_v1-release.summary.txt`, `perf/README.md`, and
this node. No created path and no path outside the write set; the node's only
move is its own `proposed/` -> `active/` status transition. No kernel change and
no dependency change: `crates/` is untouched and `Cargo.toml`/`Cargo.lock` are
unchanged. `DEF-004` was not edited — F2 took the additive `summary.json` field,
so the settled definition holds as written and its follow-up needed no
re-scoping.

Disposition: F1-F5 closed, F6 recorded as an accepted limitation. Per-finding
evidence (file:line and test name) is in `# Resolution`.

Preservation: the change touches no golden, baseline, scenario, or schema —
`git status --short` lists only the write set above — and `EVENT_VERSION` stays
2. `comparison.json` for a bucket present at seed 0 is unchanged (the sort is a
no-op on an already ascending list), and `summary.json` grows only by the added
field.

Gates on the final tree:

- `cargo test --workspace --all-features` — passed, no failure in any target.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed, no warning.
- `cargo fmt --all --check` — passed.
- `./scripts/check-dependency-direction.sh` — passed (`dependency direction OK`).
- `braintree check` — passed (75 nodes).

Both regression tests were shown to fail against the pre-fix behaviour before
being kept: F1's test reported
`[(1, NotObserved, Reported), (0, NotObserved, NotObserved)]` with the sort
removed, and F5's test reported `BatchError::ForeignContent` for the seed
directory with the staging-file rule removed.

F6 accepted limitation: `metrics.json` records event counts by mode and by
movement (`EventCounts::by_family_mode`, `by_family_movement`) and the
aggregation reads only `total`, `by_family`, and `by_family_kind`
(`apps/tangle-cli/src/aggregate.rs:655`-`:679`), so mode- and
movement-scoped event-family counts are not aggregated across seeds. Closing it
is a metric-surface change beyond this review closeout.
