---
context_rev: 1
status: proposed
updated: 2026-09-15T22:15:46Z
summary: Swept collision queries for multi-segment articulated bodies.
next: Extend the swept-query path (crates/hekate-sim/src/swept.rs) to sweep every segment of an articulated chain along a curved reference.
---

Parent [[tas-5vc5c3cbttnvcztafvns26bcs4-add-hitch-integration-runtime-dispatch-and]].

# Context

`time_of_impact`/`band_entry` in `crates/hekate-sim/src/swept.rs` operate on one `SweptBody` per agent; an articulated chain needs a swept query per segment so a contact is detected even when no endpoint overlaps and the body is turning.

# Outcome

A swept query along a curved reference detects a contact for any segment of an `ArticulatedWheeled` agent, not only its lead segment.

# Done when

- A swept test on a curved reference with a multi-segment body detects a contact that an endpoint-only or lead-segment-only check would miss.
- Existing single-body swept fixtures are unchanged.
- The five gates pass.
