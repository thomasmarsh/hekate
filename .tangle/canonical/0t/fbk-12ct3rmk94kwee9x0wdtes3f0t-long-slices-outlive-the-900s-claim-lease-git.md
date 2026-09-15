---
context_rev: 1
status: proposed
updated: 2026-09-15T19:50:59Z
summary: Long slices outlive the 900s claim lease; git add -N leaves empty-blob index entries.
tangle_revision: 1.0.0+ga6de4e4
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Coordinated a ~25-minute orchestrated slice: the implementer hashed and claimed tas-3ww2hmcr24ttka1g8py3tewgg3 under the default lease, edited and gated the change, then released. The build agent surfaced untracked node files for review with `git add -N .`, and the commit step staged them with `git add -A`.
Friction: Two coordination sharp edges. (1) `tangle claim` defaults to a 900 s lease; a ~25-minute slice outlives it and the worker release returned `expired`, not `released`, so the receipt could not use release as the trusted completion signal. Renewal is documented only inside the coordination reference, and the slice loop never tells the worker to renew or re-claim after each build/fix round. (2) `docs/orchestrated-sessions.md` tells the build agent to surface untracked files with `git add -N .`; intent-to-add entries leave an empty blob in the index, so a plain `git commit` without `git add -A` would land an empty node file. The doc says the commit must `git add` new files but does not name the empty-blob failure mode.
Improvement: State in `tangle help coordination` (or the read-and-execute loop) that a slice expected to exceed 900 s must renew its lease after each build/fix round (`tangle claim NODE AGENT --base-hash`), or raise the default. Add to `docs/orchestrated-sessions.md` known pitfalls: `git add -N` leaves intent-to-add entries whose blobs are empty, so the commit step must `git add -A` (or `git add` each new path) or it commits empty files.
