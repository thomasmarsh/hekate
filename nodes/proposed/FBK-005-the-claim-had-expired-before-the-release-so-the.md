---
context_rev: 1
updated: 2026-09-12T17:43:51Z
summary: The claim had expired before the release, so the release the skill requires before handoff repor
braintree_revision: 0.5.0+gd1a4b82
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Claimed TAS-028 with `braintree claim TAS-028 worker --base-hash 53a1aea1758270ae84f665e6a807488c3bfe07785ae7ae61f9bfcbd1849caf6a` (result: claimed, lease_expires_at: 1789234767), edited the node and code for about an hour, then ran the mandated `braintree release TAS-028 worker --base-hash 53a1aea1758270ae84f665e6a807488c3bfe07785ae7ae61f9bfcbd1849caf6a`. Observed output: `result: "no-op"`, `node: "TAS-028"`; `braintree status` then showed `active_claims: "0"`.
Friction: The claim had expired before the release, so the release the skill requires before handoff reported no-op instead of releasing anything. SKILL.md documents that no-op means no unexpired lease, but it never states the default lease duration, and nothing warns a long single-writer slice that its claim has lapsed: a second worker could have claimed TAS-028 mid-slice and produced a divergent write of the same node, which is exactly the hazard the claim exists to prevent. The undocumented default also makes --lease-seconds impossible to choose knowingly.
Improvement: State the default lease duration and the renew-on-reclaim rule in SKILL.md, and have claim/release print the remaining lease time on every call. Consider making a long-running claim extend automatically on a successful claim with the same agent and base hash, or returning a distinct `expired` result instead of `no-op` so a worker can tell a lapsed lease from a node it never held. Also worth documenting: pass --lease-seconds sized to the slice, or re-claim with the original base hash before handoff when a slice runs long.
