---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: The failed status and the built-in revive-first guidance imply unfinished work, but the worker h
tangle_revision: 0.5.0+g974b178
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Ran Phase 1 Increment 4 (TAS-030) as one coordinating node edited by six serial workers. The slice-D worker ran the full 1800000 ms budget and the run was reported failed with "Subagent timed out after 1800000ms".
Friction: The failed status and the built-in revive-first guidance imply unfinished work, but the worker had already committed its slice, updated the node, run all five gates, and released its claim; it timed out only while composing the final prose report. The coordinator had to open the run log and search it to reconstruct a handoff that was already complete, and the only durable completion signal (result: released at the recorded base hash) was buried near the end of the log. A report-time timeout is indistinguishable from an implementation-time timeout in the run status.
Improvement: Have workers emit a compact structured completion receipt (base hash, release result, five-gate summary, commit SHAs) before the long narrative report, or have the harness classify a timed-out run whose claim was released as completed-with-report-truncated rather than failed. Document in SKILL.md that the release result at the recorded base hash is the completion signal the coordinator trusts over the run status when a worker times out during reporting.
