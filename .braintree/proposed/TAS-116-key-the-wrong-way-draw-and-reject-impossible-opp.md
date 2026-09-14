---
context_rev: 1
updated: 2026-09-14T13:25:49Z
summary: Key the wrong-way draw and reject impossible opposing options.
next: Key any wrong-way draw to the versioned maneuver stream and reject a physically disconnected option before any claim or motion.
---

Parent [[TAS-097-make-contextual-wrong-way-decisions-reproducible]].

# Outcome

The wrong-way decision is reproducible and safe by construction: any random draw
is keyed to the versioned maneuver stream, and a physically impossible opposing
option is rejected before any claim or motion.

# Done when

- Random choice, if required, is keyed by root seed, run ID, stable AgentId, and
  the versioned maneuver stream; draw order and unrelated agents cannot change
  it.
- A physically disconnected opposing option yields an explicit rejection before
  any claim or motion; a legal prohibition stays available to a non-compliance
  decision.
- Fixed-seed, reversed-declaration-order, and unrelated-agent isolation tests
  pass.

# Context

Extends [[TAS-097-make-contextual-wrong-way-decisions-reproducible]]; reads
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the draw
keying and the impossible-option rejection; do not add motion or routing.
