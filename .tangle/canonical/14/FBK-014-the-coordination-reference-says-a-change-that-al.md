---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: The coordination reference says a change altering a landed seam another node owns is escalated rather than authored, but gives no rule when the assigned node's own Done-when requires an additive field on a resolved sibling's seam.
tangle_revision: 0.5.0+g7b95875
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Slice C2a of TAS-039 (a seed bank plus `batch --seed-bank`) required recording the bank path and content hash in the already-landed `BatchManifest`, which the resolved sibling node TAS-038 (aggregation) owns and deserializes.
Friction: The coordination reference says "A change that alters a landed seam another node owns, or the public schema contract, is escalated rather than authored", but the assigned node's own Done-when explicitly mandated adding that field, and the skill gives no rule for the bookkeeping when a change is authorized by the consuming node yet lands on a resolved sibling's seam: whether to escalate anyway, to bump the resolved sibling's context_rev, or to note it only in the new node's Result. A one-line additive `seed_bank: None` also had to be added to the sibling test file (apps/hekate-cli/tests/aggregate.rs) to keep a struct literal compiling, which reads as "touching" a node the worker was told not to own.
Improvement: Add a short paragraph to the coordination reference for an authorized additive change to a seam owned by a resolved node: state whether escalation is still required, whether the resolved owner's context_rev is bumped (and by whom), and confirm that mechanical literal updates in the owner's tests stay inside the consumer's write set. Candidate text: "An additive, optional field on a seam a resolved sibling owns, when the assigned node's Done-when requires it, is authored by the assigned worker; the worker records the field and the affected consumer in its own Result and does not edit the resolved node, and the coordinator decides whether the owner's context_rev needs a bump at integration."
