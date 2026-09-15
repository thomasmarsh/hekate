# Orchestrated sessions

When the user asks to run an **orchestrated**, **resumable**, **multi-slice**, or
**subagent-driven** session, the main session acts as supervisor and routes the
work to short-lived children. This file is the protocol. `AGENTS.md` only points
here; the Tangle skill remains the authority for the work ledger.

## Roles

| Role | Agent tier | Context | May write? | Delivers |
|---|---|---|---|---|
| Parent / supervisor | strong default | session | graph only | intent, routing, arbitration, acceptance, ledger |
| Scout | fast/recon | fresh | report only | compressed orientation |
| Implementer | worker | fresh per slice | one writer | one verifiable slice of edits |
| Build/test | delegate | fresh per build | no | build/test result + timings |
| Build warmup | delegate | fresh | no | a warm, compiled tree |
| Reviewer | reviewer | fresh | no | verdict + file:line findings |
| Commit | resume implementer | inherited | yes | the slice commit |

The parent keeps user intent, authority, routing, arbitration, and final
acceptance. It does not do routine work; direct parent edits are small,
intentional interventions (ledger edits, one-line corrections) with a brief
reason.

## Hand-off protocol

Every child brief contains: repo + working directory + ref; authority and
edit boundary; the objective as one verifiable change; the relevant
files/contracts/constraints; acceptance and exact validation commands; the
expected output; and stop/ask conditions.

The slice loop is fixed:

1. **Scout** — only when orientation is missing. Read-only; no builds or tests.
2. **Implementer** — edits the slice. It reports files changed and what to
   verify, and does **not** run the suite.
3. **Build/test agent** — runs the exact commands on the working tree and
   returns structured output (`{passed, timings, failures, summary}`). It never
   edits.
4. **Fix** — on failure, resume the *same* implementer with the build report,
   then dispatch a *fresh* build agent. Repeat until green.
5. **Reviewer** — fresh context; reads the diff and the changed files; returns
   `OK` / `OK with notes` / `BLOCK` with file:line findings.
6. **Commit** — resume the implementer to commit the reviewed tree, referencing
   the Tangle node in the body. Byte-compare the staged diff against the
   reviewed diff before committing.

Rules that make hand-offs consistent:

- **One writer per checkout at a time.** Scouts and reviewers are read-only.
- **The Cargo target directory and package cache are an exclusive resource.**
  Only one cargo-invoking agent at a time; even `cargo metadata` and
  `cargo nextest list` can block on the lock. Never run cargo in a scout while
  a build agent runs. Route build/measurement commands through the build agent,
  never through a scout.
- **Structured output is the contract.** Give build and review children an
  `outputSchema`; do not parse their prose.
- **The build agent owns builds; the implementer owns edits; the reviewer owns
  the verdict.** Do not blur these.
- **Every dispatched stage proves it ran.** Each gate stage asserts its own
  completion and prints a unique sentinel; the runner fails when a dispatched
  stage reports no result, so a script interpolation bug is a hard error rather
  than a silent skip. Pass a stage's command as one value; do not re-derive it
  by string surgery.
- Keep long output out of chat: the build agent writes a report file and
  returns a bounded summary plus its path.

## Efficiency

- Fresh implementer per slice; resume the same one for fixes so it keeps the
  slice's orientation instead of re-scouting.
- Fresh build agent per build; it needs no prior context, so give it a fixed
  command list plus the baseline numbers.
- Fresh reviewer per review.
- One orientation pass: do not dispatch duplicate scouts.
- Provision slow tools once in the parent (for example install
  `cargo-nextest`) before dispatch; do not let parallel agents race an install.
- Prefer `user` CPU over wall time when comparing test cost on a shared host.

### Build-warmup agent

A dedicated warmup child is worth it only when a compile dominates wall time
**and** its result is reused by later children (a shared warm `target/`), or
when the test agent's context or tool budget is tight and compiling would spend
it. Otherwise fold the warmup into the build agent: run `clippy` or
`--no-run` first, then the timed tests in the same child. In practice the
folded form (clippy warms, then the timed run) was sufficient for this repo; a
separate warmup child earns its round trip mainly when several subsequent test
children reuse one warm tree. Keep its brief minimal — compile, report version
and errors, nothing else.

## Known pitfalls

- **`git diff` blind spot.** `git diff` omits untracked files, so a reviewer
  that only sees the diff cannot see a new file and may `BLOCK` because the
  commit would omit it. Before review, the build agent must surface untracked
  files (`git add -N .` or an explicit list) in the evidence; a review must not
  `BLOCK` solely because new files are untracked when the commit step will
  stage them. `git add -N` records intent-to-add entries whose blobs are empty,
  so a plain `git commit` without `git add -A` (or `git add` of each new path)
  commits empty files; the commit step must fully stage every new path and
  preserve its mode (`100755` for scripts invoked as `./x.sh`).
- **A provider/account failure is not a timeout.** A lane that dies mid-session
  (`Insufficient Balance`, an auth error) leaves a partial diff; treat it like a
  timed-out worker — inspect the diff, run the touched crate's tests, then
  accept, finish, or revert. Check a lane's provider account before dispatch so
  a known-dead lane is never dispatched to.
- **Captured output flushes at the end.** A long `bash` command prints nothing
  until it finishes; minutes of silence are normal. Do not interrupt on the
  watchdog alone — inspect the run's transcript and status first.
- **Needs-attention is not a stall.** Periodic attention signals fire during
  long builds; triage by transcript.
- **Wall time is noisy, `user` CPU is stable.** On a shared host wall can vary
  two-fold between identical runs; compare `user` seconds and record the load
  average.
- **Interrupted children.** Interrupting a workflow child leaves the parent
  workflow partial. Revive the child with `resume` and continue outside the
  workflow; do not silently switch execution mode.
- **Stale diffs.** Regenerate the diff artifact after every fix round, and
  commit only the byte-identical reviewed revision.

## Tangle integration

- Decompose before dispatch; one node per durable outcome, executed as slices.
- Working children update only their own node and status. A coordinating
  parent's `next` advance belongs to the resolving worker when its write set
  names the parent (or its `next` line); otherwise it is a declared pending
  advance (`tangle check --allow-pending-advance PARENT`). The parent's
  resolving edit is the coordinator's alone, and resolving a coordinating parent
  is never a gate worker's write set: only the coordinator does it, or the brief
  names the parent explicitly in the write set.
- A slice expected to exceed the default 900 s claim lease renews its claim
  (`tangle claim <node> <agent> --base-hash …`) after each build/fix round, so
  `release` reports `released`, not `expired`, and stays a trusted completion
  signal.
- When reconnaissance must precede the deliverable, hash and claim the node for
  the recon, renew the lease after the measurement, and record the base hash of
  the pre-edit node the first claim saw. `tangle hash` after a node edit is a
  different digest from the claim base hash; do not read that difference as a
  mismatch.
- Every implementing commit references its node (`Refs <node>`, `Closes
  <node>`), and `tangle check` runs before every graph commit and hand-off.
- The coordinator records one `FBK` node per orchestrated session for friction
  that touches the Tangle workflow.
