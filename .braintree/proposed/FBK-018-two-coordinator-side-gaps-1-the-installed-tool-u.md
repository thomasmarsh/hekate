---
context_rev: 1
updated: 2026-09-13T03:22:32Z
summary: Two coordinator-side gaps. (1) The installed tool upgraded mid-session (0.5.0+g974b178 to 0.6.0+
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Coordinated Phase 1 Increment 5 as one fresh worker per slice, starting under Braintree 0.5.0 with a tracked nodes/ vault; mid-session the installed tool became 0.6.0 and the vault contract moved to .braintree/.
Friction: Two coordinator-side gaps. (1) The installed tool upgraded mid-session (0.5.0+g974b178 to 0.6.0+g4c6cafb) and its default resolver relocated the tracked nodes/ vault to .braintree/; the worker briefs already issued named nodes/... paths, so their exclusive write sets silently became invalid, one worker looped trying to revert the move, and nothing surfaced the version/path change to the coordinator. (2) `braintree allocate DEF` returned DEF-004 although DEF-003 is not written anywhere in the vault, so the allocated id is not the contiguous next id and the node list has a permanent gap; nothing says whether an unwritten allocation is intentionally burned or was orphaned.
Improvement: Print a one-line notice when the resolved vault directory differs from the directory a command was invoked against, and have coordinator/worker handoffs carry the installed tool version so a mid-session upgrade is visible; document explicitly that allocate burns an id permanently when the caller discards it, and let a coordinator release an unused allocation so a fresh-sidecar gap can be reclaimed.
