---
context_rev: 1
updated: 2026-09-13T22:10:23Z
summary: Five coordinator frictions from the Phase 2 Increment 1 session: a leaf write set omitted its generated-schema closure, two workers timed out, one session produced three FBK nodes, a matrix tolerance named a form the authored schema cannot express, and repeated next-resolved-node transients.
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Coordinated Phase 2 Increment 1 (TAS-019) as an orchestration-only session: groomed seven leaves, dispatched one fresh worker per leaf, verified and committed each, and resolved TAS-019.
Friction: Five coordinator frictions: a leaf write set omitted its generated-schema closure; two workers hit the 30-minute window; subagents each created an FBK so one session produced three feedback nodes; a benchmark tolerance named a fixture form the authored schema cannot express; and every leaf handoff transiently failed braintree check with next-resolved-node.
Improvement: State the compile-and-golden closure (including generated schemas) in every leaf brief; add the timed-out-worker recovery procedure to the coordination reference; have the coordinator own the single session FBK and tell workers not to create one; require a matrix tolerance to name the schema construct that expresses its reference; and make a next naming a resolved child a warning until the coordinator advances it.

## Finding 1 — a leaf write set omitted its generated-schema closure

Attempted: dispatched TAS-075 with a write set of `crates/tangle-model/**` and `scenarios/**`, because the node's stated write set was `validate.rs` and profile parameters.

Friction: the brief itself required two reconciliations that necessarily regenerate
`schemas/scenario-source.schema.json` (making `speed_policy.limit_mps` required
and adding `ModeBodySource::Capsule`), and the worker correctly stopped and asked
before editing outside its set. The generated schema is part of the compile
closure of any source-shape change, so it should never have been omitted.

Improvement: the coordination reference already says the write set is the
compile-and-golden closure; add generated artifacts (JSON schemas, snapshots,
and pinned-hash fixtures) to the enumeration, and have the coordinator spell out
"the compile closure of the approved change, including generated schemas" in every
leaf brief.

## Finding 2 — two workers hit the 30-minute window

Attempted: dispatched one fresh worker each for TAS-077 (spawning/model/cards plus
longitudinal tactics) and TAS-078 (six fixtures plus tolerance tests). Both ran the
full 30-minute window; TAS-077 finished a coherent slice and correctly split the
remainder to TAS-080, and TAS-078 finished its work but timed out just before
writing its `# Result`.

Friction: recovery was entirely manual — inspect the partial diff, run the touched
crate's tests to establish it is behavior-preserving, then either accept and commit
the slice or revert. This is the second session to record it (see `FBK-026`
Finding 2), and the skill still states the principle without the procedure.

Improvement: add the recovery subsection to the coordination reference exactly as
`FBK-026` proposed: on timeout, inspect the partial diff and run the touched crate
tests; if green, re-dispatch a narrow finishing brief (or accept the coherent slice);
if not, revert and re-scope. Also note that a worker that timed out having already
moved its node to `resolved/` and created its split child needs only coordinator
verification, not a re-dispatch.

## Finding 3 — one session produced three FBK nodes

Attempted: coordinated the session and recorded the mandated session FBK at
close-out.

Friction: two workers independently created FBK nodes (`FBK-027`, `FBK-028`) before
the coordinator recorded `FBK-029`, so a single orchestration session now has three
feedback nodes, against `AGENTS.md`'s "one `FBK` node per session". The workers were
not told whether they owned friction recording.

Improvement: state in the worker handoff template that the coordinator owns the
single session `FBK`, and that a worker reports friction in its run report instead
of creating a node unless the coordinator explicitly grants it. Alternatively,
permit worker FBKs but have `AGENTS.md` say the one-per-session rule applies to the
orchestration session's own node and point at worker-recorded nodes as evidence.

## Finding 4 — a matrix tolerance named a form the schema cannot express

Attempted: dispatched TAS-078 to satisfy `T-RT <= 1e-9 m` on
`narrow_isolated_curve_v2`.

Friction: authored `paths[].points` compile only to polylines, which cannot hold
a 1e-9 round trip at lateral offsets near a vertex, so the fixture had to declare
an analytic reference and the test reconstruct it — a supervisor decision. This is
captured in full in `FBK-028` Finding 3 and is cross-referenced here because it was
the only architecture fork of the session.

Improvement: as in `FBK-028`: require every analytic-reference tolerance in the
benchmark matrix to name the schema construct that expresses the reference.

## Finding 5 — repeated next-resolved-node transients

Attempted: verified and committed each leaf.

Friction: every leaf handoff left `.braintree/proposed/TAS-019-…` naming a
just-resolved child, so `braintree check` failed until the coordinator advanced the
parent. The skill documents the coordinator's ownership of that advance but still
treats the transient as a hard error. Recurrence of `FBK-026` Finding 1.

Improvement: as in `FBK-026`: make a `next` naming an already-resolved direct
child a non-fatal warning until the coordinator integrates, or state the
expected transient explicitly in the resolving-worker procedure.

## Finding 6 — the write set omitted the golden closure (recurrence of FBK-009)

Attempted: dispatched TAS-081 with a write set of `crates/tangle-present/**`,
`apps/tangle-tui/**`, `apps/tangle-viewer/**`, and one doc.

Friction: the required `SCENE_FORMAT_VERSION` bump invalidates the checked-in
scene golden `tests/golden/present/walking_guide_v1.seed0.tick20.scene.txt`,
which lives outside every directory in the assigned write set, so the worker had
to include and report a path outside its set. This is exactly the compile-and-
golden closure gap `FBK-009` already recorded: the assigned write set is the
closure of the change, but `tests/golden/**` is easy to forget because it is not
under the crate or app that owns the format.

Improvement: name `tests/golden/**` and `baselines/**` explicitly in every brief
whose change can invalidate a golden, and have the coordinator state the golden
closure alongside the source closure rather than expecting the worker to infer
it from "any format change is versioned and covered by a golden test".
