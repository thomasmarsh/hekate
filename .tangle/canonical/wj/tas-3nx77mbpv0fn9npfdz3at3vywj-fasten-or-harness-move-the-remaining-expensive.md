---
context_rev: 1
status: proposed
updated: 2026-09-15T19:05:24Z
summary: Fasten or harness-move the remaining expensive default-suite behavior tests.
next: Fasten the motor_overtaking and narrow_passing approaches to stop when the pass closes.
---

Parent [[tas-5syjgmtrwvgqe66yn9w1j5v0p4-make-the-expensive-behavior-tests-fast-and]].

# Outcome

The remaining over-budget default-suite behavior tests are made fast or moved to the slow harness, so no behavior test in the default suite reports SLOW.

# Done when

- `motor_overtaking` and `narrow_passing` drive a scripted 10-12 m approach that stops when the pass closes.
- The merged `close_pass` run and the `run_metrics` close-pass trio stop at the first closed observation instead of a fixed 900 ticks.
- The `inc2_determinism` other-seed canaries compare one run against a checked-in golden hash instead of re-running.
- The `scenarios.rs` car-following sweep drives 300-600 ticks, or is moved to the harness with its owner named.
- Every assertion still runs somewhere; `cargo nextest run --workspace` passes and its `user` CPU drops materially.

# Context

Informed by [[tho-55ch2x2wgytsh1xew9pbjf5py3-classify-default-suite-test-cost-and-design-the]].

The `motor_overtaking`, `narrow_passing`, `close_pass`, `run_metrics`, `inc2_determinism` canary, and `scenarios.rs` items were classified in the fasten plan; re-scope each against the current tree before dispatch because the measurement/gate split already moved several neighbours.
