# AGENTS.md

## Braintree is the work ledger (mandatory)

Non-trivial engineering work in this repo is tracked as Braintree nodes under
`.braintree/`. Before planning or changing tracked work you MUST load the full
Braintree skill (`SKILL.md`; `/skill:braintree` in pi) and follow it; its
summary alone is not sufficient. Do not plan or track multi-step work from chat
memory, and do not add ad-hoc TODO lists or plan documents.

The vault root is the repository root. The installed `braintree` command fronts
the skill, so run tooling from the repository root.

Repo-specific conventions layered on the skill:

- Run tooling as `braintree …`, for example `braintree check` to validate
  the graph and `braintree index` to rebuild the optional sidecar index.
- Every commit that implements tracked work references its node in the body:
  `Refs TAS-XXX` while the work continues, `Closes TAS-XXX` once the node's
  outcome is complete. Untracked commits are rejected in review.
- Run `braintree check` before every handoff and before committing graph
  changes; a failing check blocks the commit. Fix the graph, not the checker.

## Record Braintree friction (mandatory)

Whenever planning, implementation, or analysis work that uses the Braintree
skill struggles, capture the friction before handoff so it accumulates into
later meta-analysis:

- Record one `FBK` feedback node per session with `braintree feedback record`,
  routed under `IDX-002-braintree-feedback`:

  ```sh
  braintree feedback record \
    --route 'Area [[IDX-002-braintree-feedback]]' \
    --attempted '…' --friction '…' --improvement '…'
  ```

  The command allocates the next `FBK` id, stamps `braintree_revision` from the
  installed record, and writes `.braintree/proposed/FBK-<n>-<slug>.md`. Pass
  `--route` explicitly: without it the command routes to the vault's root hub,
  not the feedback hub. Record findings; do not edit the installed skill or the
  sidecar in the same session.
- Treat as friction: a rule the skill leaves unclear, a `braintree` result that
  surprises you, a check that fails for a reason the skill does not explain, and
  a judgment call that exposes a gap. A fresh-sidecar `braintree allocate`
  returning an ID that already exists in `.braintree/` is a concrete example.
- Put what happened, the exact command and observed output, the expected
  behavior, and a proposed change to `SKILL.md` or the tooling into the node's
  `--friction` and `--improvement` text. Extend the generated node by hand when
  one session has several findings.
- Aggregate the notes into concrete Braintree improvements in a separate task
  under the same hub; the notes are evidence, not the fix. Collect friction
  already recorded in other vaults with `braintree feedback scan <vault>`.

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
