---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: tangle claim accepts a labelled multi-line base hash without validation
tangle_revision: 0.5.0+g64359e6
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Claimed TAS-027 before editing, following SKILL.md: "Get it from `tangle hash` rather than reimplementing the algorithm, and pass that same starting value to `tangle claim` and `tangle release`", so the base hash came from `tangle claim TAS-027 worker --base-hash "$(tangle hash TAS-027)"`.
Friction: `tangle hash TAS-027` prints TOON (`node: "TAS-027"` plus `content_hash: "<digest>"`), not a bare digest, but `tangle claim --base-hash` accepted the whole multi-line output verbatim: it reported result "claimed" and echoed base_hash as the two-line labelled string, with no format validation or warning. Only when reading the claim back did the malformed hash become visible, and it forced a release plus re-claim so the release hash would match the skill rule that the release hash is "the starting hash recorded by the claim". The earlier recorded friction only covers `tangle hash <path>` rejecting a filesystem path, not the accepted-but-malformed base hash.
Improvement: Validate `--base-hash` in claim and release as a lowercase 64-character hex digest and fail non-zero naming the offending form, or print the bare digest from `tangle hash` by default (a labelled/porcelain form can be the opt-in). Alternatively make `--base-hash` default to hashing the named node, so a worker never has to pipe command output into the flag.
