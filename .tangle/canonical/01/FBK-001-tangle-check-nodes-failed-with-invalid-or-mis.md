---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: A proposed node cannot record a dependency on an unresolved predecessor: the checker demands a pin that pinning rules would themselves reject.
tangle_revision: 0.5.0+gc99a080
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Author Phase 1 Increment 2 as a proposed node gated on the still-active Increment 1 node (TAS-026) and record that dependency in # Context.
Friction: tangle check nodes failed with "invalid or missing context_rev pin for [[TAS-026-phase-1-increment-1-general-scenario-foundation]]" as soon as I wrote a Depends on edge to TAS-026 without a pin. Adding the pin (at context_rev 1.) points at an unresolved active target, which the dependency-pin check also rejects. The skill says to pin a resolved predecessor "when this increment takes the frontier" but never says how, or whether, to record a gated dependency edge before that, and the checker has no representation for a dependency that is known but not yet resolvable.
Improvement: Say explicitly in SKILL.md that a `Depends on` edge must not be authored until its target is resolved (route a gated node via Parent/Area or prose until frontier time), and/or have `tangle check` report a missing pin on an unresolved target with a diagnostic that names that sanctioned form instead of the generic "invalid or missing context_rev pin".
