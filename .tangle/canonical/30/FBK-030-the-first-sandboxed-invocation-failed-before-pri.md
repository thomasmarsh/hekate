---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: The first sandboxed invocation failed before printing help: error: Failed to initialize...
tangle_revision: 0.6.0+g169bad5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: From /Users/thomasmarsh/git/hekate, ran the mandatory read-only command: tangle help authoring.
Friction: The first sandboxed invocation failed before printing help: error: Failed to initialize cache at /Users/thomasmarsh/.cache/uv; failed to open /Users/thomasmarsh/.cache/uv/sdists-v9/.git: Operation not permitted (os error 1). A read-only help query unexpectedly required write-capable access outside the repository and had to be rerun with elevated sandbox permission.
Improvement: Package the installed tangle entry point so read-only help/check/query commands do not initialize a user-home uv cache, or direct its disposable cache to a repository-safe or temporary location. Document any unavoidable cache access in SKILL.md so agents can request the narrow permission before the first command.
