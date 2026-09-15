---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Increment 2 event, metric, trajectory, presenter, and fixture gates.
---

Area [[IDX-001-hekate]].

# Scope

Shared, opt-in reconnaissance for the Increment 2 output and acceptance work
(TAS-099/100/101/102 and children, TAS-111). Verified at commit 35d8030.

# Seams

- `crates/hekate-sim/src/event.rs`: `EVENT_VERSION`, `Event`, `EventKind`,
  `ViolationKind`, `Event::order_key`.
- `crates/hekate-sim/src/metrics.rs`: `InteractionMetrics`,
  `tick_minimum_clearance_m`.
- `crates/hekate-sim/src/query.rs`: `body_clearance_m`, `CONTACT_EPSILON_M`.
- `apps/hekate-cli/src/run_metrics.rs`: `METRIC_DEFINITION_VERSION`,
  `EVENT_FAMILY_LABELS`; `trajectories.rs`: `TRAJECTORY_FORMAT_VERSION` (3);
  `run_dir.rs`: `RUN_MANIFEST_VERSION`, `RunManifest`, `fidelity`; `trace.rs`,
  `replay.rs`.
- `crates/hekate-present/src/scene.rs`: `SCENE_FORMAT_VERSION`.

# Gates and fixtures

- `apps/hekate-cli/tests/migration_regression.rs` enumerates
  `scenarios/**/*.json5`: a new fixture under `scenarios/phase2/inc2/` enters
  that suite, so name it before choosing the path.
- `crates/hekate-present/tests/v2_fixtures.rs`; benchmark-matrix checked-path
  tests; `docs/benchmark-matrix.md` (Markdown and JSON);
  `docs/model-card-template.md`.
- Tolerances T-O1/T-O2/T-O3 and T-H1/T-H2; fixtures `narrow_passing_v2`,
  `motor_passing_narrow_v2`, `motor_lane_change_v2`, `narrow_wrong_way_v2`.
- Do not widen a matrix cell or disposition; preserve goldens and frozen
  versions unless the leaf explicitly versions an output.
