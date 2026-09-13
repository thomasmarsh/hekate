---
context_rev: 1
updated: 2026-09-12T15:21:14Z
summary: Braintree friction starting Phase 2 Increment 0: `braintree hash` rejects a node path, the frontier recipe still lists every up-front child, the THO/FBK feedback split recurs, and gating on un-tracked plan work has no stated status.
---

# Scope

Session: taking the Phase 2 Increment 0 frontier, confirming the Phase 1 entry
gate, and capturing the versioned Phase 1 baseline, with `braintree`
0.5.0+gc99a080 and the installed skill on 2026-09-12. This node captures skill
and tooling friction for later meta-analysis. It does not fix the skill or the
sidecar.

# Findings

## F1. `braintree hash` rejects a node path, but the skill hands workers a path

The parallel-worktree contract says the coordinator assigns each worker "a direct
node path" and that the worker "hashes its starting Markdown node with
`braintree hash`". The vault and node contracts are written in terms of files and
paths, so passing the assigned path is the natural reading. It fails:

```
$ braintree hash nodes/proposed/TAS-018-phase-2-increment-0-baseline-extension-contract.md
error: "unknown node: nodes/proposed/TAS-018-phase-2-increment-0-baseline-extension-contract.md"
help: "Use a bare ID or a full node name from the vault."
```

A bare ID then succeeds (`braintree hash TAS-018`). A fresh sidecar is not the
cause: `braintree init` in an empty sidecar directory followed by
`braintree hash TAS-019` without an explicit `braintree index` returns the hash,
so the command reconciles Markdown on its own.

Expected behavior: the command accepts the vault-relative node path the skill
tells a worker to hold, or the skill states the accepted argument forms (bare ID
or bare filename stem) next to the command and in the worktree contract.

Proposed change: accept a path in `braintree hash`/`claim`/`release`, or add the
accepted forms to `SKILL.md`. The error message is actionable, so severity is
low; the mismatch is between the skill's path-oriented handoff and the tool's
ID-oriented arguments.

## F2. Recurrence: the Frontier recipe lists every up-front plan child (THO-011 F1)

With the eight requested increment children created up front, the documented
Frontier recipe still returns all eight:

```
$ rg --files-without-match '^next:.*\[\[' nodes/*/ | rg '/(active|proposed|blocked)/'
nodes/proposed/TAS-019-... (and TAS-020 through TAS-025)
```

Only the coordinator's `next` identifies the deliberate frontier (TAS-018). A
fresh worker who follows the documented first orientation step sees eight
candidates. Recorded as a recurrence; the fix proposed in THO-011 F1 still
applies.

## F3. Recurrence: the skill's FBK contract and this repo's THO mechanism diverge (THO-011 F3)

`AGENTS.md` mandates "one resolved `THO` node under `IDX-002-braintree-feedback`
per session", while the skill declares `FBK` "the one feedback marker" and offers
`braintree feedback record`/`feedback scan`. This session again followed
`AGENTS.md`, so the skill's discovery path still cannot see the friction:

```
$ braintree feedback scan .
feedback: 0 nodes
```

Recorded as a recurrence; the fix proposed in THO-011 F3 still applies.

## F4. No stated status for a node gated on un-tracked plan work

Phase 2 Increment 0's gate requires the Phase 1 definition of done, but Phase 1
Increments 1-6 exist only as plan text in `PHASE_1_PLAN.md` and are not graph
nodes. The skill's blocked/proposed guidance covers an unowned external
dependency and a sibling decision, but not "gated on plan work that no node
owns". Both readings were arguable: `proposed` because the increment's own
non-behavioral deliverables could still be built, or `blocked` because its
completion criterion cannot be met. I marked the node `blocked` with an action
`next` naming the unblock work, and `braintree check nodes` accepted it.

Expected behavior: the skill states which status a gate on un-tracked plan work
takes, and whether the unblock action is to admit that plan work as nodes.

Proposed change: add a sentence to the decomposition section: a gate whose
prerequisite exists only as plan text is `blocked`, and the `next` is to admit
the prerequisite as nodes. Severity is moderate because the choice determines
the frontier a later worker follows.

# What worked

- `braintree allocate THO` returned THO-012 with no collision, so the
  stale-ID defect in THO-003 F1 and THO-009 F1 did not recur.
- `braintree claim` and `braintree release` used the exact `braintree hash`
  value, and releasing with the claim-time hash succeeded.
- A `blocked` node with a `Blocked by`/`Unblocks when` section and an action
  `next` passed `braintree check nodes`; so did a coordinating parent whose
  `next` targets the now-blocked child.
- `braintree hash` needed no explicit `braintree index` on a fresh sidecar.

# Result

Four findings captured with exact commands, observed output, expected behavior,
and proposed changes; F2 and F3 are recurrences. This node is finished;
meta-analysis and any Braintree changes are separate work.

Area [[IDX-002-braintree-feedback]].
