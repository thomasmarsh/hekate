---
context_rev: 1
updated: 2026-09-13T19:57:56Z
summary: Complete narrow longitudinal tactics and compiled-facility route completion.
next: Author a narrow longitudinal fixture with a leader, a stop line, a signal, and a compiled facility, and assert envelope and body bounds across all five behaviors.
---

Parent [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]].

# Outcome

Complete the narrow longitudinal behavior TAS-077 wired: route one narrow agent
through the shared four stages so it accelerates, follows a leader, holds a stop
line, yields at a signal, and completes its route along a compiled facility, and
prove command envelopes and body bounds hold in each state. Narrow modes select
and follow a compiled facility route and complete it with no named-mode branch in
shared code.

# Done when

- A test proves command envelopes and body bounds are respected for one narrow
  agent accelerating, following a leader, holding a stop line, yielding at a
  signal, and completing its route along a compiled facility.
- Narrow modes select and follow a compiled facility route and complete it, with
  no named-mode branch in shared code.
- `cargo test -p tangle-sim` and `cargo test --workspace` pass with every Phase 1
  golden trace unchanged.

# Context

The remainder of [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]],
split because TAS-077 landed the spawn wiring, narrow model, profile sampling,
and model cards but the longitudinal-tactic suite and facility route completion
did not fit one session. TAS-077 delivered: narrow agents spawn from their
compiled mode template through `AgentFamily::WheeledCapsule` family dispatch, the
narrow wheeled model (`crate::narrow`) reached through the controller seam,
`sample_narrow_profile`, the bicycle and scooter model cards, and the
no-shared-mode-branch guard.

What remains is behavior, not wiring: following, stop lines, and signal yields
already flow through the shared stages for any path-following profile, so the
work is a checked-in narrow fixture carrying a leader, a stop line, and a signal,
plus compiled-facility route selection and completion, and the single Done-when
test over them. Read [[TAS-068-controller-stage-interfaces]],
[[TAS-074-compile-version-2-facility-and-connector-shapes]], and
[[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]].
