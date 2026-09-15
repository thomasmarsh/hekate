---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: The skill's status rules say to move the unchanged filename between directories and the coordina
tangle_revision: 0.5.0+g7b95875
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Resolve an active node in one commit: edit the body (add # Resolution, remove next), then move it from nodes/active/ to nodes/resolved/ with 'git mv', then commit with the Closes footer.
Friction: The skill's status rules say to move the unchanged filename between directories and the coordination reference says to keep the node's content update and its status move coherent in one commit, but neither mentions that 'git mv' stages only the rename and can leave the body edits unstaged. I edited nodes/active/TAS-037-*.md, ran 'git mv nodes/active/... nodes/resolved/...', and committed. The commit recorded the rename with the OLD body: 'git show HEAD:nodes/resolved/TAS-037-*.md | grep -c "^# Resolution"' printed 0 while the working tree printed 1, and 'git status --short' showed ' M nodes/resolved/TAS-037-*.md'. The resolution commit therefore passed 'tangle check nodes' while missing the Resolution evidence; I had to 'git add' and amend. The skill's statusing example ('mv') hides the git-level staging trap.
Improvement: In SKILL.md's status/mutation rules, next to the 'move the unchanged filename' instruction, note that a status move and any body edit must be staged together (e.g. 'git add' the destination after 'git mv', or run 'git mv' as the final step) and that a resolving commit should be verified with a grep for the new section before handoff; optionally have 'tangle verifiers' flag a resolved node whose body lacks a Resolution/Result section.
