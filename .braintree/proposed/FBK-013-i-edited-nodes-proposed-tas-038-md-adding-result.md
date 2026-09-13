---
context_rev: 1
updated: 2026-09-13T01:37:36Z
summary: I edited nodes/proposed/TAS-038-*.md (adding # Result), then ran 'git mv nodes/proposed/TAS-038-
braintree_revision: 0.5.0+g7b95875
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Claim TAS-038, move the node nodes/proposed/ -> nodes/active/ as part of the claimed edit, and record the base hash and write set in its # Result, then commit the code and the node separately.
Friction: I edited nodes/proposed/TAS-038-*.md (adding # Result), then ran 'git mv nodes/proposed/TAS-038-*.md nodes/active/TAS-038-*.md', then staged only the four code files and committed the implementation. 'git status --porcelain' showed 'RM nodes/proposed/... -> nodes/active/...': git mv had staged the rename carrying the OLD blob, so the implementation commit recorded the moved node with its pre-edit body ('git show HEAD:nodes/active/TAS-038-*.md | grep -c "Landed in one session"' printed 0) while my # Result edit stayed in the worktree. This is FBK-012's finding ('git mv stages only the rename and can leave the body edits unstaged'), and knowing that advice did not prevent it: the danger is the two-commit claim/resolution split, where the status move lands in whichever commit runs first and its body edit is staged in a later one, so a reviewer reading the implementation commit sees a status change with no record. I recovered by staging the node explicitly in the next commit, but the intermediate commit is misleading.
Improvement: In SKILL.md's mutation rules, next to 'move the unchanged filename between status directories', state that the status move and the # Result/# Resolution body edit belong in the SAME commit, that 'git mv' may stage the pre-edit blob so the destination must be 'git add'-ed after the move, and that the move should be performed as the last step before committing that node's body; optionally have 'braintree check nodes' or 'braintree verifiers' warn when an active/resolved node's most recent commit is a pure rename, since that commit carries no body change.
