---
context_rev: 1
status: proposed
updated: 2026-09-15T12:46:40Z
summary: Increment 2 frictions: circular gate, filesystem search, dead lane, TAS-110, gate skip.
tangle_revision: 1.0.0+g11f5042
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Closed Phase 2 Increment 2 (TAS-020) as a serial fresh-worker orchestration with a per-leaf implementation stage and a per-leaf gate stage, then recorded the session friction in the ledger roundup.
Friction: Five frictions. (1) A circular gate between TAS-106 and TAS-133 forced the gate worker to be pre-adjudicated repeatedly about a Done-when clause owned by another branch, and the brief delegated a coordinating parent's resolution to that gate worker although SKILL.md reserves resolution authority to the coordinator. (2) Repeatedly, a fresh worker began with `find /` or `grep -r ... .` and scanned the whole filesystem or the 28 GB `target/` directory until it was killed on budget, because AGENTS.md's "load the Tangle skill (SKILL.md)" instruction gives no path. (3) One worker hit `deepseek-v4-pro` `Insufficient Balance` mid-session and its partial uncommitted work had to be salvaged by a fresh worker. (4) TAS-110 needed three sessions (two timeouts) because its Done-when bundled workload authoring, a long release benchmark, sim instrumentation, and presenter timing. (5) A workflow-script interpolation bug (calling `.slice().join()` on an already-joined string) silently skipped a gate stage twice.
Improvement: See the findings below. Name the installed skill path in AGENTS.md (or add a `tangle skill` verb) so orientation never starts with a filesystem search; state the circular-gate rule and that a coordinating parent's resolution is never a gate worker's write set; treat a lane quota failure like a worker timeout; split a measurement Done-when by its own wall-clock cost; and make each workflow gate stage assert its own completion so a bad interpolation is a hard error.

## Finding 1 — a circular gate, and resolution authority delegated to a gate worker

Attempted: gated the Increment 2 fixture leaves, including TAS-106
(`check in increment 2 passing fixtures`) and TAS-133 (`reproduce every
increment 2 fixture and golden its transitions`), and briefed one gate worker
for the fixture and golden stage.

Friction: TAS-106's `# Done when` clause requiring every fixture at the
benchmark matrix presets could only be delivered by TAS-133, and TAS-133 was
`Gated on` TAS-106, so each node's Done-when was owned by the other branch and
the gate worker had to be pre-adjudicated, repeatedly, on which node owned the
clause. `tangle check` validates graph structure only, so a requirement cycle
written as an output dependency is invisible to it, and SKILL.md's "record an
unresolved target as a gate" gives no shape for a mutual requirement. In the
same stage the brief delegated resolving a coordinating parent to the gate
worker, although SKILL.md reserves resolution authority to the coordinator.
That resolution-authority gap is already recorded in `FBK-006` Finding 1, and
the adjacent "a Done-when clause owned by another node" gap in
`fbk-1aw3mq1hev266ekb5c9zr3wkgy`, so both are cross-referenced here rather than
re-filed; what is new is the cycle itself.

Improvement: state in the dependencies reference that a bidirectional
requirement is authored in one direction with the requirement linked from the
body, and that a `Gated on` target may legitimately name a sibling's
deliverable; add a `tangle check` lint for a requirement cycle expressed as an
output dependency; and state in the coordinator's brief template that resolving
a coordinating parent is never a gate worker's write set (only the coordinator
performs it, or the brief names the parent explicitly in the write set).

## Finding 2 — workers searched the whole filesystem for SKILL.md

Attempted: dispatched fresh workers with the standard orientation instruction
from AGENTS.md.

Friction: AGENTS.md says to "load the full Tangle skill (`SKILL.md`;
`/skill:tangle` in pi)" but never gives the installed path, so several workers
began with `find /` or `grep -r ... .` and scanned the whole filesystem or the
28 GB `target/` directory until they were killed on budget. The instruction is
the first thing every session must satisfy, so its cost is paid before any
orientation, and the brief that carried it could not correct it.

Improvement: name the installed path in AGENTS.md, for example
`~/.pi/agent/skills/tangle/SKILL.md`, or add a `tangle skill` verb that prints
it, exactly as AGENTS.md already names the installed `tangle` entry point. State
the resolved path in every worker brief and state the negative rule beside it:
never search `.` or `/` recursively, and locate a node with
`find .tangle -name '<ID>-*'`.

## Finding 3 — a model lane failed mid-session and its partial work had to be salvaged

Attempted: dispatched one Increment 2 leaf to the `deepseek-v4-pro` lane.

Friction: the lane returned `Insufficient Balance` mid-session, after the worker
had written uncommitted edits and before it recorded a `# Result` or committed.
A provider account state is not a timeout and no node in the vault owns it, so
the worker died at the frontier with a partial diff and a fresh worker had to
inspect and salvage it, paying the same recovery cost as a timed-out worker.

Improvement: treat a mid-session provider failure like a timed-out worker
(inspect the partial diff, run the touched crate's tests, then accept, finish,
or revert) and say so explicitly in the coordination reference; add a lane's
account balance to the pre-dispatch checks in the worker handoff template so a
known-dead lane is not dispatched to at all.

## Finding 4 — TAS-110 needed three sessions because its Done-when bundled a measurement

Attempted: recorded the Increment 2 representative mixed-mode profile as one
node (TAS-110).

Friction: the Done-when bundled four independently acceptable outcomes — author
the workload fixture, run the workspace release benchmark sweep, instrument the
sim counters, and time the presenter — and two are dominated by their own
measurement: the release sweep is about 506 s and each ablation row is three
timed passes. The node therefore took three sessions and two 30-minute
timeouts. `FBK-032` Finding 1 and `FBK-033` Finding (d) already record the
general bundling rule; the part they do not cover is that a measurement
deliverable's acceptance run is itself a multi-minute cost that slice sizing by
"one verifiable change" does not account for.

Improvement: a `# Done when` that names a release-mode benchmark or a profiled
capture should name that run's wall-clock cost, and the coordinator should make
the long run its own slice (author the artifact and its harness in one session,
execute the sweep in another), so a measurement node is never dispatched as a
single session.

## Finding 5 — a workflow-script interpolation bug silently skipped a gate stage

Attempted: ran the session's workflow script, which interpolated each gate
stage's command from the step list.

Friction: the script called `.slice().join()` on a value that was already a
joined string, so the interpolation produced one wrong token instead of failing
and the gate stage did not run — twice — with no error and no output. The skip
was noticed only from the absence of the stage's expected output line, because
nothing asserted that a dispatched stage had run.

Improvement: have each gate stage assert its own completion and print a unique
sentinel, and have the runner fail when a dispatched stage reports no result, so
an interpolation bug becomes a hard error instead of a silent skip; prefer
passing a stage's command as one value rather than re-deriving it by string
surgery.
