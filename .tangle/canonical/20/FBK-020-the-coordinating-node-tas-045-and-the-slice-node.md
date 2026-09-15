---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: The Increment 6 handoff asked a worker to reuse an existing Increment 5 experiment-spec format, but no such checked-in artifact or format exists in the repo or in any node, so the worker had to choose the spec's artifact and shape alone.
tangle_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Working TAS-047 from the coordinator's handoff, which said: 'A checked-in experiment spec and a common-random-number seed bank naming both variants, the fidelity/step, the seeds, and the sampling policy. Reuse the existing experiment-spec and seed-bank formats from Increment 5 rather than inventing a new one.'
Friction: No checked-in Increment 5 'experiment spec' format exists to reuse. The seed bank is concrete (`seed_bank_version` 1, `apps/hekate-cli/src/seed_bank.rs`), but the only Increment 5 specification surface is the `BatchSpec` struct recorded inside each per-batch `batch.json` (`scenario`, `ticks`, `fidelity`, `step_s`, `event_version`, `model_version`, `build_revision`, `sampling`), which is a recorded output of `batch`, not a checked-in input artifact. Nothing in `PHASE_1_PLAN.md` Increment 5, TAS-031, TAS-035..TAS-043, or the resolved node bodies defines what file an 'experiment spec' is, where it lives, or which fields it carries, so the handoff's instruction could not be followed literally and the worker had to choose a shape that a downstream slice (TAS-048/TAS-049) will consume. The coordinating node TAS-045 and the slice node TAS-047 both list 'a checked-in experiment spec, seed bank, results, and concise comparison report' as an outcome without naming the spec's artifact or format, so each slice in the increment can only infer it from the neighbouring slice's prose.
Improvement: Name the spec artifact in the plan text and in the slice node: give it a versioned file name and location (for example `experiments/<id>/experiment.json` under a declared `experiments/` root), and list its fields, reusing the Increment 5 `BatchSpec` field names (`fidelity`, `step_s`, `ticks`, `sampling`) plus the scenario and seed-bank paths. If the intended reading is instead 'the batch specification is the experiment spec, authored by hand instead of recorded by `batch`', say so, because the two readings imply different artifacts and different consumers. Also state the extension point: a new top-level input directory is a repo-structure decision the worker otherwise makes alone.
