---
context_rev: 1
status: proposed
updated: 2026-09-15T22:15:46Z
summary: Segment-level broad/narrow-phase collision for articulated chains.
next: Extend broad-phase indexing and narrow-phase contact reporting to resolve the actual contacting segment pair for an ArticulatedWheeled agent, not one proxy shape for the whole chain.
---

Parent [[tas-5vc5c3cbttnvcztafvns26bcs4-add-hitch-integration-runtime-dispatch-and]].

# Context

Depends on the segment-pose chain the hitch-integration slice adds to the per-agent runtime state; today collision and broad-phase code (`crates/hekate-sim/src/swept.rs` and its broad-phase caller) assume one shape per agent.

# Outcome

Broad phase indexes one proxy per chain or one per segment for an `ArticulatedWheeled` agent; narrow phase reports the specific contacting segment pair (agent, segment index) rather than the whole chain, so a contact event names which segment touched.

# Done when

- Broad phase never misses a contact a single enclosing proxy would have caught, and narrow phase resolves the specific segment pair for an articulated-chain contact.
- A fixture/test proves a contact is detected on a non-lead segment (e.g. the trailer, not the tractor) that a single-proxy check would miss or misreport.
- Existing box/circle/capsule collision fixtures and event shapes are unchanged.
- The five gates pass.
