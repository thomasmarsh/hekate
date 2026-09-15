---
context_rev: 1
priority: P0
status: proposed
updated: 2026-09-15T16:09:50Z
summary: Eradicate all pre-rename tangle naming from the working tree.
next: Remove the remaining libtangle_* target artifacts and add a checked-in guard that fails the repo if the old tangle-* crate names return.
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
