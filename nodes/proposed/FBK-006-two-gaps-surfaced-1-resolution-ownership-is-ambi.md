---
context_rev: 1
updated: 2026-09-12T18:49:06Z
summary: Two gaps surfaced. (1) Resolution ownership is ambiguous: SKILL.md says "the coordinator alone r
braintree_revision: 0.5.0+gd1a4b82
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Orchestrate Phase 1 Increment 3 through Braintree: author coordinating node TAS-028, run slices A-G as fresh serial sub-agents, integrate each, and resolve the parent from slice evidence.
Friction: Two gaps surfaced. (1) Resolution ownership is ambiguous: SKILL.md says "the coordinator alone resolves a coordinating parent after all required child work is integrated", while the harness slice plan had a worker "resolution slice" close findings and resolve the node. When the parent is the coordinating node itself, it is unclear whether the resolving edit is a coordinator action (allowed: next/status) or must be delegated to a fresh writer. (2) The read-only reviewer agent has no shell, so it cannot run the five gates it is asked to independently verify; its sign-off was necessarily conditional on the orchestrator rerunning cargo/clippy/fmt/dependency-direction/braintree check, and it explicitly could not inspect commit diffs or bodies. "Independent verification" then depends on a non-read-only actor for the strongest evidence.
Improvement: State in SKILL.md that a coordinating parent is resolved by the coordinator as a graph action (status move + evidence/limitations + next removal) once required children are integrated, and that a worker slice may only prepare closeout evidence. For verification, specify that a read-only verifier must be paired with a coordinator-run gate transcript, or give the verifier an execution-limited tool profile for the five gates, since a no-shell reviewer cannot falsify recorded gate claims.
