---
context_rev: 1
updated: 2026-09-14T13:03:29Z
summary: TAS-095 bundles four multi-hour outcomes under one Done when, so each dispatch exceeded the...
braintree_revision: 0.6.0+g169bad5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Orchestrated Phase 2 Increment 2 (TAS-020): verified the tree with one scout, admitted the missing compiled-adjacency prerequisite TAS-112, dispatched one writer per slice, and recovered two 30-minute worker timeouts from green partials.
Friction: TAS-095 bundles four multi-hour outcomes under one Done when, so each dispatch exceeded the 30-minute window; the recovery procedure worked but cost a run each. A resolving node that is its parent current next makes plain braintree check fail, and the acceptance list named plain check. A handoff write-set named the wrong owning call-site function. The node record slug truncated mid-word.
Improvement: See the findings below: split a Done when that bundles several multi-hour deliverables into direct children before dispatch, have the acceptance line name --allow-pending-advance when the resolved node is the parent current next, name the owning call-site function in a seam reference, and let node record keep a whole-word slug.

## Finding 1 — a Done when that bundles several multi-hour outcomes

Attempted: dispatched TAS-095's outbound hazard matrix, then its return-leg
obstruction, as two fresh writers scoped from the node's recorded remaining
scope.
Friction: the node's `# Done when` bundles four independently substantial
outcomes (outbound hazard matrix, return obstruction, cross-facility return leg,
current+destination constraints). Each dispatch reached real, green partial work
but exceeded the 30-minute window, so the coordinator paid the timeout-recovery
cost twice and the leaf is still `active`.
Improvement: the leaf brief template already grants a worker authority to
create a direct child for an outcome that will not fit; the coordinator should
exercise that before dispatch when a `# Done when` names several multi-hour
deliverables, so each child owns one outcome with its own completion evidence.
That keeps the bundled node's `# Done when` intact (no silent re-scoping of the
parent) while sizing the session to the slice, per the repo's one-session rule.

## Finding 2 — resolving the parent's current `next` fails plain `braintree check`

Attempted: asked the TAS-112 worker to resolve its node and run `braintree check`
as acceptance; TAS-092's `next` named TAS-112, and TAS-092 was outside the
worker's write set.
Friction: a correct resolution made plain `braintree check` fail with
`next-resolved-node`, which the acceptance list could not satisfy; the worker had
to use `--allow-pending-advance TAS-092` and explain why the named command could
not pass.
Improvement: when the node being resolved is the current `next` of a parent
outside the write set, the acceptance line should name `braintree check
--allow-pending-advance <PARENT>` explicitly, and the report should surface the
pending advance as the coordinator's integration action. This recurs across
leaves and is cheap to pre-empt in the brief.

## Finding 3 — a handoff seam named the wrong owning call site (recurrence of FBK-031 Finding 1)

Attempted: the TAS-112 brief said the regions were built at `compiled.rs` ~2846
(`compile_validated`) and that `compile_facility_adjacencies` could read them.
Friction: `compile_facility_adjacencies` is actually called from `compile_v2`
(~2739), where `CompiledScenario.regions` does not yet exist; matching the stated
design required extracting a `compile_regions` helper and building the polygons
earlier. The `file:line` seam citation named the wrong stage of the same file.
Improvement: a brief that cites a seam in a large module should name the owning
call-site function, not only a line, because the same helper can be invoked from
a different compilation stage than the field it must read. Prefer `path
(Symbol)` citations, as FBK-031 Finding 5 recommends.

## Finding 4 — capture commands truncate slugs and summaries mid-word

Attempted: recorded TAS-112 and FBK-032 with `braintree node record` and
`braintree feedback record`.
Friction: the node slug truncated to `...-lateral-co` (a mid-word fragment that
is now the wikilink basename every `next` must reproduce exactly), and the
derived FBK summary was cut to `...under...` with a warning line. The truncation
is collision-safe but produces an awkward, hard-to-reference basename.
Improvement: prefer a whole-word slug boundary with a short stable suffix, or
accept an explicit `--slug` in the brief whenever the derived slug would be
clipped mid-word, so the graph's stable basenames stay readable and easy to
reproduce in wikilinks.
