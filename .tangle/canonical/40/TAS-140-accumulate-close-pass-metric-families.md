---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Accumulate close-pass metric families in RunMetrics
---

Parent [[TAS-122-close-the-close-pass-observation-and-report-clea]].

# Outcome

Closed `ClosePass` observations accumulate into `RunMetrics` as attempts,
aborts, completions, violations, and clearance-band distributions, by mode pair,
movement, facility, and applicability.

# Done when

- `apps/hekate-cli/src/run_metrics.rs (RunMetricsRecorder)` accumulates the
  close-pass families from the closed observations with explicit applicability.
- Nominal travel is not counted, and an inapplicable mode reports no close-pass
  family.
- Focused tests cover the accumulation and inapplicable modes.

# Context

Extends [[TAS-122-close-the-close-pass-observation-and-report-clea]] and depends
on the sibling metric-definition-v3 node for the family names. Owns the
accumulation only; the run-artifact and aggregate/compare surface is the next
sibling slice.

# Result

The accumulation landed in `apps/hekate-cli/src/run_metrics.rs`. The recorder now
counts the four overtaking families of
[[DEF-006-metric-definition-v3]] from the edge-triggered `Event::Maneuver`
records whose `kind` is `TacticKind::Overtake` — attempt (`following ->
preparing`), commit (`preparing -> committed`), completion (`committed ->
returning`), and abort, with the `returning -> following` edge completing the
return rather than the pass — and reads the close-pass families off the closed
observations `Simulation::close_pass_tracker().overtakes()` holds: the
`close_passes` count, `close_pass_minimum_clearance_m` carrying the observation's
`min_clearance_time_s` and `relative_speed_mps` at that minimum, one duration
series per declared band keyed by its stable `ClearanceBandId`, and
`close_pass_violations`. `RunMetrics` carries them as `close_pass:
ClosePassMetrics` over the run and per `ModePair` label, pairwise movement key,
and facility (`facility:<name>`).

Applicability is explicit and never a false `0`. A mode pair whose mode class
declares neither the `pass` nor the `overtake` tactic, and a facility whose
compiled `lateral_use` is `centered` (nominal travel, no lateral freedom), are
`not_applicable` for the clearance minimum and for every band duration; a bucket
that could host a pass and recorded none is `not_observed`; a bucket with
observations reports the value, including a true `0.0` band duration. The
countable families stay counts, as DEF-006 fixes for them: a none-observed or
inapplicable bucket reports `0` rather than an absent value.

The run-directory artifact, aggregate, and compare are untouched: `RunMetrics` is
the in-memory capture and `RunMetricsArtifact::new` does not yet serialize the
block. That surface is [[TAS-141-surface-close-pass-metric-families]], and the
same node must re-export these new types from `apps/hekate-cli/src/lib.rs` with
it.

Evidence: `cargo test -p hekate-cli --test run_metrics` (9 passed, including
`close_pass_families_accumulate_from_the_closed_observations`,
`nominal_travel_is_not_counted`,
`an_inapplicable_mode_reports_no_clearance_value`, and
`a_centered_facility_reports_no_clearance_value`), `cargo test -p hekate-cli`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo fmt --all --check`, `scripts/check-dependency-direction.sh`.

Observed limitation, outside this slice: the CLI run loop never calls
`Simulation::close_open_close_passes`, so an interval still alongside at the run's
final tick contributes no observation to the slice; that belongs to the
observation lifecycle the enclosing node owns.
