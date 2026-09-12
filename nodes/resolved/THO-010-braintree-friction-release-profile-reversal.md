---
context_rev: 1
updated: 2026-09-12T14:46:01Z
summary: Braintree friction reversing an already-committed task outcome: update-in-place versus supersession and commit-body drift are undefined.
---

# Scope

Session: executing [[TAS-016-release-profile-lto]] after the operator reversed
its direction from "enable thin LTO" to "no LTO until measured.", with `bt`,
`graph-check`, and the Braintree skill on 2026-09-12. This node captures skill
and tooling friction for later meta-analysis. It does not fix the skill.

# Findings

## F1. No rule for a reversed outcome on a task that was already partly implemented

[[TAS-016-release-profile-lto]] was admitted to enable `lto = "thin"`, and that
change was already committed as `c654c7b` with `Refs TAS-016`. Before the
node was resolved, the operator reversed the requirement: no LTO at all. The
skill gives two tools that both seem to apply and does not say which wins:

- "Prefer updating the existing node when new information advances the same
  outcome, question, component, decision, or defect." Updating TAS-016 in place
  keeps the "release-profile LTO decision" concern in one node.
- `disposition: superseded` with a replacement link, for an "obsolete node".
  The commit `c654c7b` now points at a node whose `# Outcome` says the opposite
  of the commit body, so the old direction is arguably obsolete.

I updated TAS-016 in place, bumped `context_rev` from 1 to 2 for the semantic
reversal, and resolved it. The cost is that a historical commit's `Refs` now
resolves to a node that contradicts it, and nothing in the graph or checker
flags that. Choosing supersession instead would have produced a replacement
`DEC` node and left TAS-016 as a tombstone, which the admission rule's
"prefer updating" language discourages.

Expected behavior: the skill should state whether reversing a task's outcome
after partial implementation is an in-place update (with `context_rev` bump) or
a supersession, and what the resolved node owes a commit that referenced the
old direction.

Proposed change: add a short "Reversal" paragraph to the mutation rules: update
in place while the concern is unchanged and note the reversal in the body, or
set `disposition: superseded` and relink when the concern itself is retired;
either way record the reversed commit ID in the node body so the drift is
traceable.

## F2. `bt allocate THO` returned an existing ID again (recurrence, not a new finding)

`bt allocate THO` returned `THO-008`, which already exists on disk alongside
`THO-009`; the next free ID was `THO-010`. This is the same defect recorded as
[[THO-009-braintree-friction-renderer-conformance]] F1, now reproduced a third
session in a row after an explicit `bt reindex nodes`. Recorded here only as a
recurrence data point for the pending meta-analysis; not re-argued, and this
session used `THO-010` from the on-disk scan.

# What worked

- Reusing the existing TAS-016 node for the reversed decision avoided admitting
  a speculative replacement node.
- `bt claim TAS-016 ... --base-hash`, `bt release TAS-016 ...` with the
  claim-time base hash, and `graph-check nodes` all behaved as documented, and
  `cargo build --workspace --release` passed after the profile edit.
- `graph-check nodes` reported `passed (27 nodes)` after the reversal edit and
  the status move.

# Result

One new finding (F1) with the exact situation, expected behavior, and a
proposed change, plus one recurrence note (F2). This node is finished;
meta-analysis and any Braintree changes are separate work.

Area [[IDX-002-braintree-feedback]].
