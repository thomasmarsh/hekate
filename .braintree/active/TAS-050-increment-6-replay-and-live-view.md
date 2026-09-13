---
context_rev: 1
priority: P1
updated: 2026-09-13T11:48:58Z
summary: Slice E of Phase 1 Increment 6 produces a recorded replay of the comparison and live-view instructions so a reviewer can inspect geometry, state, decisions, and events visually, reusing the existing replay and viewer/TUI surfaces.
next: Produce a recorded replay of an Increment 6 run and write live-view instructions that reuse the existing replay and viewer/TUI for visual review.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Reuses the existing `replay` command and the viewer/TUI from the terminal
rendering epic ([[TAS-006-bevy-viewer-skeleton]] and Increments 4–5 viewer work).
This is a demonstration and documentation slice: it adds no new simulation
behavior and must not change the kernel or the determinism contract.

# Outcome

- A recorded replay artifact produced from an Increment 6 run.
- Live-view instructions (documented command and options) that let a developer
  watch the same run and inspect geometry, state, decisions, and events.

# Done when

- A recorded replay artifact is checked in or is reproducible by one documented
  command from a checked-in run.
- Live-view instructions exist and are accurate against the current CLI/viewer.
- The recorded replay and live view do not change state-affecting behavior; any
  headless-versus-viewed trace comparison stays equal where the contract
  requires it.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.
