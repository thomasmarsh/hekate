---
context_rev: 1
updated: 2026-09-12T12:41:26Z
summary: Braintree friction while executing the terminal-rendering survey node.
---

# Scope

Session: executing [[THO-002-terminal-rendering-landscape]] as the frontier child
of [[TAS-009-renderer-backends]] with `bt`, `graph-check`, and the Braintree
skill on 2026-09-12. This node captures tooling and skill friction for later
meta-analysis. It does not fix the skill.

# Findings

## F1. A `context_rev` bump makes `graph-check nodes` fail, so the documented stale workflow cannot be committed

SKILL.md says a semantic change leaves dependents' pins unchanged so one exact
backlink search finds the reconciliation work, and to reconcile stale consumers
"before their dependent execution". My survey node is a semantic change:
`THO-002` went from `context_rev 1` to `2`, which made its only pinned consumer
`DEC-002-terminal-backend-strategy` stale (pin 1, current 2). But
`graph-check nodes` treats any revision mismatch as a hard error unless
`--allow-stale` is passed, and the repo's AGENTS.md requires a passing
`graph-check nodes` before every commit and before handoff. The documented
"leave the pin, reconcile later" workflow is therefore uncommittable as written:
either the change is never committed with the definition, or `--allow-stale` is
required, which neither SKILL.md nor AGENTS.md authorizes.

Observed: after editing `THO-002` to `context_rev: 2` and moving it to
`resolved/`, with `DEC-002`'s pin left at 1, `graph-check` fails. Reproduced on a
copy of the vault that restores `DEC-002`'s pin to 1:

```
$ uv run --project "$SKILL_ROOT" --frozen graph-check /tmp/bt-repro/nodes
error: /tmp/bt-repro/nodes/proposed/DEC-002-terminal-backend-strategy.md: context_rev mismatch for [[THO-002-terminal-rendering-landscape]] (pinned 1, current 2)
exit=1
```

The unmodified vault with `DEC-002`'s pin raised to 2 passes
(`graph check: passed (21 nodes)`).

Expected behavior: the skill should make the intended commit shape explicit.
Either (a) say a `context_rev` bump must reconcile every pinned consumer's pin
and content in the same commit or handoff, which makes stale state transient; or
(b) say a commit may carry stale pins and the mandated gate is
`graph-check --allow-stale`, with reconciliation tracked as separate work.

Proposed change: state the intended shape in SKILL.md's "Dependency revisions
and staleness" section and align AGENTS.md's mandated command, so a worker is
not forced to choose between the staleness contract and a green checker.

## F2. Advancing the frontier past a resolved knowledge node is a judgment call

`TAS-009`'s `next` named `THO-002`, a proposed `THO` (knowledge) node. The
read/execute and decomposition rules are task-centric: they describe executing
and resolving tasks, and say the parent's `next` is one direct child at the
frontier, but they do not say what resolving a frontier knowledge node implies
for the coordinating parent's `next`. The mutation rules also say never to update
unrelated nodes as bookkeeping, which reads as forbidding a touch of `TAS-009`.
I resolved `THO-002` and repointed `TAS-009`'s `next` at
`[[TAS-010-kitty-graphics-poc]]`, on the reading that `next` is routing rather
than bookkeeping.

Expected behavior: the loop should state that when the frontier is a knowledge
node (`THO`/`DEF`/`DEC`), the worker answers and resolves it and advances the
coordinating parent's `next` to the next deliberate frontier child in the same
change.

Proposed change: add that sentence to the "Read and execute loop" section of
SKILL.md.

# What worked

- Deriving the frontier from `TAS-009`'s `next` after a hub backlink search was
  unambiguous once located; `graph-check` reported no structural problems before
  the edits.
- `bt init`, `bt reindex nodes`, and `bt claim` behaved as documented.
- `bt allocate THO` returned `THO-004`, one past the on-disk maximum of
  `THO-003`, and did not collide.

# Result

Two findings captured with evidence and proposed changes. This node is finished;
meta-analysis and any Braintree changes are separate work.

Area [[IDX-002-braintree-feedback]].
