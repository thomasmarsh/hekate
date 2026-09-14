---
context_rev: 1
updated: 2026-09-14T13:26:28Z
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

## Finding 5 — the skill has no operational model for node load size or opt-in reconnaissance

Attempted: as coordinator, sized Phase 2 Increment 2 leaves for fresh workers.
Each leaf body was small, but the real cost of a session was loading the context
around it: a 6,300-line `sim.rs`, a 1,500-line contract, and resolved-sibling
modules. Workers handed an exact seam landed green slices; the workers that had
to locate their own seam timed out at 30 minutes (Finding 1).
Friction: the skill's rule "a node owns one durable outcome, not a session" is
correct but not operational. A durable outcome can still require far more context
than one agent can load in a session, and the skill provides no relation for a
node to *index* opt-in context nodes: every session re-pays the same orientation
cost because there is no sanctioned place to record and link shared
reconnaissance. `braintree check` neither measures nor warns on node body size,
so "small and fully loaded" is unenforced, and there is no scoping recipe that
turns a bundled `# Done when` into right-sized children.
Improvement: (1) define an explicit, non-pinned reference relation from a work
node to context (`THO` reconnaissance) nodes, so shared orientation is opt-in
rather than restated, and have `braintree node` report those references; (2) add
a size check or `braintree size NODE` that warns when a node body exceeds a
loadable bound; (3) document a scoping recipe — split a `# Done when` that bundles
independent deliverables, hand a worker one verifiable slice plus its exact seam
(`path (Symbol)`), and defer a newly discovered outcome into a direct child rather
than widening the slice; (4) provide an orientation packet such as `braintree node
NODE --with-references` that loads the node, its route, and its linked
reconnaissance in one command, since the fixed per-session orientation cost is
the dominant term for a fast model.

## Finding 6 — no batch decomposition primitive, so re-scoping is a two-pass chore

Attempted: re-scoped the Phase 2 Increment 2 tree into 27 small nodes (3 `THO`
reconnaissance plus 24 task children) using `braintree node record`, one
invocation per node, then rewrote each parent's `next` to point at its first
child.
Friction: every node is a separate command carrying a long `--body`, `--route`,
and `--next`; the children's allocated ids are only known after creation, so
advancing each parent's `next` and adding its `# Slices` list is necessarily a
second pass. There is no way to express "these are the ordered children of X"
in one place, so a large, deliberate decomposition — exactly what the skill now
encourages — is slow and error-prone to author, and a mid-script failure leaves a
half-built tree.
Improvement: add a batch or cascade primitive such as `braintree decompose
PARENT --from PLAN.md` (or `braintree node record --parent X --children FILE`)
that allocates ids, writes the ordered children, and advances the parent's `next`
to the first child in one transaction; a `braintree node next X CHILD` shorthand
for the parent-advance step alone would also remove the second pass.
