---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: An active task's next field rejected a plain action sentence that named a sibling node with a wikilink, and the pinned-consumer search prescribed for the metric-definition bump matches its own quoted text.
tangle_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Two findings from settling metric definition v2 under `TAS-046`.

## A next action sentence may not name another node

Attempted: gave an active task's `next` a plain action sentence that named a
sibling node, `Move this node to .tangle/resolved/ with a Closes TAS-046
commit once TAS-045's next advances to the scenario-variants slice.`, where the
first draft spelled the scenario-variants slice as a wikilink. Then ran
`tangle check`.

Friction: the check failed with `frontier is not a direct child` and named no
link, so the offending link had to be found by trial: the wikilink named
`TAS-047-increment-6-scenario-variants-and-experiment-spec`, which is TAS-045's
direct child and not TAS-046's. SKILL.md says a `next` is either "a plain action
sentence, `Do X.`" or "a single `[[direct-child]]` link", but it does not say an
action sentence must contain no wikilink at all, and the checker appears to
parse any wikilink inside `next` as the route form.

Improvement: state in SKILL.md that an action-sentence `next` must contain no
wikilink — links belong to the route form only — and make `tangle check` name
the wikilink it treated as the frontier route instead of printing only
`frontier is not a direct child`.

## The prescribed pinned-consumer search matches its own quoted text

Attempted: ran the exact search `TAS-045` prescribes for the definition bump,
`rg -n -F 'Depends on [[DEF-004-metric-definition-v1]] at context_rev '
.tangle`, before and after adding `disposition: superseded` to `DEF-004`.

Friction: the search matches any occurrence of the pin text in the vault,
including the line where `TAS-045` quotes the search itself, so it cannot be
read as "zero consumers" without inspection; writing the command into a node body
to record evidence makes the next run non-zero for a reason that has nothing to
do with a stale consumer. The vault has no pinned edge on `DEF-004` at all
(`rg -n '^Depends on \[\[DEF-004' .tangle` matches nothing), which is the
signal that was wanted.

Improvement: prescribe the anchored form `rg -n '^Depends on \[\[DEF-004-metric-definition-v1\]\]
at context_rev ' .tangle` (or without the trailing space), which matches a pin
line and not a quoted command, and say in SKILL.md that a quoted search inside a
node is a self-match rather than a consumer.
