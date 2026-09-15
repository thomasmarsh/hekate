---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction while executing the shared render layer node.
---

# Scope

Session: executing [[TAS-011-shared-render-layer]] as the frontier child of
[[TAS-009-renderer-backends]] with `bt`, `graph-check`, and the Tangle skill
on 2026-09-12. This node captures skill and tooling friction for later
meta-analysis. It does not fix the skill.

# Findings

## F1. A pinned dependency that is still `proposed` passes both `graph-check` and `bt stale`

The read/execute loop (step 4) says to "confirm it is resolved" for each
context-bearing dependency, and the dependency section says resolution is
detected from the status directory. But the planning step that created the
renderer epic left [[DEF-002-renderer-backend-contract]] in `nodes/proposed/`,
and the executable frontier [[TAS-011-shared-render-layer]] pins it at
`context_rev 1`. Both validators were green on that state:

```
$ uv run --project "$SKILL_ROOT" --frozen graph-check nodes
graph check: passed (22 nodes)
$ uv run --project "$SKILL_ROOT" --frozen bt stale
stale: 0 dependency pins
```

`bt stale` treats a pin as fresh when the target exists and the `context_rev`
matches; it does not consider the target's status. `graph-check` likewise
validates pin syntax, revision mismatch, and reachability, not whether a task's
pinned dependency is resolved. So the documented "confirm it is resolved" gate
is a purely manual step with no tooling support, and a plan can hand a worker a
frontier that the skill says is blocked while every command reports success.

The skill also never says when a knowledge node becomes `resolved`. It says "a
resolved `DEF` or `DEC` is current knowledge unless its sparse `disposition`
says ...", which implies a `DEF` is only authoritative once resolved, but it
gives no completion criterion for a definition and no rule that settled
definitions belong in `resolved/` from the start. The worker must guess whether
to (a) resolve the definition as a prerequisite step, (b) treat a proposed
definition as knowledge anyway and proceed, or (c) halt the frontier. I resolved
`DEF-002` first, moved it to `resolved/` without changing `context_rev`, and
proceeded; nothing in the skill confirms that call.

Expected behavior: the skill should state when a `DEF`/`DEC` is resolved (its
content is settled, so a definition authored during planning should be filed
under `resolved/`), and the tooling should flag a pinned dependency that is not
`resolved` so the manual gate is enforceable. If proposed definitions are
deliberately allowed as pinned assumptions, the skill should say so explicitly
and `bt stale` should still surface it.

Proposed change: (a) add to the "Dependency revisions and staleness" section
that a pinned dependency must be `resolved` before execution and that a settled
`DEF`/`DEC` is resolved; (b) extend `graph-check` (or `bt stale`) to report a
pinned dependency whose target is `proposed` or `blocked`; and (c) if
definitions are exempt, document the exemption and the intended ordering instead
of leaving the loop's "confirm it is resolved" unenforceable.

# What worked

- `bt init`, `bt claim TAS-011 ... --base-hash <sha256 of the node file>`,
  `bt allocate THO`, and `bt reindex` behaved as documented.
- Deriving the frontier from `TAS-009`'s `next`, then validating the candidate's
  header, was unambiguous.
- `graph-check nodes` passed at every checkpoint, including after resolving
  `DEF-002`, resolving `TAS-011`, and advancing `TAS-009.next` to `TAS-012`.

# Result

One finding captured with exact commands, observed output, expected behavior,
and a proposed change. This node is finished; meta-analysis and any Tangle
changes are separate work.

Area [[IDX-002-tangle-feedback]].
