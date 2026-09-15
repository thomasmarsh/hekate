# Phase 2 entry gate: Phase 1 definition of done

**Verdict: SATISFIED.** All seven items of the Phase 1 definition of done in
`PHASE_1_PLAN.md` are met by the committed tree. Phase 1 Increments 1 through 6
are tracked and resolved as
[[TAS-026-phase-1-increment-1-general-scenario-foundation]] through
[[TAS-050-increment-6-replay-and-live-view]], with the Increment 6 closeout and
per-criterion evidence in
[[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Date: 2026-09-13. Baseline: `baseline.json` against Phase 1 scenario content hash
`668c4bdc575c648133b93f3a406390579c27f3e56567d500298dbfa7c681ff21`. This file
was last re-checked after Phase 1 Increments 1-6 landed; the earlier
2026-09-12 verdict of NOT SATISFIED is superseded.

## Checklist

| # | Phase 1 definition of done | Evidence | Verdict |
|---|---|---|---|
| 1 | A live GPU-backed debug viewer is useful for inspecting geometry, state, decisions, collisions, and conflicts. | `apps/hekate-viewer` (Bevy) drives the kernel only in whole fixed steps and renders every `hekate-present` `SceneFrame`; `crates/hekate-present/src/safety.rs` projects safety markers, body emphasis, region occupancy, standstill, and controller states, so geometry, decisions, collisions, and conflicts are all inspectable. Pinned by `crates/hekate-present/tests/safety_overlays.rs`. | Satisfied |
| 2 | The exact same kernel runs headless, faster than wall-clock, and in parallel replications. | `hekate-cli run` and `batch` drive the same `hekate-sim` `Simulation`; a batch at `--jobs 8` produces the same per-run trace hashes and manifest bytes as `--jobs 1`. `baselines/phase1/performance.json` runs 12.5 s of simulated time in under 1 ms per preset and `perf/release-bench.json` measures a full simulated hour. | Satisfied |
| 3 | Cars and pedestrians move in continuous coordinates through scenarios composed from general primitives. | The `hekate-sim` `AgentStore` holds oriented-box vehicles and circle pedestrians in continuous world coordinates. The schema composes paths, portals, boundaries, regions, movements, crossings, waiting areas, pedestrian routes, conflict regions, rules, signals, and demand, and `scenarios/benchmarks/` builds straight, perpendicular, offset, and four-leg layouts with no scenario-specific simulator branch. | Satisfied |
| 4 | The supported determinism contract is exercised by golden traces and viewer/headless equivalence tests. | `apps/hekate-cli/tests/golden_trace.rs` checks the canonical trace bytes and hash; `apps/hekate-tui/tests/clock_parity.rs` drives one seed through the Bevy viewer clock, the terminal clock, and direct stepping and asserts identical tick sequences and trace hashes; `backend_parity.rs`, `golden_cells.rs`, and `golden_kitty.rs` pin the renderer backends. | Satisfied |
| 5 | Collision/contact and surrogate metrics have analytic/reference fixtures and fidelity sensitivity results. | `crates/hekate-sim/tests/` covers swept and geometry queries, contact and safety events, and time-to-collision, post-encroachment time, and minimum separation against an independent reference occupancy (`metrics.rs`). `experiments/increment6_signal_timing_v1/convergence_evidence.json` reports per-metric Fast (100 ms), Standard (50 ms), and Fine (20 ms) sensitivity. | Satisfied |
| 6 | Two scenario-only design variants can be reproduced and compared from checked-in manifests and seed banks. | `scenarios/experiments/four_leg_pedestrian_{ew,ns}_priority_v1.json5` differ through scenario data alone; `experiments/increment6_signal_timing_v1/{experiment.json,seed_bank.json}` are checked in; `scripts/reproduce-increment6.sh` regenerates the comparison and convergence evidence byte for byte and verifies 60 manifests with `replay --verify`. | Satisfied |
| 7 | Known model limitations and unvalidated claims are explicit. | `docs/known_limitations.md` states the claims that are and are not made. The vehicle and pedestrian model cards in `crates/hekate-sim/src/control.rs` and `crates/hekate-sim/src/pedestrian.rs` document state, parameters, constants, decision inputs, bounds, tie-breaks, and the emergency backstop, protected by `crates/hekate-sim/tests/model_cards.rs`. | Satisfied |

## Additional constraints

- The one-way dependency direction passes: `scripts/check-dependency-direction.sh`
  reports OK, and the baseline and other output commands live in the `hekate-cli`
  application crate and add no kernel dependency.
- Phase 1 Increments 1 through 6 are now tracked: TAS-026 through TAS-043 and
  TAS-045 through TAS-050 are all in `.tangle/resolved/`.

## Consequence

The Phase 2 prerequisite is met, so behavior-changing Phase 2 increments may
begin. Phase 2 Increment 0
([[TAS-018-phase-2-increment-0-baseline-extension-contract]]) still owns the
schema-version-2 design, the deterministic version-1 migration, the compiled
agent components, the model-card template, the benchmark matrix, and the
arbitrary-body-kind representation that
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]] consumes, so Increment 1
remains gated on it.
