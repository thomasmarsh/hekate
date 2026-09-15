---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction while executing the Kitty graphics spike node.
---

# Scope

Session: executing [[TAS-010-kitty-graphics-poc]] as the frontier child of
[[TAS-009-renderer-backends]] with `bt`, `graph-check`, and the Tangle skill
on 2026-09-12. This node captures tooling and skill friction for later
meta-analysis. It does not fix the skill.

# Findings

## F1. The claim base hash has no documented algorithm and no way to obtain it

SKILL.md requires a worker to "hash its starting Markdown node" and pass it as
`bt claim NODE AGENT --base-hash HASH`, but it never says which hash. The sidecar
stores `sha256(text)` of the raw node file (`src/tangle/index.py`), and
`bt search`, `bt backlinks`, `bt status`, and `bt location` expose no content
hash, so a worker must either read the Python or guess. `bt status` output in
this session showed only project, sidecar, `initialized`, and `active_claims`;
no per-node hash.

Expected behavior: the skill should name the exact hash (SHA-256 hex of the raw
UTF-8 file bytes, including frontmatter) and, better, `bt` should expose it, for
example `bt hash NODE` or a hash field in `bt search`/`bt status`, so the value
the claim compares against is obtainable from the tool rather than inferred.

Proposed change: document the algorithm in SKILL.md's hybrid-sidecar section and
add a `bt hash` command (or a hash field in existing read commands).

## F2. `bt release` with a mismatched base hash reports `no-op` and exits zero, leaving the lease held

After editing the claimed node, releasing with the *current* file hash printed
`result: "no-op"` with exit status 0; the lease was still held
(`active_claims: "1"`). Releasing with the *starting* hash printed
`result: "released"` and dropped the claim (`active_claims: "0"`).

```
$ uv run --project "$SKILL_ROOT" --frozen bt release TAS-010-kitty-graphics-poc pi-tas010 --base-hash <current>
result: "no-op"
node: "TAS-010-kitty-graphics-poc"
exit=0
$ uv run --project "$SKILL_ROOT" --frozen bt status   # active_claims: "1"

$ uv run --project "$SKILL_ROOT" --frozen bt release TAS-010-kitty-graphics-poc pi-tas010 --base-hash <starting>
result: "released"
exit=0
```

SKILL.md says the claim is "released with `bt release`" and to "release the
matching claim" before handoff, but does not say that the release hash must be
the recorded starting hash even though the node has since changed, nor that a
mismatch is a silent no-op. A worker who computes the hash from the edited file
sees exit 0 and a `no-op` line that reads like success, and can hand off with the
lease still held and blocking other workers.

Expected behavior: a mismatched `bt release` should fail with a non-zero exit and
an explicit message, or `no-op` should be documented as "nothing released: hash
or owner did not match" and named distinctly from success.

Proposed change: return a distinct non-zero status for a hash/owner mismatch and
state the release-hash rule in SKILL.md.

# What worked

- `bt init`, `bt reindex nodes`, `bt claim`, and `bt allocate THO` behaved as
  documented. `bt allocate THO` returned `THO-005`, one past the on-disk maximum
  of `THO-004`, and did not collide.
- `graph-check nodes` passed at every checkpoint once the node was coherent.
- Deriving the frontier from `TAS-009`'s `next` and confirming the parent's
  `Done when`/criteria before starting was unambiguous.

# Result

Two findings captured with exact commands and observed output, plus proposed
changes. This node is finished; meta-analysis and any Tangle changes are
separate work.

Area [[IDX-002-tangle-feedback]].
