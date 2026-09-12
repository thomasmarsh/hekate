---
context_rev: 1
priority: P1
updated: 2026-09-12T14:06:20Z
summary: Terminal backend selection is capability-gated: `auto` probes and requires a positive reply, explicit flags override, and failover restores the terminal and continues in cells.
---

# Outcome

`tangle-tui` selects a terminal backend from proven capabilities: it uses Kitty
graphics only when support is confirmed, falls back to character cells
otherwise, and never leaves the terminal corrupted.

# Done when

- Detection consults environment hints and terminal queries, and treats
  ambiguous answers as unsupported.
- A positive probe gates the backend but is never treated as proof of safety:
  [[TAS-010-kitty-graphics-poc]] shows a terminal that answers the probe `OK`
  and then crashed under the naive same-id lifecycle. Selecting the Kitty
  backend always uses the bounded lifecycle mandated by
  [[DEC-002-terminal-backend-strategy]].
- An explicit `--backend ascii|kitty|auto` flag overrides detection, with `auto`
  as the default.
- Falling back after a partial failure restores the alternate screen, cursor,
  and raw mode, and preserves playback position and selection.
- Detection and fallback are unit-tested against a fake capability responder and
  a fake terminal writer.

# Context

Depends on [[DEC-002-terminal-backend-strategy]] at context_rev 2.
Depends on [[TAS-012-terminal-viewer-ascii]] at context_rev 1.
Detection only matters for an opt-in backend, so this follows the decision and
the character-cell fallback.

Parent [[TAS-009-renderer-backends]].

# Result

Detection, gating, and failover are implemented and tested. The Kitty pixel
renderer itself remains [[TAS-013-terminal-viewer-kitty]]; this node owns the
single gate through which it is selected, so there is no path to a naive
transmission lifecycle.

- `capability.rs` holds the seam. `EnvironmentHints` captures the usual
  `TERM`/`TERM_PROGRAM`/`KITTY_WINDOW_ID`/mux/SSH variables and yields a verdict
  only for definite negatives (`TERM=dumb`/empty, `TERM_PROGRAM=Apple_Terminal`);
  a positive hint alone never selects the backend. `TerminalResponder` sends the
  protocol's `a=q` probe followed by `CSI c` with a bounded `mio` poll, and
  `classify` maps `OK` to `Supported` and every other shape
  (`unsupported`, `graphics-error`, `no-reply`) to non-supported.
  `BackendKind::resolve` selects Kitty for `auto` only on `Supported`; `ascii`
  and `kitty` override detection and skip the probe.
- `terminal.rs` extracts `TerminalModes` (`enter`/`restore`), so the alternate
  screen, cursor, autowrap, and raw mode have one owner whether the process is
  exiting, panicking, or switching backends.
- `fallback.rs` holds `BackendFallback`: on a primary `draw`/`present` error it
  restores the terminal first, switches to the fallback, and replays the same
  `SceneFrame`. Playback position and selection live in the
  `PresentationController`, so they survive the switch unchanged.
- `main.rs` parses `--backend ascii|kitty|auto` (default `auto`) and runs
  detection for `auto`. A failed or unreadable probe degrades to cells with a
  note rather than aborting.

# Evidence

`cargo test -p tangle-tui` is 41 library tests plus 3 argument tests plus 2
walking-parity integration tests, all green. The new tests cover reply parsing
and every classification shape, hint short-circuiting without sending a query,
the rule that a positive environment hint alone does not select Kitty, explicit
override, `auto` selecting Kitty only on `Supported`, and failover restoring the
terminal and replaying the same frame and selection. The probe and fallback
tests use a fake responder and fake backends, so they need no terminal.

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace --all-features`, and
`./scripts/check-dependency-direction.sh` are clean; the checked-in golden trace
hash is unchanged.

# Residual

The bounded Kitty lifecycle is a property of the backend rather than of
selection; [[TAS-013-terminal-viewer-kitty]] implements it and is the one
consumer of `BackendKind::Kitty`. [[TAS-009-renderer-backends]] advances to that
node.
