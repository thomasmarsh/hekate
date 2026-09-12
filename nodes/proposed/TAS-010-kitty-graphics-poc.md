---
context_rev: 1
priority: P1
updated: 2026-09-12T12:34:01Z
summary: Spike proves or disproves Kitty graphics as a Tangle terminal backend.
next: Build a throwaway Kitty client that blits a moving RGBA frame and measure coverage and throughput.
---

# Outcome

A recorded go/no-go on using the Kitty graphics protocol for the Tangle terminal
viewer, backed by a runnable spike that transmits, places, animates, and deletes
pixel frames on real terminals.

# Done when

- The spike draws a moving scene frame through the Kitty graphics protocol on at
  least one supporting terminal, and again under tmux with passthrough.
- It reports frame size, encode and transmit cost, and achievable frame rate
  for a walking-skeleton-sized scene and for a denser synthetic scene.
- It records behavior on at least one non-supporting terminal and the detection
  signal that distinguishes the two cases.
- It states whether image placement survives resize, scroll, and alternate
  screen transitions without leaking images.
- The spike is either disposed after the decision or promoted as the basis of
  the conditional Kitty backend.

# Context

Depends on [[TAS-002-rust-workspace-ci]] at context_rev 1.
The pin covers the workspace and toolchain only. The spike is deliberately
independent of the shared presentation layer so it can fail cheaply before any
refactor.

Parent [[TAS-009-renderer-backends]].
