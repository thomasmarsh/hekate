---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Tangle friction while executing the terminal capability-detection node.
---

# Scope

Session: executing [[TAS-014-terminal-capability-detection]] as the frontier
child of [[TAS-009-renderer-backends]] with `bt`, `graph-check`, and the
Tangle skill on 2026-09-12. This node captures skill and tooling friction for
later meta-analysis. It does not fix the skill.

# Findings

## F1. A frontier node's `Done when` can require a sibling that is not in its dependency closure

[[TAS-014-terminal-capability-detection]] was the deliberate frontier: the
previous session's [[THO-007-tangle-friction-tui-session]] records advancing
`TAS-009.next` to it, and its header validated. But one of its `Done when`
criteria asserts a property of a *different* node:

> Selecting the Kitty backend always uses the bounded lifecycle mandated by
> [[DEC-002-terminal-backend-strategy]].

The bounded lifecycle is implemented by [[TAS-013-terminal-viewer-kitty]], which
is an independent sibling of TAS-014, not a dependency of it. The two share only
`Parent [[TAS-009-renderer-backends]]`, which the command below confirms by
listing both files as having a `Parent` line (the tool prints only filenames here
because the full match would embed wikilinks; see F2):

```
$ rg -l '^Parent' nodes/proposed/TAS-013-terminal-viewer-kitty.md nodes/resolved/TAS-014-terminal-capability-detection.md
nodes/proposed/TAS-013-terminal-viewer-kitty.md
nodes/resolved/TAS-014-terminal-capability-detection.md
```

Nothing in TAS-014 pins TAS-013, and TAS-014's own `# Context` names only
[[DEC-002-terminal-backend-strategy]] and [[TAS-012-terminal-viewer-ascii]]. So
the frontier node cannot fully satisfy its own acceptance criteria without
writing into a sibling's scope, and every checker stays green on that state:

```
$ uv run --project "$SKILL_ROOT" --frozen graph-check nodes
graph check: passed (25 nodes)
```

The read/execute loop (step 2) says to follow the coordinator's `next` and
"validate the candidate's status and header"; step 4 confirms pinned
dependencies are resolved. Neither step asks whether each `Done when` criterion
is decidable from the node's own dependency closure. `graph-check` validates
links, lifecycle, and pin revisions, so it cannot see a criterion that silently
depends on an undeclared sibling. The worker is left to either narrow the node
after the fact (do the gating and defer the lifecycle claim to TAS-013) or expand
into TAS-013, and the skill gives no rule for choosing.

Expected behavior: the skill should state that a `Done when` criterion which
asserts a property of another node's implementation belongs to that node. Either
the dependency is declared (TAS-014 depends on TAS-013) so the node is
correctly blocked until it lands, or the criterion is restated as a constraint
the current node's own interface must carry (for example, "the selection seam
exposes exactly one Kitty construction point, so no alternate lifecycle path
exists"), and the owning node keeps the implementation claim. The worker should
not have to make that call silently.

Proposed change: add to the decomposition guidance that `Done when` criteria
must be decidable from the node's declared dependency closure, and that a
criterion naming another node's behavior is either a declared dependency or a
statement about this node's interface. Add a coordinator step before resolving a
parent: confirm each child's `Done when` is decidable from that child's own
closure. If it is not mechanically checkable, say so explicitly so the manual
gate is known to be manual.

## F2. `graph-check` parses wikilinks inside fenced code blocks

While recording F1, the fidelity requirement ("record the exact command and
observed output") collided with the checker. The natural evidence for F1 is the
output of a `Parent`-backlink search, and every matching line contains a
wikilink. Embedding that output verbatim in a fenced code block made
`graph-check` reject the node:

```
$ uv run --project "$SKILL_ROOT" --frozen graph-check nodes
error: nodes/resolved/THO-008-...: broken link (the code-block line was read as a link)
error: nodes/resolved/THO-008-...: broken link (the elided output was read as a link)
exit=1
```

The two "links" the checker extracted were a truncated fragment of a `Parent`
line and the literal double-bracket ellipsis placeholder that a human would
use to elide
output. The checker scans the raw Markdown, so fenced code blocks are not
exempt. That makes the mandated evidence style ("exact command and observed
output") dangerous for any node whose evidence contains link syntax, and the
worker must discover the constraint by failing a check.

Expected behavior: `graph-check` should skip fenced code blocks when extracting
links, matching Obsidian and CommonMark, so command output can be quoted
verbatim. If code blocks are deliberately scanned, the skill should document an
escape and this session's failure mode.

Proposed change: make `graph-check` ignore fenced code blocks (at minimum
triple-backtick and triple-tilde fences) during link extraction, and add a
regression test that a wikilink inside a fenced block does not raise a broken
link. Until then, document the workaround the skill expects (elide or rewrite
the output).

# What worked

- `bt init`, `bt claim TAS-014 ... --base-hash <sha256 of the node file>`,
  and `bt release` behaved as documented.
- Deriving the frontier from `TAS-009.next` was unambiguous, and
  `graph-check nodes` passed at every checkpoint, including after resolving
  TAS-014 and advancing `TAS-009.next` to [[TAS-013-terminal-viewer-kitty]].
- The node's own `# Context` pins and the `Depends on` wikilink-plus-`context_rev`
  convention made the intended split (detection here, rendering in TAS-013)
  discoverable once the criterion was read against the dependency list.
- The claim base-hash documentation gap in
  [[THO-005-tangle-friction-kitty-spike]] is still present but already
  recorded, so this session did not duplicate it.

# Result

Two findings captured with the exact commands, observed output, expected
behavior, and proposed changes. This node is finished; meta-analysis and any
Tangle changes are separate work.

Area [[IDX-002-tangle-feedback]].
