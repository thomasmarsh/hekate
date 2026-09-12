# Phase 2 entry gate: Phase 1 definition of done

**Verdict: NOT SATISFIED.** Phase 1 is implemented only through Increment 0 (a
walking skeleton) plus the terminal-rendering epic. Most of the Phase 1
definition of done in `PHASE_1_PLAN.md` has no implementation, so Phase 2
Increment 0 may capture the current baseline but no behavior-changing Phase 2
increment may begin.

Date: 2026-09-12. Baseline: `baseline.json` against scenario content hash
`668c4bdc575c648133b93f3a406390579c27f3e56567d500298dbfa7c681ff21`.

## Checklist

| # | Phase 1 definition of done | Evidence | Verdict |
|---|---|---|---|
| 1 | A live GPU-backed debug viewer is useful for inspecting geometry, state, decisions, collisions, and conflicts. | `apps/tangle-viewer` (Bevy) renders `tangle-present` scene frames with pause/step/speed controls and overlays. Geometry and constant-speed state render; decisions, collisions, and conflicts do not exist in the kernel to inspect. | Partial |
| 2 | The exact same kernel runs headless, faster than wall-clock, and in parallel replications. | `tangle-cli run` drives the same `Simulation`. `performance.json` shows 12.5 s of simulated time in under 1 ms per preset. `tangle-cli` exposes only `run` and `baseline`; there is no `batch`/replication command. | Partial |
| 3 | Cars and pedestrians move in continuous coordinates through scenarios composed from general primitives. | `tangle-sim` has one `AgentStore` of box-shaped constant-speed cars on a polyline guide path. No pedestrian body, demand, route, or controller exists (`rg -i pedestrian crates/tangle-sim` returns nothing). The scenario schema has paths, portals, and population only: no movements, crossings, signals, or pedestrians. | Not met |
| 4 | The supported determinism contract is exercised by golden traces and viewer/headless equivalence tests. | `apps/tangle-cli/tests/golden_trace.rs` checks the canonical trace bytes and hash; `apps/tangle-tui/tests/walking_parity.rs` checks clock pacing against the same trace. There is no kernel-versus-Bevy-viewer equivalence test. | Partial |
| 5 | Collision/contact and surrogate metrics have analytic/reference fixtures and fidelity sensitivity results. | No broad phase, distance query, swept check, contact event, TTC, PET, or minimum-separation metric exists in `tangle-sim`. `baseline.json` is the only convergence evidence and covers counts, not safety metrics. | Not met |
| 6 | Two scenario-only design variants can be reproduced and compared from checked-in manifests and seed banks. | `scenarios/` contains a single `walking/walking_guide_v1.json5`. There is no experiment spec, seed bank, batch runner, summary, or comparison report. | Not met |
| 7 | Known model limitations and unvalidated claims are explicit. | No model cards exist for the car or pedestrian models. `baselines/phase1/README.md` is a scope note, not a model card with parameter sources and validated ranges. | Not met |

Additional constraints from the Phase 1 plan:

- The one-way dependency direction is enforced and passing:
  `scripts/check-dependency-direction.sh` reports OK, and `baseline.rs` adds no
  kernel dependency (it lives in the `tangle-cli` application crate).
- Phase 1 Increments 1 through 6 are plan text only. None is tracked as a
  Braintree node, so the graph does not currently own the work that clears this
  gate.

## Consequence

`baseline.json` freezes the behavior that exists today, so a later Phase 2 change
can still be classified as intentional or a regression. It does not satisfy the
Phase 2 prerequisite: the Phase 1 definition of done remains open.

The behavior-changing Phase 2 increments ([[TAS-019-phase-2-increment-1-facilities-narrow-modes]]
through [[TAS-025-phase-2-increment-7-release-demonstration]]) must not start
until Phase 1 Increments 1 through 6 land and this checklist passes. Clearing the
gate is Phase 1 work, not Phase 2 work.
