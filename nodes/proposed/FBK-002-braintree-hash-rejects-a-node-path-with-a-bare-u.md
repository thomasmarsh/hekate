---
context_rev: 1
updated: 2026-09-12T16:16:45Z
summary: braintree hash rejects a node path with a bare "unknown node" error
braintree_revision: 0.5.0+gc99a080
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Hash the assigned node before claiming it, following the task protocol "hash the node, braintree claim it", and run braintree claim with the recorded base hash.
Friction: braintree hash nodes/active/TAS-027-...md failed with "unknown node: nodes/active/TAS-027-...md". The skill and the task both say "hash the node" and the skill command list shows "hash NODE", but neither states the accepted addressing form. A filesystem path is the natural reading of "NODE" for a worker that was handed a direct node path, and the error gives no hint that a bare ID or filename stem is required.
Improvement: State the accepted node addressing in SKILL.md next to braintree hash (bare ID or filename stem, never a path), and make the "unknown node" error suggest the accepted forms when the argument looks like a path (contains a slash or a .md suffix).

Attempted: Follow the coordination protocol that orders the frontier transition (edit # Context, move the node to nodes/active/) before hashing and claiming, then release with "the same base hash".
Friction: The skill's parallel-worktree contract says a worker hashes the starting node and claims it before editing, so the base hash is the pre-edit content. When the frontier move is itself an edit to the assigned node and is ordered before the claim, the "base hash" already includes that edit, so it is not a pre-edit hash and the claim's content-hash guarantee covers a state a reader never saw as proposed. Nothing in the skill resolves whether the frontier transition is part of the claimed edit or precedes it.
Improvement: In SKILL.md, say whether a status move plus # Context edit that takes the frontier belongs to the claimed edit (claim first, hash the proposed state, then move) or to pre-claim setup (move first, then claim the active state), and make "record the base hash" name which of the two contents it means.
