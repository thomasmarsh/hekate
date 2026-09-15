---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Exclusive write set can be narrower than the compile-and-golden closure of an approved change
tangle_revision: 0.5.0+gf9b330c
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Executed slice C of TAS-030 under the handed-off exclusive write set, which named crates/hekate-sim/{src,tests}, crates/hekate-present only for a compile fix, and the assigned node.
Friction: Two parts of this repo defeat a crate-local write set for a deliberate event-record change. (1) The record shape is re-declared in apps/hekate-cli/src/trace.rs, whose EventRecord match is exhaustive and whose spawned arm does not use `..`, so adding an Event variant or an Event::Spawned field cannot compile without editing apps/hekate-cli; appending a variant also breaks exhaustive matches in apps/hekate-tui/src/session.rs, apps/hekate-viewer/src/main.rs, and apps/hekate-cli/tests/scenarios.rs. (2) Regenerating an invalidated golden requires running the CLI, and the checked-in goldens (tests/golden/walking_guide_v1.trace.{jsonl,sha256}) and baselines/phase1/baseline.json live outside every crate directory. The handoff said only "report additions" and the skill offers no rule for what to do when the compile-and-golden closure of an approved change exceeds the assigned set, so the worker had to widen the set itself and report it, which is exactly the judgment call the exclusive-write-set contract exists to keep with the coordinator.
Improvement: State in the skill what a worker does when the closure of an approved change exceeds its write set: either the handoff must name the closure up front (grep for exhaustive matches on the changed type, plus the golden and baseline paths) or the worker must stop and ask. A concrete rule would be: "the write set is the compile-and-golden closure, not the crate: any file the change must touch is part of it, and a worker reports the additions in the handoff rather than pausing, unless an addition is in another node or a shared hub."
