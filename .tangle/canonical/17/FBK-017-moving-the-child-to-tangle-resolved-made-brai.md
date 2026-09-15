---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Moving the child to .tangle/resolved/ made "tangle check" fail with "error: .tangle/act
tangle_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Resolved the frontier child TAS-043 while its coordinating parent TAS-031 still had "next: TAS-043", with a write set that excluded the parent (the task said do not touch TAS-031 or the index map).
Friction: Moving the child to .tangle/resolved/ made "tangle check" fail with exit 1 at 75 nodes: "error: .tangle/active/TAS-031-....md: next frontier ... is already resolved", so the resolution could not be committed with the repo gate green. The coordination reference covers this: the coordinator owns a parent next advance when the worker write set excludes the parent, and the worker "reports the stale route ... as a handoff action". But it does not say what the worker commits in the meantime, and AGENTS.md calls a failing "tangle check" a commit blocker, so the worker is left choosing between an unresolvable red gate, an unauthorized parent edit, and a node that stays active after its Done when is met. The check also fires on the pre-commit worktree, so the failure appears the moment the file is moved, before any commit exists (verified with the kind of git mv staging sequence the coordination help documents: git add the source, git mv, git add the destination, then tangle check).
Improvement: State the worker-side completion rule for this case in SKILL.md/references/coordination.md: the frontier-child worker commits the node in resolved/ and the coordinator advances the parent in the same handoff, and until that advance lands the expected state is a known, named graph-check failure that the coordinator resolves rather than a blocker; or add an explicit option such as "tangle resolve --defer-parent" that either refuses before the move, or records the pending parent advance, so a worker never sits between a failing gate and an out-of-write-set edit. Also make the checker distinguish "child resolved but parent next not yet advanced" from a genuine stale route, since the first is a normal transient of a multi-writer handoff.
