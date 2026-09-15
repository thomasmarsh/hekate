---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction decomposing a requested full plan: the Frontier recipe lists every up-front proposed child, the read loop points at an absent index-map recipe, and THO friction notes are invisible to `feedback scan`.
---

# Scope

Session: decomposing `PHASE_2_PLAN.md` into
[[TAS-017-phase-2-mixed-traffic]] plus eight proposed increment children, with
`tangle` 0.5.0+gc99a080 and the installed skill on 2026-09-12. This node
captures skill and tooling friction for later meta-analysis. It does not fix the
skill or the sidecar.

# Findings

## F1. The Frontier recipe reports every up-front plan child as frontier

The skill now tells workers that "A user-requested plan is not speculative
decomposition: create its children up front as `proposed` work", and the read
loop says the Frontier recipe "returns the unfinished nodes whose `next` is an
action rather than a wikilinked child route, so a coordinating node's `next`
target
appears instead of the coordinator." After creating the eight requested
increment children, each carrying its own action `next`, the documented recipe
returned all eight:

```
$ rg --files-without-match '^next:.*\[\[' nodes/*/ | rg '/(active|proposed|blocked)/'
nodes/proposed/TAS-018-phase-2-increment-0-baseline-extension-contract.md
nodes/proposed/TAS-019-phase-2-increment-1-facilities-narrow-modes.md
nodes/proposed/TAS-020-phase-2-increment-2-lateral-passing-wrong-way.md
nodes/proposed/TAS-021-phase-2-increment-3-heavy-articulated-vehicles.md
nodes/proposed/TAS-022-phase-2-increment-4-transit-service.md
nodes/proposed/TAS-023-phase-2-increment-5-pedestrian-groups-pairwise.md
nodes/proposed/TAS-024-phase-2-increment-6-mixed-mode-safety-outputs.md
nodes/proposed/TAS-025-phase-2-increment-7-release-demonstration.md
```

Only [[TAS-018-phase-2-increment-0-baseline-extension-contract]] is the
deliberate frontier: its parent's frontmatter is
`next: [[TAS-018-phase-2-increment-0-baseline-extension-contract]]`. The recipe
does not distinguish the coordinator's named frontier child from the seven
sequenced-but-not-frontier siblings, so a fresh worker who follows the
documented first orientation step sees eight legitimate candidates; THO-009 F2
already recorded the related two-hub-member case.

Expected behavior: the recipe or the loop text around it yields the one
deliberate frontier child, for example by following each hub coordinator's
`next` route, or states explicitly that for an up-front user-requested plan only
the coordinator's `next` target is the frontier and other action-`next`
`proposed` siblings are merely sequenced.

Proposed change: derive the Frontier query from coordinating nodes' `next`
routes, or add the explicit caveat to the "Read and execute loop" and the
Common queries comment.

## F2. Read loop step 2 points at a Frontier recipe the vault may not contain

Read and execute loop step 2 says: "run the `Frontier` recipe in
`nodes/index-map.md` to list the current frontier directly". This vault's
index-map contains only routing pointers:

```
$ cat nodes/index-map.md
# Hekate
...
- Indexes [[IDX-001-hekate]].
- Indexes [[IDX-002-tangle-feedback]].
```

There is no `# Focus` list and no recipe section. The recipe text exists only in
SKILL.md under "Common queries". Neither `tangle init` nor `tangle
feedback record` seeds recipes into `nodes/index-map.md`, and `tangle check`
does not require them, so the documented first orientation step is a dead
reference in this vault and the worker must fall back to the SKILL.md snippet.

Expected behavior: the loop names the recipe's canonical location or an explicit
fallback when the vault's index-map has no recipe, and tooling that creates an
index-map seeds the recipe.

Proposed change: have the read loop say to run the Common queries recipe when
`nodes/index-map.md` has none, and optionally have `tangle init` place the
recipe in a new index-map.

## F3. The repo's THO friction mechanism is invisible to the skill's FBK path

`AGENTS.md` mandates: "Add one resolved `THO` node under
`IDX-002-tangle-feedback` per session", and [[IDX-002-tangle-feedback]]
repeats that mechanism. The installed skill now declares the opposite:
"A consuming project records Tangle friction as an `FBK` node. The `FBK`
type is the one feedback marker, so `find nodes -name 'FBK-*.md'` discovers
feedback from Markdown alone", backed by `tangle feedback record` and
`tangle feedback scan`. This vault follows its repo instruction, so:

```
$ find nodes -name 'FBK-*.md' | wc -l
0
$ find nodes -name 'THO-*.md' | wc -l
9
$ tangle feedback scan .
feedback: 0 nodes
```

Nine prior friction notes are therefore invisible to the skill's own discovery
command, and a future session must choose between two authoritative mechanisms
with no stated precedence.

Expected behavior: the Feedback section states how project-level instructions
interact with the `FBK` contract, and `feedback scan` can see the friction the
project actually records, for example via a type or route option.

Proposed change: document repo-override precedence in the Feedback section and
let `feedback scan`/`find` accept a configured friction type (or provide a
THO-to-FBK migration); otherwise state that `AGENTS.md` and `IDX-002` should be
updated to `tangle feedback record`.

## F4. Recurrence: a double-bracket token in inline code is still parsed (THO-008 F2)

Quoting the skill's Frontier sentence verbatim in this node produced
`error: nodes/resolved/THO-011-...: broken link` because the checker read a
double-bracket `child` token inside inline backticks as a wikilink. The quote
was reworded until the plain gate passed. This is the same unmasked-parsing
root cause recorded in THO-008 F2 (which reproduced it with a fenced code
block), now reproduced for inline code in prose and in a resolved node;
recorded as a recurrence data point, not re-argued. The immediate workaround is
the same: never reproduce a link-shaped token in quoted skill text, in code or
prose.

# What worked

- `tangle check nodes` passed after creating nine proposed nodes with
  `Parent`/`Area` routes and a single coordinator `next`; the checker accepted
  an up-front proposed tree without pinning proposed siblings, matching the
  documented "pinned dependency whose target is proposed" rejection.
- `tangle allocate TAS` returned TAS-017 through TAS-025 with no collisions,
  unlike the recurring stale-ID defect recorded in THO-003 F1 and THO-009 F1.
- `tangle hash`, `claim`, and `release` on [[IDX-001-hekate]] behaved as
  documented, including releasing with the claim-time output.
- The skill's `next` grammar (single wikilinked direct child for a coordinator)
  was sufficient to make the frontier unambiguous in the graph itself.

# Result

Four findings captured with exact commands, observed output, expected behavior,
and proposed changes; F4 is a recurrence of THO-008 F2. This node is finished;
meta-analysis and any Tangle changes are separate work.

Area [[IDX-002-tangle-feedback]].
