---
status: proposed
context_rev: 1
priority: P2
updated: 2026-09-15T01:22:43Z
summary: The always-on interaction-metrics pass costs roughly 18–33 microseconds per tick (54–75% of the tick) on mixed_interaction_v1 and cannot be switched off; make it switchable and/or optimized against that recorded ablation baseline.
next: Reproduce the mixed_interaction_v1 ablation, then make the always-on interaction-metrics pass switchable and/or optimize it, recording the before/after per-tick cost.
---

# Context

Area [[IDX-001-hekate]]. Residual of Phase 1 Increment 5, recorded as
[[TAS-031-phase-1-increment-5-experiments-outputs-convergence]]'s limitation and
measured by [[TAS-042-phase-1-increment-5-release-benchmarks-profiling]]:
release-mode ablation on `mixed_interaction_v1` puts the always-on
interaction-metrics pass at roughly 18–33 microseconds per tick depending on the
window, 54–75% of the tick, confirming Increment 4's ~22/7 microsecond ratio in
shape. Increment 5 deliberately recorded this as its baseline and did not
optimize it; Increment 6 likewise does not absorb it.

This node exists because the measured cost is a durable, independently resumable
performance outcome: a consumer may want the metrics pass switchable (off for
throughput-only runs) or cheaper (fewer candidates, earlier rejection) without
changing reported metric values when it is on.

# Outcome

The always-on interaction-metrics pass is switchable and/or measurably cheaper,
with the recorded ablation as the before baseline and no change to reported
metric values when the pass is enabled.

# Done when

- The `mixed_interaction_v1` ablation is reproduced on the current tree as the
  before baseline.
- A switch (or an equivalent scoped control) can disable the pass without
  changing kernel determinism, event streams, or any non-metric output, and is
  covered by tests.
- Any optimization preserves reported metric values when the pass is enabled;
  differential tests compare optimized against reference.
- Before/after per-tick cost is recorded and the improvement is reported.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `tangle check`.
