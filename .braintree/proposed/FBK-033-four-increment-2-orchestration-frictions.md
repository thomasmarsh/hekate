---
context_rev: 1
updated: 2026-09-14T15:02:18Z
summary: Four Increment 2 orchestration frictions: loop cost, recovery scope, git mv, sizing.
braintree_revision: 0.6.0+gef4b49d
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Ran Phase 2 Increment 2 TAS-020 as a serial fresh-worker orchestration (TAS-113 return crossing; TAS-114 destination leader/follower constraints), with a timed-out-worker recovery per leaf.
Friction: Four frictions. (1) Worker loop cost: briefs required cargo test --workspace as per-leaf acceptance; one touched test binary is 34-37s (tick-loop integration tests) and the full workspace gate is 148s warm over 83 test binaries, so three of three dispatched workers hit the 30-minute harness timeout. (2) Timed-out-recovery write set: TAS-113 finisher brief said "remove scratch only", but cargo clippy -D warnings and fmt were red on retained-slice lines outside the scratch test, so the finisher had to make clippy/fmt repairs the brief did not clearly authorize. (3) git mv stages the pre-edit blob, while the brief said do not git add; the staged rename carried a stale blob that no longer matched the written # Result. (4) Slice sizing: TAS-113 (cross-facility return crossing) and TAS-114 (destination leader/follower constraints) were each multi-hour outcomes dispatched as a single leaf, so each exceeded a session before completion; the AGENTS.md size-slices-up-front rule was applied only after the timeout.
Improvement: For SKILL.md/handoff guidance: (a) make per-leaf acceptance the touched test binary only and reserve cargo test --workspace for the coordinator gate, and note that cargo-nextest plus a pruned target/ (25 GB) would cut the loop; (b) a timed-out-recovery brief must state explicitly whether the finisher may repair clippy/fmt failures in the retained partial slice; (c) permit `git add` of the destination after `git mv`, or state that the integrator stages the renamed blob, since the rename otherwise cannot carry the # Result edit; (d) split a multi-hour increment leaf before dispatch (state the deliverable and keep each slice to one session) instead of detecting the overrun at the worker timeout.
