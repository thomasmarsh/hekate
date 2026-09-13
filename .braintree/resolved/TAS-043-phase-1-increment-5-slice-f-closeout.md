---
context_rev: 1
priority: P1
updated: 2026-09-13T03:22:00Z
summary: Slice G of Phase 1 Increment 5 closed the independent slice-F review findings: the P1 comparison.json unpaired-ordering defect and its regression test, the DEF-004 summary.json version field, the reproducer for the perf within-pass split, the run command's help contract, the atomic completion marker, and the accepted F6 limitation.
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
moves are its own `proposed/` -> `active/` -> `resolved/` status transitions. No
kernel change and
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

# Resolution

Resolved on the tree of commit `1ff0dd2` (the slice landing, after the five
gates passed), with the node's own status the only later change. Every finding
with its evidence:

- **F1 (P1) — `comparison.json`'s `unpaired` list is ascending again.**
  `PairedAccumulator::finish` sorts the list by seed
  (`apps/tangle-cli/src/compare.rs:1205`-`:1206`), and the doc comments on
  `fill_missing` (`:1180`) and `finish` (`:1200`) now state why insertion order
  is not seed order for a sparse bucket. Regression test
  `a_bucket_first_reached_at_a_later_seed_still_orders_its_unpaired_seeds`
  (`apps/tangle-cli/tests/compare.rs:817`): a two-seed pair whose bucket only
  side B carries, and only at seed 1, so seed 0 exists only through the fill.
  With the sort removed the test reports
  `[(1, NotObserved, Reported), (0, NotObserved, NotObserved)]`; with it, the
  seeds ascend.
- **F2 (P2) — DEF-004's summary requirement holds as written.**
  `RunSummary` carries `metric_definition_version`
  (`apps/tangle-cli/src/run_dir.rs:333`), set from
  `run_metrics::METRIC_DEFINITION_VERSION` (`:440`), so `summary.json` emits
  `metric_definition_version: 1` next to the run's numbers. The additive field
  closes the finding, so `.braintree/resolved/DEF-004-metric-definition-v1.md`
  was not edited and its `context_rev` is unchanged. Tests:
  `summary_reports_the_run_and_traces_back_to_the_manifest`
  (`apps/tangle-cli/tests/run_directory.rs:340`) and
  `metrics_json_carries_the_versioned_run_minima_and_slices`
  (`apps/tangle-cli/tests/run_metrics.rs:368`), whose
  `:389`-`:394` assertion ties `summary.json`'s revision to `metrics.json`'s.
- **F3 (P2) — the within-pass split is reproducible from the checked-in
tools.** `scripts/profile-symbols.py:119` folds the rows below the pass's frames
  by terminal symbol name and `:209` prints them as the `within-pass frames`
  section with each frame's share of the pass. Folded from the checked-in
  capture the section reads `candidate_pairs 1898 64.5%` and
  `time_to_collision 458 15.6%`, which are exactly the README's 1 898 of 2 944
  (64 %) and 458 (16 %); `perf/profiles/mixed_interaction_v1-release.summary.txt:38`
  is that fold and `perf/README.md:183` says so, including that terminal-name
  folding is what puts the bisection's inlined `first_fraction` closure (174)
  with the bisection (284). Re-running the script over the capture reproduces
  the artifact byte for byte.
- **F4 (P3) — `run` documents its contract.** `RUN_LONG_ABOUT`
  (`apps/tangle-cli/src/main.rs:141`) hangs off the `Run` variant
  (`:113`) with usage, inputs, output, and exit codes, and
  `help_documents_usage_inputs_outputs_and_exit_codes`
  (`apps/tangle-cli/tests/run_directory.rs:445`) pins the help text like
  `validate --help`'s test does.
- **F5 (P3) — a truncated marker no longer fails the batch closed.** The
  completion marker is staged in `MANIFEST_TEMP_FILE` (`run_dir.rs:75`) and
  renamed into place (`write_manifest`, `:520`-`:529`, called at `:405`), so
  `manifest.json` is never truncated, and `batch` counts a leftover staging file
  as a run artifact (`apps/tangle-cli/src/batch.rs:314`, `:334`) so an
  interrupted marker write reads as a partial run to clear and re-run.
  Regression test `an_interrupted_marker_write_is_completed_rather_than_refused`
  (`apps/tangle-cli/tests/batch.rs:316`) stages the crash and asserts the re-run
  reproduces the completed bytes with no staging file left; without the rule it
  fails with `BatchError::ForeignContent`.
- **F6 (P3) — accepted, recorded above.**

Every `# Done when` bullet, with its evidence:

1. **F1 fixed with a falsifiable regression test.** — `compare.rs:1205`, test
   `a_bucket_first_reached_at_a_later_seed_still_orders_its_unpaired_seeds`.
2. **F2 closed by the additive `summary.json` field.** — `run_dir.rs:333`,
   `run_directory.rs:340`, `run_metrics.rs:368`.
3. **F3's numbers reproducible from the checked-in script/artifact.** —
   `scripts/profile-symbols.py`, `perf/profiles/mixed_interaction_v1-release.summary.txt:38`,
   `perf/README.md:183`.
4. **F4 and F5 fixed with tests.** — `main.rs:141` with
   `run_directory.rs:445`; `run_dir.rs:520` and `batch.rs:314` with
   `batch.rs:316`.
5. **F6 recorded as an accepted limitation.** — the paragraph above, with the
   unread aggregation inputs named.
6. **Canonical trace, goldens, and Phase 1 baseline unchanged; `EVENT_VERSION`
   stays 2.** — the slice's commits touch no path under `tests/golden/`,
   `baselines/`, `scenarios/`, `schemas/`, or `crates/`; `crates/tangle-sim` and
   `Cargo.toml`/`Cargo.lock` are unchanged and `EVENT_VERSION` is still 2. The
   trace and its hash goldens are byte-identical (`run_directory.rs`'s golden
   round-trip tests pass).
7. **The five gates pass on the final tree.** — `cargo test --workspace
   --all-features` (550 passed, 0 failed), `cargo clippy --workspace
   --all-targets --all-features -- -D warnings` (clean),
   `cargo fmt --all --check` (clean),
   `./scripts/check-dependency-direction.sh` (`dependency direction OK`), and
   `braintree check` (75 nodes) recorded in `# Result`.
