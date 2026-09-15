---
status: proposed
context_rev: 1
priority: P3
updated: 2026-09-15T01:22:43Z
summary: Triage the accumulated Tangle feedback nodes FBK-001 through FBK-018 into concrete, ordered Tangle improvements, because the feedback notes are evidence rather than the fix.
next: Triage FBK-001 through FBK-018 into a concrete, ordered Tangle improvement change set and record it in this node.
---

# Outcome

Aggregate the accumulated `FBK` feedback nodes into a concrete Tangle
improvement change set. The feedback nodes are the evidence; this task owns the
fix. It should group the notes by theme, decide which changes are worth making,
and record the resulting change set (a proposed SKILL.md or tooling edit) so a
later implementation task can act on it.

The known themes from the current backlog:

- Dependency-pin and checker behavior when a predecessor is unresolved
  (FBK-001).
- `tangle hash` / `claim` operand forms and base-hash validation
  (FBK-002, FBK-004).
- Slice write-set scope versus a gate that needs a primitive or golden outside
  it (FBK-003, FBK-008, FBK-009).
- Lease timing and report-timeout handoff (FBK-005, FBK-010).
- Resolution ownership and the coordinating parent's `next` (FBK-006,
  FBK-011, FBK-012, FBK-017).
- Timestamp honesty in handed-off nodes (FBK-007).
- `git mv` staging trap (FBK-013, and recurrences).
- Reusing a resolved sibling's seam by widening visibility (FBK-014, FBK-015).
- A claimed node with a false premise, and the mid-session 0.6.0 vault
  relocation (FBK-015, FBK-016, FBK-018).

This task does not gate Phase 1 Increment 5 or any other increment; it is P3
backlog maintenance.

# Done when

- The feedback nodes are grouped into concrete improvement themes with a
  decision (adopt / defer / reject) per theme.
- The adopted themes are written as an ordered, actionable change set naming
  the exact `SKILL.md` section or tooling behavior to change.
- The notes stay in place as evidence; this task records conclusions, not a
  duplicate of the notes.

# Context

Area [[IDX-002-tangle-feedback]].

The installed Tangle skill requires one `FBK` node per session of friction and
`AGENTS.md` requires aggregating those notes into a separate task under the same
hub. This is that task. It remains `proposed`; it must not gate Increment 5.

# Result

Pending.
