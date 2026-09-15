---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction while executing the terminal viewer node.
---

# Scope

Session: executing [[TAS-012-terminal-viewer-ascii]] as the frontier child of
[[TAS-009-renderer-backends]] with `bt`, `graph-check`, and the Tangle skill
on 2026-09-12. It also covers a one-line release-profile change requested mid
session. This node captures skill and tooling friction; it does not fix the
skill.

# Findings

## F1. No sanctioned path for a trivial change that still needs a commit

Mid-session the user asked for a one-line build-config change: `lto = "thin"` in
`[profile.release]`. `AGENTS.md` says "Every commit that implements tracked work
references its node" and that untracked commits are rejected in review, but the
skill's node-admission rule says "incidental or mechanical cleanup ... never
qualify" as a distinct node. A one-line profile setting is exactly that
mechanical cleanup, yet it still needs a node reference to be committable.

The worker is forced into one of two wrong outcomes: over-admit a node whose
outcome is "add one line to `Cargo.toml`" (this session created
[[TAS-016-release-profile-lto]]), or make an untracked commit that review
rejects. There is no lightweight path such as referencing the owning area hub
without creating a leaf, and no statement that a `build`/`chore` commit may
reference the enclosing increment's node.

Expected behavior: either the skill and `AGENTS.md` should define a non-node
commit path for mechanical changes (for example `Refs` to the enclosing area or
increment node), or the admission rule should carve out a tracked-but-not-a-node
category with a stated commit convention. As written, the two rules guarantee
either node sprawl or review rejection.

Proposed change: add to the skill's decomposition section that a mechanical
change with no independently resumable outcome is recorded in the enclosing
node's `next`/result or committed with `Refs` to that node instead of being
admitted as a new node, and state the matching commit convention in `AGENTS.md`.

# What worked

- Frontier derivation from [[TAS-009-renderer-backends]]'s `next`, dependency
  pin validation, and `bt claim`/`bt release` behaved as documented.
- `graph-check nodes` passed after resolving [[TAS-012-terminal-viewer-ascii]],
  creating [[TAS-016-release-profile-lto]], and advancing `TAS-009.next` to
  [[TAS-014-terminal-capability-detection]].
- The claim base-hash documentation gap in THO-005 F1 is still present, but it
  was already recorded, so this session did not duplicate it.

# Result

One finding captured with the exact situation, expected behavior, and a proposed
change. This node is finished; meta-analysis and any Tangle changes are
separate work.

Area [[IDX-002-tangle-feedback]].
