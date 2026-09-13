---
context_rev: 1
updated: 2026-09-13T14:34:06Z
summary: node record truncates summaries silently and allocate skips unowned ids
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Groomed TAS-018 into 16 proposed child nodes: reserved ids with `braintree allocate TAS`, then created each with `braintree node record --id ... --summary ...`.
Friction: Two surprises. (1) `braintree node record` silently truncated four --summary values to roughly 96 characters mid-phrase and exited 0, so the stored summary is a broken sentence: TAS-058 ends "..., with a mode", TAS-059 ends "..., pairwise, and", TAS-060 ends "..., scene data, and", TAS-064 ends "..., migration vers". (2) `braintree allocate TAS`, run when the highest TAS node in .braintree/ was TAS-052, returned TAS-057 on the first call; TAS-053 through TAS-056 exist neither as nodes nor as visible reservation files, so four reserved-but-unwritten ids are indistinguishable from missing ones and the gap is silent.
Improvement: Document the summary length limit in authoring.md, and make `braintree node record`/`braintree feedback record` fail or warn when --summary exceeds it instead of truncating mid-word (or truncate on a word boundary with an ellipsis). Add a read-only way to inspect outstanding reservations, or have `braintree allocate` skip ids that have neither a node nor a live reservation, so a groomer can tell a skipped id from a missing node.
