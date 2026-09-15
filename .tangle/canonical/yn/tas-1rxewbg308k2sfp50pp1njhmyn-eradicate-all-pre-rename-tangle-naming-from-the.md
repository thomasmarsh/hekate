---
context_rev: 1
priority: P0
status: resolved
updated: 2026-09-15T16:17:33Z
summary: Eradicate all pre-rename tangle naming from the working tree.
---

Area [[IDX-001-hekate]].

# Outcome

No pre-rename `tangle` naming remains anywhere in the working tree except the
intentionally named Tangle work-ledger and the two files that document the
reclaimer. The stale `libtangle_*` build artifacts are gone, cannot reappear,
and a checked-in guard fails the repo if the old crate names return.

# Done when

- `find target -name 'tangle*' -o -name 'libtangle*'` is empty.
- Every remaining `tangle` occurrence in tracked files is recorded in an audit as
  intentional (the Tangle work-ledger, `tangle_revision`, or the reclaimer prose
  in `docs/dev-loop.md` / `scripts/clean-target.sh`) rather than a pre-rename
  artifact.
- No manifest, source identifier, script, or binary name uses `tangle-*`.
- `scripts/clean-target.sh` removes both `tangle*` and `libtangle*`, and a
  checked-in guard prevents the old crate names from returning in a manifest,
  source identifier, or generated artifact name.
- `tangle check` passes and the workspace gate is green.

# Context

`target/` held 312 stale `libtangle_*` rlibs/rmeta/dep-info files (~409 MiB)
from before commit `e9a418d refactor(hekate)!: rename project to hekate and
adopt the tangle ledger`. A precise audit of tracked files (excluding `.tangle/`)
found no old `tangle-*` crate name in any manifest or source; the only
occurrences are the Tangle work-ledger itself, `tangle_revision` in `AGENTS.md`,
and the two reclaimer files that name the patterns deliberately. The `lib` prefix
is why a `-name 'tangle*'` glob missed them: Rust writes
`libtangle_model-*.rlib`, not `tangle_model-*`.

Sibling TAS-137 slice C prunes `target/` and adds `scripts/clean-target.sh`; its
follow-up extends the matcher to `libtangle*`.

Out of scope unless confirmed: the project's work-ledger is itself named
"Tangle" (`.tangle/`, the `tangle` command, `tangle_revision`). That is
intentional and is not a pre-rename artifact.

# Result

The pre-rename artifacts are gone and a guard prevents their return.

- `target/` has zero `tangle*`/`libtangle*` matches. TAS-137 slice C removed
  312 `libtangle_*` files (408.9 MiB) plus the earlier `tangle*` and incremental
  artifacts; the recipe is `scripts/clean-target.sh` (35 GB -> 17 GB).
- `scripts/check-no-legacy-tangle.sh` (commit `0c64445`) fails on any tracked
  `tangle-<crate>`, `tangle_<crate>` identifier, or `libtangle` artifact name,
  ignores the word "rectangle" and the intentional ledger spellings, and ships a
  `--self-test` that rejects 13 forbidden tokens and ignores 6 allowed ones. CI
  runs it next to the dependency-direction check.
- Audit of remaining tracked `tangle` occurrences: all are intentional. The
  ledger (`AGENTS.md`, `README.md`, `.gitignore`, `baselines/phase1/entry-gate.md`,
  `perf/tick-phases.json`, `scripts/measure-tick-phases.sh`,
  `spikes/kitty-graphics-poc/src/main.rs`, all `.tangle/**`) uses "Tangle" as the
  work-ledger name; `scripts/clean-target.sh` and `docs/dev-loop.md` name the
  patterns only to remove them; `scripts/check-no-legacy-tangle.sh` names every
  pattern as its rule. Every other `tangle` substring in the tree is inside the
  word "rectangle".
- No Rust or manifest changed since the last green workspace gate
  (`b384748`; 1118 passed / 0 failed / 3 skipped), so it remains green; the guard
  is shell/docs/CI only. `tangle check` passes.

Recorded residuals (P2, not blocking): the guard enumerates the six known old
crate names, so a brand-new `tangle-<other>` would need adding; the two reclaimer
files are excluded wholesale; the self-test exercises the boundary only for the
underscore family. Evidence: commits `bbf4887`, `b384748`, `0c64445`;
`/tmp/tas137/p0-verify.md`.
