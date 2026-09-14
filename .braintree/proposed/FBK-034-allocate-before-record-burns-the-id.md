---
context_rev: 1
updated: 2026-09-14T22:54:51Z
summary: `allocate` before `record` burns the id; `node record` adds a default route.
braintree_revision: 0.6.0+gef4b49d
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Discover the next free DEF id, then record a DEF node, before authoring a metric-definition node: ran `braintree allocate DEF` (returned DEF-006), then `braintree node record --type DEF --body <whose first line is the authored Parent route>`.
Friction: 1) `braintree allocate DEF` permanently burned the DEF-006 id it returned, so the next `braintree node record --type DEF` would have allocated DEF-007; the author had to pass `--id DEF-006` to reuse the intended id, and `braintree reservations` still lists 6 as burned. There is no read-only id preview and no `record --dry-run`, and the skill only warns that a *discarded* allocate burns an id, not that using allocate to discover the next id does. 2) `braintree node record` with no `--route` silently wrote a default `Area [[IDX-001-tangle]]` route line above the body's authored `Parent [[TAS-101-...]]` line, giving the node two primary routes; the skill requires exactly one primary `Parent`/`Area` and `braintree check` passed only after the line was deleted by hand. `braintree node record --help` does not mention the inserted default route and no warning was emitted.
Improvement: 1) Let `braintree node record` un-burn and accept a reserved-but-unwritten id, or add a non-burning id preview / `--dry-run`; and document in the skill that running `allocate` before `record` burns the id. 2) Either omit the default `Area` route when the supplied body already contains a primary `Parent`/`Area` link, or emit a warning and require an explicit `--route`; document the default-route behavior in `braintree node record --help`.
