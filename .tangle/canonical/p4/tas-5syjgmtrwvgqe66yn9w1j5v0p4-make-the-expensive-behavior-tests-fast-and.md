---
context_rev: 1
status: proposed
updated: 2026-09-15T19:31:30Z
summary: Make the expensive behavior tests fast and narrow.
next: "[[tas-3nx77mbpv0fn9npfdz3at3vywj-fasten-or-harness-move-the-remaining-expensive]]"
---

Parent [[tas-2hx2hbxy3gywdr5qcd2zfny0ff-keep-the-default-test-suite-fast-with-a]].

# Outcome

The expensive behavior tests run sub-second with the same assertions, so the
default suite is quick without a budget exemption.

# Done when

- The `lane_transitions` constraint rules are driven by a crate-internal two-band
scripted fixture (`crates/hekate-sim/src/sim.rs`, next to `push_rider`) in under a
second per rule, with the integration tests replaced and the old thresholds
re-derived on the scripted placement.
- `motor_overtaking`, `narrow_passing`, the merged `close_pass` run, the
`run_metrics` close-pass trio, the `inc2_determinism` other-seed canaries, and
the `scenarios.rs` sweeps are each fast (about a second) or moved to the harness.
- Every assertion still runs; the workspace gate passes and its `user` CPU drops
materially.

# Context

Informed by [[tho-55ch2x2wgytsh1xew9pbjf5py3-classify-default-suite-test-cost-and-design-the]].
