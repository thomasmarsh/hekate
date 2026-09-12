---
context_rev: 1
updated: 2026-09-12T12:36:52Z
summary: Postmortem of Braintree friction while planning the multi-backend renderer epic.
---

# Scope

Session: planning and recording the renderer-backend epic
([[TAS-009-renderer-backends]]) with `bt`, `graph-check`, and the Braintree
skill on 2026-09-12. This node captures tooling and skill friction for later
meta-analysis. It does not fix the skill.

# Findings

## F1. ID allocation never reconciles with Markdown, so it hands out existing identities

`bt allocate` numbers come only from the sidecar's sequence counter. Neither
`bt init` nor `bt reindex nodes` seeds or repairs that counter from the
Markdown maxima, and `bt allocate` does not check whether the returned identity
already exists on disk.

Evidence, fresh sidecar with `nodes/resolved/TAS-001..006` and `DEC-001` on
disk: `bt allocate TAS` returned `TAS-001`, colliding with an existing node.
The decisive case is `IDX`: after `bt init` plus a `bt reindex nodes` run that
reported the whole edge set, `bt allocate IDX` still returned `IDX-001`, which
is `nodes/resolved/IDX-001-tangle.md`. Reindex had not moved the sequence.

The earlier apparently correct results (`TAS-009`, `THO-002`, `DEF-002`,
`DEC-002`) were not reconciliation: those counters were only past the on-disk
maxima because the collision in the first sentence had already advanced them and
then the IDs were burned. `IDX` had never been allocated, so it exposed the bug.

Severity: high. The recommended `bt init` then `bt reindex` recovery in SKILL.md
describes disaster recovery, not first-run setup, so a worker can reasonably
init, reindex, allocate, and create a duplicate identity.

Proposed change: seed and verify the sequence from Markdown in `bt init` or
`bt reindex`, or make `bt allocate` refuse an identity that already exists,
or print an explicit "run `bt reindex` before `bt allocate`" step that actually
takes effect.

## F2. Reserved identities leave no audit trail and cannot be reclaimed

After F1, `TAS-001..008`, `THO-001`, `DEF-001`, and a phantom `IDX-001` were
reserved by the sidecar with no files ever created, and `IDX-002` was allocated
to route around the collision. Nothing lists reservations, `bt reindex` does not
reclaim them, and `bt status` does not surface them, so the vault now has ID
gaps and a sidecar holding phantom reservations.

Proposed change: expose reservations in `bt status`, or add a reconcile/return
command, or at minimum have `bt allocate` report the previous high-water mark and
what it is departing from.

## F3. `bt backlinks` accepts only the bare ID and fails silently

`bt backlinks TAS-009-renderer-backends` printed `backlinks: 0 matching edges`
even though nine nodes linked it. `bt backlinks TAS-009` printed all nine.

Proposed change: accept the node name, or error on an unknown node, or document
the bare-ID argument in SKILL.md. A silent zero is the worst failure mode
because it reads as "no dependents".

## F4. Dependency pins must terminate the line, but the diagnostic blames the pin

The line `Depends on [[TAS-006-bevy-viewer-skeleton]] at context_rev 1. That
node left the only presentation path...` produced `invalid or missing
context_rev pin for [[TAS-006-bevy-viewer-skeleton]]`. The checker full-matches
the text after the link, so trailing prose invalidates a syntactically correct
pin. SKILL.md shows the pin text but not that nothing may follow it on the line.

Proposed change: state the line-termination rule in SKILL.md and make the
diagnostic name the trailing text, for example "pin must be the last text on the
line".

## F5. The `next` grammar lives only in the checker

Rules recovered from reading `graph_check.py`, not from SKILL.md: `next` may
contain at most one wikilink; that link must resolve to a direct child whose
primary `Parent` or `Area` is this node; and a resolving `next` may point at a
knowledge node. SKILL.md says "one concrete frontier action, or one wikilinked
direct child" without defining "direct child" operationally.

Proposed change: document the accepted `next` forms and the direct-child test.

## F6. `blocked` versus a planned dependency is ambiguous on an unstarted tree

`TAS-013-terminal-viewer-kitty` is gated on a decision node that is itself part
of the same proposed plan. It is unclear whether that is `blocked` (missing
input) or merely `proposed` (dependency). This session chose `proposed` because
the gate is an internal planned step, not missing external state, but the
SKILL.md boundary does not settle it.

Proposed change: add a SKILL.md example contrasting a proposed node with a
dependency against a genuinely blocked node.

## F7. "Decompose just in time" conflicts with a request for a complete plan

SKILL.md says to decompose just in time and not to pre-create speculative trees,
but the user asked for the complete node set up front. This session recorded one
proposed epic with eight children as an explicitly requested plan.

Proposed change: guidance distinguishing a user-requested proposed tree
(acceptable, with resolve-or-dispose as reality arrives) from speculative
decomposition the skill discourages.

## F8. Minor output and documentation ambiguities

- `bt stale` prints `stale: 0 dependency pins`, which reads as either zero stale
  pins or zero pins total.
- SKILL.md's status list includes `blocked/`, but this vault had no `blocked/`
  directory and nothing creates or validates the directory set.
- `graph-check` counted `18 nodes` for 18 node files while `bt reindex` also
  reported `nodes: 18`, so the counts agreed; this is a note, not a defect.

# What worked

- `graph-check` rejected real errors with exact file paths and messages.
- Once the graph was valid, canonical `Parent` and `Area` edges produced a
  correct derived backlink set.
- The TOON reporting convention was easy to follow.
- Routing the epic through `Area` and children through `Parent` made
  reachability and the frontier unambiguous.

# Suggested meta-analysis axes

For later aggregation: category (allocation, checker, output, documentation),
severity, reproduction count, and whether SKILL.md or the tooling should change.
F1 is the only high-severity, reproducible finding; F2 and F3 are medium because
they mislead a worker; F4 through F8 are low-cost documentation gaps.

# Result

Eight findings captured with evidence and proposed changes. This node is
finished; meta-analysis and any Braintree changes are separate work.

Area [[IDX-002-braintree-feedback]].
