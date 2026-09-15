---
context_rev: 1
status: resolved
updated: 2026-09-15T16:10:58Z
summary: Prune the 25 GB target/ directory and document a housekeeping recipe.
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

# Result

`target/` went from 35 GB to 17 GB; the stale pre-rename `tangle*` and
`libtangle*` artifacts, the incremental cache, and rustdoc output were removed.

- `scripts/clean-target.sh` (linked from `docs/dev-loop.md`) prunes
  `target/**/tangle*`, `target/**/libtangle*`, `target/debug/incremental`, and
  `target/doc`, with a `--dry-run` mode, honoring `CARGO_TARGET_DIR`, and
  descending only from the target directory.
- Measured: initial prune 35 GB -> 18 GB (about 17 GiB); the follow-up matcher
  fix removed 312 `libtangle_*` files (408.9 MiB), 18 GB -> 17 GB.
- Verification: `cargo check --workspace --all-features` clean and
  `cargo nextest run -p hekate-model --all-features` 172/172 after each prune.
- Evidence: commits `bbf4887`, `b384748`; `/tmp/tas137/after-c.md`,
  `/tmp/tas137/after-c-fix.md`.
- P0 sibling `tas-1rxewbg308k2sfp50pp1njhmyn` owns the remaining naming guard.
