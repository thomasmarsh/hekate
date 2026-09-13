---
context_rev: 1
updated: 2026-09-13T18:22:06Z
summary: Five coordination frictions from the Phase 2 Increment 0 session: the check/next resolution ordering, timed-out worker recovery, write-set closure for a lockfile, coordinator timestamp clamp, and source-text no-branch proofs.
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Coordinated Phase 2 Increment 0 (TAS-018) by dispatching one fresh worker per leaf: children wrote their own Result and moved their node to resolved, and the coordinator advanced each parent next and committed (Closes TAS-XXX, Refs TAS-018).
Friction: Repeated tooling surprise: braintree check requires next on a proposed task but forbids it on a resolved one, and resolving a frontier leaf necessarily leaves the direct parent next naming a resolved child. A worker therefore cannot both keep its node proposed for the mandated pre-move check and move it to resolved without a transient placeholder next, and the worktree is transiently next-resolved-node failing after every leaf until the coordinator edits the parent. Observed at every leaf handoff: error: .braintree/proposed/TAS-0NN-....md: next frontier (leaf node) is already resolved.
Improvement: State the resolution ordering explicitly in SKILL.md: a resolving worker keeps a placeholder action next only long enough to run the pre-move check, then removes next as the final edit before the status move; and state that the coordinator, not the worker, owns the parent advance, so the transient next-resolved-node diagnostic is expected at handoff and is not a failure. Alternatively make a next naming an already-resolved direct child a non-fatal warning until the coordinator integrates.

## Finding 2 — recovery from a timed-out worker

Attempted: dispatched a single fresh worker for TAS-068 (four controller-stage interfaces). It exceeded the 30-minute run window after implementing stage.rs and routing sim.rs through the four stages, but before adding stage tests or resolving the node.

Friction: the coordination reference says to capture the partial diff, re-scope, and report when a run goes long, but gives no procedure for deciding between resuming, reverting, and re-dispatching a worker that left uncommitted partial work. Observed: the worktree held a compiling, behavior-preserving refactor with all existing tests passing, yet the node was still proposed and the acceptance tests were missing.

Improvement: add a short recovery subsection to the coordination reference: on timeout, inspect the partial diff and run the touched crate tests to establish whether the partial state is behavior-preserving; when it compiles and passes, re-dispatch a narrow finishing brief for the remaining slice (tests plus node bookkeeping) on the same node instead of reverting; when it does not, revert and re-scope. State that the node stays proposed until the finishing worker resolves it.

## Finding 3 — write-set closure when a dependency is added

Attempted: TAS-071 added a `glam` dependency to `apps/tangle-cli` to carry body-segment poses into the trajectory artifact.

Friction: adding a crate dependency requires editing the workspace `Cargo.lock` at the repository root, which is outside the declared write set (`apps/tangle-cli/**`), so a worker must either exceed its write set or stop.

Improvement: the coordination reference's compile-and-golden write-set closure should name the workspace manifest and lockfile as in scope when the approved change needs a dependency, or the handoff should include them explicitly.

## Finding 4 — coordinator timestamp ahead of the host clock

Attempted: as coordinator, stamped a parent node `updated` ahead of the host clock on an early handoff.

Friction: the clamp rule (a worker refreshing an inherited `updated` uses `max(now, previous updated)`) is correct, but it then preserves that future timestamp for every later edit in the session and looks stale against the children's real times.

Improvement: note in SKILL.md that a coordinator should stamp the host clock rather than a rounded or estimated time, so it does not create a clamp it must carry for the rest of the session.

## Finding 5 — proving the absence of a named-mode branch

Attempted: TAS-070's synthetic-template gate and TAS-072's presenter gate both needed to prove the shared code gained no branch on a mode or scenario name.

Friction: the skill has no guidance for proving the absence of a pattern; both leaves used `include_str!` source-text guards, which are defeatable by string construction and require care to avoid matching legitimate strings.

Improvement: add an authoring note on negative assertions: a checked-in source-text guard is acceptable when paired with a falsification probe, and should name the exact tokens and the modules it covers.
