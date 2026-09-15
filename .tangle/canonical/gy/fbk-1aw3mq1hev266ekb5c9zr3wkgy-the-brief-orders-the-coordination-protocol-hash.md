---
context_rev: 1
status: proposed
updated: 2026-09-15T06:45:14Z
summary: The brief orders the coordination protocol hash -> claim -> implement, but the approved...
tangle_revision: 1.0.0+gdd7fddb
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: TAS-133 (reproduce every Increment 2 fixture and golden its transitions), fresh writer session in main: hash the node, claim it, implement two CLI suites plus a seed bank and twelve trace goldens, update the node, commit, release.
Friction: The brief orders the coordination protocol hash -> claim -> implement, but the approved reading (B) could only be validated by an empirical seed search over the fixtures at Standard and Fine before any deliverable existed; the immediate hash+claim would have leased the node blind for the whole search. The claim was therefore made after the implementation, so the recorded base hash covered a node the session had already decided how to change, and the base-hash convention ("the starting hash covers the handed-off node before its frontier status change or # Context edit") did not tell me what to do when reconnaissance must precede the claim. Second friction: the node bundles "including stable simultaneous-claim ordering" into # Done when, but that property is owned by TAS-127 (crates/hekate-sim/tests/claims.rs) and no checked-in fixture produces two simultaneous claims at any pinned seed, so the clause is satisfiable only by delegation and by asserting the negative; the skill gives no shape for a Done-when clause that another node owns.
Improvement: SKILL.md: state the rule for reconnaissance-before-claim explicitly, e.g. "when a brief requires measurement before the deliverables are known, hash the node, claim it for the reconnaissance, renew the lease after the measurement, and record the base hash of the pre-edit node the first claim saw" - and note that tangle hash after a node edit is a different digest from the claim base hash, so a later session must not read that difference as a mismatch. SKILL.md: add a short rule for Done-when clauses owned by another node, e.g. "a clause whose evidence lives in a sibling node is recorded as delegated in this node Result, naming the owner and the evidence, rather than met here"; that is what this session did by hand for the simultaneous-claim clause.
