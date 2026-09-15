---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction while executing the renderer conformance suite: stale ID allocation and ambiguous multi-frontier routing.
---

# Scope

Session: executing [[TAS-015-renderer-conformance-suite]] as the frontier child
of [[TAS-009-renderer-backends]], then rolling up that parent, with `bt`,
`graph-check`, and the Tangle skill on 2026-09-12. This node captures skill
and tooling friction for later meta-analysis. It does not fix the skill.

# Findings

## F1. `bt allocate THO` returned an ID that already exists on disk

With [[THO-008-tangle-friction-terminal-capability-detection]] present in
`nodes/resolved/`, this session asked the sidecar to reserve the next feedback
ID:

```
$ uv run --project "$SKILL_ROOT" --frozen bt allocate THO
id: "THO-007"
```

`THO-007-tangle-friction-tui-session` already exists on disk, and `THO-008`
also exists, so the sidecar handed back a two-generations-stale ID. I had
already run `bt reindex nodes` earlier in the session (`nodes: 26, edges: 48`),
so the index knew about all 26 nodes, yet allocation still returned a live ID.

The immediate hazard is concrete: the skill's parallel-creation rule says a
local `find` is never an ID reservation and directs workers to
`bt allocate PREFIX`. A worker that trusts this result would overwrite or
collide with an existing node. This session only avoided the collision because
the existing `THO-007` was readable on disk.

Expected behavior: `bt allocate PREFIX` either reconciles from Markdown before
reserving, or refuses to return an ID at or below the on-disk maximum, or names
the collision so the caller can retry. It should never return an ID that exists
in `nodes/`.

Proposed change: make `allocate` derive the next ID from a fresh Markdown scan
or a checked monotonic counter, and add a regression test that allocates after
an out-of-band node file lands. If reconciliation is too expensive, make
`allocate` fail loudly on collision rather than return a busy ID, and update
SKILL.md's "Parallel worktree contract" to state that allocation is only
collision-free after a reindex.

This is the failure mode `AGENTS.md` names as its worked example; recording the
fresh occurrence so the meta-analysis has a second data point.

## F2. Two hub members can each carry a `next`, so the frontier is ambiguous

The skill derives the frontier by following "each coordinating node's `next`".
At the start of this session two hub members of [[IDX-001-hekate]] had a live
`next`:

```
$ rg -l '^next:' nodes/active nodes/proposed
nodes/active/TAS-016-release-profile-lto.md
nodes/proposed/TAS-009-renderer-backends.md
```

[[TAS-009-renderer-backends]] pointed at
[[TAS-015-renderer-conformance-suite]] and
[[TAS-016-release-profile-lto]] pointed at an unrun release build.
Nothing in the read/execute loop picks between two valid frontier candidates,
and the loop explicitly says `priority` and `active` are not the frontier, so
neither P1-versus-P2 nor one being `active` is a documented tie-break. I was
lucky: the operator supplied the frontier in the prompt. A fresh worker with no
pointer would have to guess, and a wrong guess is not caught by any checker.

Expected behavior: the loop should state how to choose among multiple
`next` routes that all reach a hub, for example that the coordinator names one
frontier and every other live `next` is a candidate to be sequenced, or that the
worker must ask when more than one hub member has an actionable `next`.

Proposed change: add a tie-break rule to the "Read and execute loop" and to the
"Reachability contract", such as preferring the lowest-ID coordinating parent's
`next` or requiring a single deliberate `# Focus` pointer to arbitrate, and
document it in SKILL.md.

# What worked

- `bt init`, `bt claim TAS-015 ... --base-hash <sha256 of the node file>`, and
  `bt release` behaved as documented; releasing with the claim-time base hash
  succeeded after the node had been edited and moved.
- `graph-check nodes` passed at every checkpoint (before the work, after
  resolving TAS-015, and after rolling up TAS-009).
- Deriving the TAS-015 frontier from `TAS-009.next` was unambiguous once the
  operator's pointer was in hand, and all five `Done when` criteria were
  decidable from TAS-015's own files and dependency closure.
- `bt reindex nodes` and `bt status` agreed with the on-disk node count, so the
  stale `allocate` result in F1 was not an indexing failure.

# Result

Two findings captured with exact commands, observed output, expected behavior,
and proposed changes. This node is finished; meta-analysis and any Tangle
changes are separate work.

Area [[IDX-002-tangle-feedback]].
