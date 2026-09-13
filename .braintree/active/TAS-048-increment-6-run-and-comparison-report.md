---
context_rev: 1
priority: P1
updated: 2026-09-13T11:48:58Z
summary: Slice C of Phase 1 Increment 6 runs the two variants into immutable run directories and produces the concise comparison report over throughput, delay, queues, violations, collision/contact events, TTC, PET, and minimum separation by mode and movement, adding the F6 event-family aggregation and per-metric convergence tolerance that the report depends on.
next: Run both variants into immutable run directories and produce the concise comparison report with every gate metric by mode and movement, adding F6 event-family aggregation and per-metric convergence tolerance.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Consumes the variant scenarios and experiment spec from
[[TAS-047-increment-6-scenario-variants-and-experiment-spec]], the v2 metrics
from [[TAS-046-increment-6-gate-metrics-throughput-delay-queues]], and the
Increment 5 `batch`, `aggregate`, `compare`, and `converge` surfaces.

Two Increment 5 residuals are folded into this slice because it owns the
comparison report:

- **F6:** the batch-level mode slice carried only `minimum_separation_m`, and the
  run artifact's event-family mode/movement counts were not aggregated. The gate
  requires collision/contact events, violations, and queue counts by mode and
  movement, so aggregation must carry them.
- **Per-metric convergence tolerance:** convergence used one global 5% relative
  tolerance, so count metrics (0 → 1 is unbounded) commonly flagged as
  materially sensitive. A per-metric tolerance must let the report distinguish
  material sensitivity from noise.

# Outcome

- Both variants are run into immutable run directories from the checked-in spec
  and seed bank.
- A concise comparison report reports throughput, delay, queues, violations,
  collision/contact events, TTC, PET, and minimum separation by mode and
  movement.
- `aggregate` carries the full mode and movement slices, including event-family
  counts (F6 closed).
- `converge` applies a per-metric relative tolerance, and the comparison report
  reports material sensitivity rather than hiding it.

# Done when

- Immutable run directories exist for both variants across the specified seeds,
  with manifests, summaries, event streams, and sampled trajectories consistent
  with the declared sampling policy.
- The comparison report reports throughput, delay, queues, violations,
  collision/contact events, TTC, PET, and minimum separation by mode and
  movement, and every number links to its manifest(s) and its
  `metric_definition_version` value (2 for the new metrics).
- Aggregation carries the full mode and movement slices including event-family
  counts (F6 closed), with tests.
- Convergence uses a per-metric relative tolerance and the report states material
  sensitivity per metric; tests cover the tolerance rule.
- Parallel and serial batch execution produce identical per-run trace hashes.
- No existing golden or baseline is broken without a deliberate, reported
  regeneration.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.
