---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: A model-plus-profile-plus-card slice is not buildable in this Rust workspace when it lands befor
tangle_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Decomposed TAS-077 (wire narrow-mode spawning) into the smallest dead-code-clean slice, following the coordinator's example slice 'the narrow motion model + profile sampling + model card'.
Friction: A model-plus-profile-plus-card slice is not buildable in this Rust workspace when it lands before its spawn consumer: cargo clippy --workspace --all-targets -- -D warnings rejects the unused replaceable-trait method and the unread profile fields, so an 'isolated controller' with no live caller fails the gate. I had to wire the spawn path and the kernel dispatch in the same slice to make the model live, which is larger than the suggested slice.
Improvement: SKILL.md's decomposition guidance should note that in a workspace with warnings-as-errors, a just-in-time slice that lands a type or trait before its consumer is not independently acceptable; either the slice must include a live consumer, or the node's next must name the consumer as a mandatory companion. The example 'model + profile + card' slice in the worker handoff was not viable as written.
