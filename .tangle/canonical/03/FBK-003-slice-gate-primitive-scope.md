---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Assigned-slice scope rule is unclear when a gate needs a primitive the write set lacks
tangle_revision: 0.5.0+g64359e6
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Executed the assigned slice B node TAS-027, whose Done-when and brief required cars to come to rest at an "authored stop line" and whose gate required profile-bounded accelerations without body overlap, while editing only crates/hekate-sim and crates/hekate-model.
Friction: The assigned node implied two primitives that the compiled model did not provide: an authored stop line (no MovementSource or SignalHead field existed) and a spawn admission compatible with bounded IDM braking (the landed clearance-only admission inserted full-speed vehicles close behind slow leaders, so the comfortable-brake bound could not hold). Neither the SKILL.md write-set/slice guidance nor the node text said whether supplying a primitive or seam a slice gate needs is in-scope authoring for the worker or requires a new node or an escalation, so the worker stopped mid-slice twice.
Improvement: Add a short SKILL.md rule for continuing an assigned slice: when a gate or Done-when criterion requires a minimal primitive or seam the current write set does not yet contain, the worker may author that minimal piece inside its declared write set and record it in the node Result, and must escalate only when the change alters a landed seam another node owns (for example another slice's admission contract) or the public schema contract. Have coordinator task briefs name any primitive a slice must introduce so the worker does not have to infer it.
