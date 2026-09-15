---
context_rev: 1
status: proposed
updated: 2026-09-15T14:20:37Z
summary: Prune the 25 GB target/ directory and document a housekeeping recipe.
next: Measure the target/ size breakdown, prune safely regenerable artifacts, and write the housekeeping recipe.
---

Parent [[TAS-137-cut-the-rust-dev-loop-and-workspace-test-wall]].

# Outcome

`target/` no longer holds tens of gigabytes of stale build output, and the repo
documents how to keep it small without forcing a full cold rebuild.

# Done when

- Target-directory size is measured before and after the prune.
- Only safely regenerable artifacts are removed; the prune does not delete
source, checked-in goldens, or caches the workspace cannot rebuild.
- A housekeeping recipe (doc or script) records what to prune, the exact
commands, expected space reclaimed, and the rebuild cost to expect.
- The recipe is linked from the repo's dev-loop documentation.

# Context

TAS-137 recorded `target/` at 25 GB; the working tree measured 34 GB on
2026-09-15. `[profile.dev] debug = 0` already limits debug info.
