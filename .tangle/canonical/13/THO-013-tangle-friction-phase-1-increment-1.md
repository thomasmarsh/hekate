---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction taking Phase 1 Increment 1: `tangle hash` prints a labelled object rather than the documented hex digest, it still rejects the node path the worktree contract hands a worker, and the THO/FBK feedback split recurs.
---

# Scope

Session: admitting and starting the next frontier task from Phase 1 increments
1-6 (the new `TAS-026` node) with `tangle` 0.5.0+gc99a080 and the installed
skill on 2026-09-12. This node captures skill and tooling friction for later
meta-analysis. It does not fix the skill or the sidecar.

# Findings

## F1. `tangle hash` does not print the documented base hash

The skill says: "`tangle hash NODE` prints the base hash a claim records: the
SHA-256 hex digest of the node file's raw UTF-8 bytes, frontmatter included. Get
it from `tangle hash` rather than reimplementing the algorithm, and pass that
same starting value to `tangle claim`." The observed command printed a
labelled two-line object, not a bare digest:

```
$ tangle hash TAS-026
node: "TAS-026"
content_hash: "a3d6f776326c0ae8005df0072ba346794db3d7080752e3204e82677c9d5c0ccb"
```

Passing that whole two-line string as `--base-hash` made `tangle claim`
succeed, and the command echoed the entire string back as `base_hash`, so it is
unclear whether the intended operand is the `content_hash` field alone or the
whole output. The skill's claim/release contract says "the exact `tangle
hash` value", which reads as the field but is not stated.

Expected behavior: print only the digest the claim records, or state that the
operand is the `content_hash` field of a labelled result and show it in the
`hash`/`claim`/`release` examples.

Proposed change: emit a bare digest from `tangle hash` (or document the
labelled form and which field to pass). Severity is moderate because a mismatch
between the claim-time and release-time operand fails release.

## F2. Recurrence: `tangle hash` still rejects a node path (THO-012 F1)

The parallel-worktree contract hands a worker "a direct node path" and says to
hash "its starting Markdown node", but the command accepts only a bare ID. The
error remains actionable, so this is a recurrence of the mismatch between the
path-oriented handoff and the ID-oriented arguments.

```
$ tangle hash nodes/active/TAS-026-phase-1-increment-1-general-scenario-foundation.md
error: "unknown node: nodes/active/TAS-026-phase-1-increment-1-general-scenario-foundation.md"
```

Proposed change: the same as THO-012 F1 — accept a path, or state the accepted
forms next to the command and in the worktree contract.

## F3. Recurrence: the skill's FBK contract and this repo's THO mechanism diverge (THO-011 F3, THO-012 F3)

`AGENTS.md` still mandates one resolved `THO` node under
`IDX-002-tangle-feedback` per session, while the skill declares `FBK` the
one feedback marker and offers `tangle feedback record`/`feedback scan`.
Following `AGENTS.md` keeps the friction invisible to the skill's discovery
path:

```
$ tangle feedback scan .
feedback: 0 nodes
```

Recorded as a recurrence; the fix proposed in THO-011 F3 still applies.

# What worked

- `tangle allocate TAS` and `tangle allocate THO` returned unused ids with
  no sidecar collision.
- Creating a plain `active` leaf node (no pre-created children) under the root
  hub passed `tangle check nodes`; its action `next` is a valid frontier.
- Pinning the resolved predecessor with `Depends on
  [[TAS-001-phase-1-increment-0]] at context_rev 1.` passed the checker.

# Result

Three findings captured with exact commands, observed output, expected behavior,
and proposed changes; F2 and F3 are recurrences. This node is finished;
meta-analysis and any Tangle changes are separate work.

Area [[IDX-002-tangle-feedback]].
