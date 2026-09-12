---
context_rev: 1
updated: 2026-09-12T17:06:28Z
summary: braintree claim accepts a labelled multi-line base hash without validation
braintree_revision: 0.5.0+g64359e6
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Claimed TAS-027 before editing, following SKILL.md: "Get it from `braintree hash` rather than reimplementing the algorithm, and pass that same starting value to `braintree claim` and `braintree release`", so the base hash came from `braintree claim TAS-027 worker --base-hash "$(braintree hash TAS-027)"`.
Friction: `braintree hash TAS-027` prints TOON (`node: "TAS-027"` plus `content_hash: "<digest>"`), not a bare digest, but `braintree claim --base-hash` accepted the whole multi-line output verbatim: it reported result "claimed" and echoed base_hash as the two-line labelled string, with no format validation or warning. Only when reading the claim back did the malformed hash become visible, and it forced a release plus re-claim so the release hash would match the skill rule that the release hash is "the starting hash recorded by the claim". The earlier recorded friction only covers `braintree hash <path>` rejecting a filesystem path, not the accepted-but-malformed base hash.
Improvement: Validate `--base-hash` in claim and release as a lowercase 64-character hex digest and fail non-zero naming the offending form, or print the bare digest from `braintree hash` by default (a labelled/porcelain form can be the opt-in). Alternatively make `--base-hash` default to hashing the named node, so a worker never has to pipe command output into the flag.
