# AGENTS.md

## Tangle is the work ledger (mandatory)

Work whose conclusion or executable state must outlive the session is tracked
as Tangle nodes in the stationary store `.tangle/canonical/`; work finished and
committed within one session needs no node — Git is its durable record. Before
planning or changing tracked work you MUST load the full Tangle skill
from its installed path `~/.pi/agent/skills/tangle/SKILL.md` (or `/skill:tangle`
in pi) and follow it; its summary alone is not sufficient. Never search `.` or
`/` recursively for it — the path is known — and locate a known node with
`find .tangle -name '<ID>-*'`. Do not plan or track multi-step work from chat
memory, and do not add ad-hoc TODO lists or plan documents.

The vault root is the repository root. The installed `tangle` command fronts
the skill, so run tooling from the repository root.

Repo-specific conventions layered on the skill:

- Run tooling as `tangle …`, for example `tangle check` to validate the graph.
  The derived index maintains itself on every interaction; run `tangle index`
  only to repair or rebuild it from Markdown.
- Every commit that implements tracked work names its node in the body —
  `Refs <node-id>` while the work continues, `Closes <node-id>` once the node's
  outcome is complete. The id is the leading identity in the node's basename,
  for example `tas-5syjgmtr…` or a legacy `TAS-101`. Untracked commits are
  rejected in review.
- Run `tangle check` before every handoff and before committing graph
  changes; a failing check blocks the commit. Fix the graph, not the checker.

## One session, one coherent slice (mandatory)

A Tangle node is a durable outcome, not a session: one node may span many
sessions and one session may advance several frontier nodes. Size the work to the
session, not the session to the node.

- Before writing code, read the frontier node's `# Done when` and state the slice
  you will deliver and what explicitly stays out of scope. Split it into child
  nodes before starting when it bundles outcomes with independent acceptance or
  verification boundaries; only a split that adds an outcome the request did not
  ask for needs the user's agreement. Session duration, file count, subsystem
  count, and anticipated commits are sizing guidance, never split evidence. A
  `# Done when` that names a release-mode benchmark, profiled capture, or other
  long measurement names that run's wall-clock cost, and the long run is its own
  slice rather than bundled with the artifact's authoring.
- If a node's `# Done when` cannot be met in one session, do not expand the
  session to meet it. Deliver the smallest coherent slice that clears its blocker
  or completes one deliverable, record the exact remaining scope and evidence in
  the node, and leave the node `proposed`/`active` with a `next` naming the first
  remaining action. A session that unblocks a node has not completed it.
- Do not reinterpret a request as larger than asked. A node that bundles several
  independent deliverables is several nodes; split it and record each child in
  the graph rather than expanding the session silently.
- Do not add deliverables, new nodes, or plan documents because the node "really
  needs" them. Record them as ledger scope, not as this session's work.

## Small, fully-loaded nodes and shared reconnaissance (mandatory)

A node is the unit of *loading*, so a node an agent must load to work should be
small enough to read completely in one pass. Keep the outcome, `# Done when`, and
pointers in the node; never paste reconnaissance, large code excerpts, or session
history into it. Deep detail belongs in linked nodes that the reader loads only
when a pointer requires it.

- **Share reconnaissance instead of relitigating it.** Orientation is the largest
  fixed cost of a session. Once a session has paid it, record the durable
  findings — code seams and symbols, prior-session evidence, decision context —
  as `THO` nodes routed to the appropriate hub, and have the working nodes link
  them. Link reconnaissance with the non-pinned `Informed by [[TARGET]].` line
  in `# Context` (opt-in context), not a
  `Depends on` pin, which stays reserved for context-bearing dependencies. A
  node that needs the context loads the linked node on demand; it does not
  restate it.
- **Decompose freely, to any depth.** Tangle is a graph: a child, its child,
  and its child are all ordinary nodes. There is no mandated taxonomy depth and
  no required two-level shape. Break a problem into as many nodes as it has
  independently resumable outcomes, and chase the `next` route down the tree
  depth-first to the current leaf rather than planning breadth-first.
- **Size slices up front, not by clock.** Roughly ten minutes of agent work is a
  guideline, not a rule. The binding constraint is that a slice delivers one
  verifiable change and leaves every node truthful. When a frontier node's
  `# Done when` plainly bundles independent deliverables, split it before
  dispatching rather than discovering that mid-run.
- **Defer instead of expanding.** When execution reveals a larger-than-expected
  scope, the working agent creates the smallest direct child that owns the newly
  discovered, independently resumable outcome (recording its outcome, route,
  and `next`) and leaves its own node `active` with a `next`; it does not widen
  the slice to meet the node. Growing a slice to finish a node is the failure
  this section exists to prevent.

## Record Tangle friction (mandatory)

Whenever planning, implementation, or analysis work that uses the Tangle
skill struggles, capture the friction before handoff so it accumulates into
later meta-analysis:

- One orchestration session records one `FBK` feedback node, owned by the
  coordinator. A worker that hits friction reports the attempted action,
  friction, and improvement in its run report and creates the node only when
  the coordinator grants it. Use `tangle feedback record`, routed under
  `IDX-002-tangle-feedback`:

  ```sh
  tangle feedback record \
    --route 'Area [[IDX-002-tangle-feedback]]' \
    --attempted '…' --friction '…' --improvement '…'
  ```

  The command generates a lowercase 128-bit `fbk-` id from cryptographic
  entropy, stamps `tangle_revision` from the installed record, and writes the
  node to the stationary store at
  `.tangle/canonical/<suffix>/fbk-<id>-<slug>.md`. Pass `--route` explicitly:
  without it the command routes to the vault's root hub, not the feedback hub.
  Record findings; do not edit the installed skill or the local coordination
  state in the same session.
- Treat as friction: a rule the skill leaves unclear, a `tangle` result that
  surprises you, a check that fails for a reason the skill does not explain, and
  a judgment call that exposes a gap. A `tangle allocate` for a legacy numeric
  prefix returning an id that already exists in `.tangle/` is a concrete
  example.
- Put what happened, the exact command and observed output, the expected
  behavior, and a proposed change to `SKILL.md` or the tooling into the node's
  `--friction` and `--improvement` text. Extend the generated node by hand when
  one session has several findings.
- Aggregate the notes into concrete Tangle improvements in a separate task
  under the same hub; the notes are evidence, not the fix. Collect friction
  already recorded in other vaults with `tangle feedback scan <vault>`.

## Orchestrated sessions

When asked to run an orchestrated, resumable, multi-slice, or subagent-driven
session, read `docs/orchestrated-sessions.md` first. It records the roles
(scout, implementer, build/test, reviewer, commit), the consistent hand-off
protocol, the build-warmup trade-off, and the known pitfalls (the `git diff`
untracked-file blind spot and the `git add -N` empty-blob trap, cargo-lock
serialization, noisy wall time, mid-session lane failure). Keep this file free
of that detail.

## Commits

Always use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

- **Types**: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`,
  `revert`, `style`, `test`.
- **Scope** is the affected subsystem or package, e.g. `hekate-sim`,
  `hekate-model`, `cli`, `viewer`.
- Keep the subject imperative, lowercase after the colon, and without a
  trailing period.
- Add a body for non-trivial changes; reference the Tangle node as described
  in the work-ledger section above.
- Mark breaking changes with `!` before the colon and a `BREAKING CHANGE:`
  footer.

Local commits are enforced by the `commit-msg` hook
(`scripts/check-commit-message.sh`). Enable it once per clone:

```sh
git config core.hooksPath .githooks
git config commit.template .gitmessage
```

CI re-checks every pushed commit range with the same script.
