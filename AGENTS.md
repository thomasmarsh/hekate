# AGENTS.md

## Braintree is the work ledger (mandatory)

Non-trivial engineering work in this repo is tracked as Braintree nodes under
`nodes/`. Before planning or changing tracked work you MUST load the full skill
at `<skill-root>/SKILL.md` and follow it; its summary alone is not sufficient
(`/skill:braintree` in pi). Do not plan or track multi-step work from chat
memory, and do not add ad-hoc TODO lists or plan documents.

The vault root is the repository root. The skill may be installed project-locally
or globally, so resolve it from the repository root before running tooling:

```sh
for d in "$BT_SKILL_ROOT" .pi/skills .claude/skills .agents/skills "$HOME/.pi/agent/skills"; do
  [ -n "$d" ] && [ -d "$d/braintree" ] && SKILL_ROOT="$d/braintree" && break
done
```

Repo-specific conventions layered on the skill:

- Run tooling as `uv run --project "$SKILL_ROOT" --frozen bt …` and
  `uv run --project "$SKILL_ROOT" --frozen graph-check nodes`.
- Every commit that implements tracked work references its node in the body:
  `Refs TAS-XXX` while the work continues, `Closes TAS-XXX` once the node's
  outcome is complete. Untracked commits are rejected in review.
- Run `graph-check nodes` before every handoff and before committing graph
  changes; a failing check blocks the commit. Fix the graph, not the checker.

## Record Braintree friction (mandatory)

Whenever planning, implementation, or analysis work that uses the Braintree
skill struggles, capture the friction before handoff so it accumulates into
later meta-analysis:

- Add one resolved `THO` node under `IDX-002-braintree-feedback` per session.
  Record findings; do not edit the installed skill or the sidecar in the same
  session.
- Treat as friction: a rule the skill leaves unclear, a `bt` or `graph-check`
  result that surprises you, a check that fails for a reason the skill does not
  explain, and a judgment call that exposes a gap. A fresh-sidecar `bt allocate`
  returning an ID that already exists in `nodes/` is a concrete example.
- For each finding record what happened, the exact command and observed output,
  the expected behavior, and a proposed change to `SKILL.md` or the tooling.
- Aggregate the notes into concrete Braintree improvements in a separate task
  under the same hub; the notes are evidence, not the fix.

## Commits

Always use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

- **Types**: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`,
  `revert`, `style`, `test`.
- **Scope** is the affected subsystem or package, e.g. `tangle-sim`,
  `tangle-model`, `cli`, `viewer`.
- Keep the subject imperative, lowercase after the colon, and without a
  trailing period.
- Add a body for non-trivial changes; reference the Braintree node as described
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
