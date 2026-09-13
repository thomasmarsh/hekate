---
context_rev: 1
updated: 2026-09-13T14:26:32Z
summary: The node is a frontier node whose # Done when spans several sessions, but SKILL.md has no rule t
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Implemented TAS-018 (Phase 2 Increment 0), whose # Blocked section named a single unblock action (re-check the Phase 1 entry gate) while its # Done when listed schema version 2 with a deterministic migrate command and provenance, compiled agent components with mode templates and a model-card template, a benchmark matrix, and arbitrary-body-kind representation.
Friction: The node is a frontier node whose # Done when spans several sessions, but SKILL.md has no rule telling a worker to size the session to a coherent slice. Read together, the rules push either toward stopping with no progress (the blocker was the whole Phase 1 program) or toward silently attempting every deliverable; this session initially drifted toward the latter by treating full Increment 0 as the target until the user narrowed it to the gate-only unblock. SKILL.md says one node may span sessions but never says a session must stop at a coherent slice, and the single-next form leaves nowhere obvious to record remaining sub-deliverables except the body. A second ambiguity: SKILL.md says never bump context_rev for status moves, but clearing a blocker flips decision-relevant context (whether downstream increments may start), so it is unclear whether a blocked-to-proposed move is a status move or a semantic change.
Improvement: Add a SKILL.md rule in the status/next section: when a frontier node has # Done when criteria that cannot be met in one session, the worker must deliver the smallest coherent slice (clearing the blocker, or completing one deliverable), record the exact remaining scope and evidence in the body, and leave the node proposed or active with a next naming the first remaining action; unblocking a node is not completing it. Also state explicitly whether a blocked node whose blocker resolves is a status move that must not bump context_rev, or a semantic change that must (this session bumped it to 3 and recorded that reasoning).
