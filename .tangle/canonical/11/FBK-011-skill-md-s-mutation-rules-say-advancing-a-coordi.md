---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: SKILL.md's mutation rules say "Advancing a coordinating parent's next after its frontier child i
tangle_revision: 0.5.0+g974b178
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Executed slice B1 of TAS-033 under an exclusive write set that named only apps/hekate-cli/**, apps/hekate-cli/Cargo.toml, Cargo.lock, and the assigned node, and that explicitly said "Do NOT touch TAS-031". The slice met every Done when bullet, so I resolved my own node in a second commit per the handoff.
Friction: SKILL.md's mutation rules say "Advancing a coordinating parent's next after its frontier child is resolved is part of that resolution rather than bookkeeping, so the resolving worker owns that edit", but the handoff's write set forbade touching the parent, TAS-031. After the resolution commit, TAS-031's next still names [[TAS-033-phase-1-increment-5-run-directory-core]], a resolved node. tangle check nodes still passed, so the checker does not flag it, and the skill offers no rule for who advances next when the resolving worker cannot write the parent. I followed the handoff (do not touch TAS-031) and reported the stale pointer to the coordinator rather than editing outside my set.
Improvement: State in SKILL.md how the parent-next advance is owned when the resolving worker's write set excludes the parent: either the handoff must name the parent's next (or the parent file) in the write set so the resolving worker owns the edit, or the skill must say the coordinator owns that advance and that a worker reports the stale pointer as its handoff action. A checker rule would also help: flag an unfinished coordinating node whose next names a node that is already resolved.
