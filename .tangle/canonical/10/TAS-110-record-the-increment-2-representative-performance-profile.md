---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T10:34:41Z
summary: Record the representative mixed-mode profile and Increment 2 performance budget.
next: Run the five-gate validation and resolve.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

A checked representative Increment 2 mixed-mode profile records the cost of
lateral decisions, candidates, prediction, and output and sets declared later
optimization budgets relative to the frozen Increment 0 Phase 1 baseline.

# Done when

- One checked scenario/profile combines passenger cars, bicycles, and scooters
  with passing and opposing interactions at a declared density and fixed seed
  bank; it is an evidence workload, not the final release comparison.
- Release-mode measurements on named hardware record agent-steps/s, tactical
  and broad-phase candidates, prediction work/agent-step, simulated/wall time,
  output bytes/simulated hour, and presenter frame time where applicable.
- The report compares the same common workload at disabled-versus-enabled
  lateral machinery and to the frozen Phase 1 baseline without claiming
  cross-hardware equivalence.
- Budgets, dominant measured work, profiler commands, raw artifact hashes, and
  rerun commands are checked in; no optimization is performed in this leaf.
- The workload reproduces, tests pass, and unsupported density/fidelity ranges
  are labeled.

# Context

Gated on [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]] and
[[TAS-109-document-increment-2-evidence-and-model-limits]]. Owns one Increment 2
performance workload, its seed bank if distinct, and benchmark/report artifacts
under the repository's existing benchmark conventions. Do not optimize code or
author the final mixed-mode release fixture.

Gates my scenario and benchmark artifacts enter: scenario enumeration, any
checked benchmark-artifact manifest test, and cargo test --workspace.

# Slices

- Add the representative mixed-mode workload fixture: cars, bicycles, and
  scooters with passing and opposing interactions at a declared density and a
  fixed seed bank.
- Record release-mode measurements on named hardware: agent-steps per second,
  tactical and broad-phase candidates, prediction work per agent-step,
  simulated/wall time, output bytes per simulated hour, and presenter frame time.
- Write the disabled-versus-enabled and Phase 1 baseline comparison, budgets,
  profiler commands, raw artifact hashes, and rerun commands; label unsupported
  density and fidelity ranges.

# Result

Recorded the Increment 2 representative profile on the same host as the Phase 1
capture. All Done-when clauses are now met; the five-gate validation is the only
remaining step.

- **Release profile row.** `scenarios/phase2/inc2/mixed_mode_profile_v2.json5`
  (480 arrivals/hour on the corridor, ~9 live bodies) is now the seventh row of
  `perf/release-bench.json`; the six Phase 1 rows are byte-identical to `HEAD`
  (only the new row was appended, and the artifact's `generated_unix_s` stays the
  Phase 1 capture's). One simulated hour, median of three passes: 111.431 s wall
  (1 547.65 µs/tick, 32.31 simulated s per wall s), min 111.422 s; 702 894 agent
  steps, 6 308 agent steps/s; 460 spawned / 449 despawned / 11 remaining;
  770 144 trace bytes and 3 033 055 run-directory bytes per simulated hour. Sweep:
  `cargo test --release -p hekate-cli --test release_benchmark -- --ignored
  --nocapture`, exit 0, 506 s wall, nearly all of it the new scenario (its three
  timed passes alone are 334 s).
- **Profiler capture.** `perf/profiles/mixed_mode_profile_v2-release.*` from
  `scripts/capture-profile.sh … 300000 8`: `Simulation::predict_candidate` is
  5 491 of 5 737 tick samples (95.7 %), and the per-candidate clearance
  primitives are its top self-time frames. The metrics pass is 1.9 %.
- **Comparison.** Against the six Phase 1 rows (same host, harness, and release
  profile): 22.8× `car_following_v1` and 35.1× `mixed_interaction_v1` per tick,
  and ~38× `mixed_interaction_v1`'s per-live-agent cost. The 960 arrivals/h
  shared-facility draft gridlocks (superlinear 440 → 9 300 µs/tick over 500 →
  4 000 ticks) and is recorded in `perf/README.md` as the labelled unsupported
  density, where a uniform hour-long sweep is infeasible.
- **One description corrected.** The new row's `role` string said "960
  arrivals/hour, ~18 live bodies", the gridlocked first draft's density; it now
  names the fixture's declared density (480 arrivals/hour on the corridor, ~9
  live bodies), so the harness and the artifact row agree with the fixture.
- **Counters, presenter, ablation, budgets (this slice).** Added non-behavioural
  counters (`Simulation::performance_counters()`): a broad-phase candidate
  counter in `crates/hekate-sim/src/index.rs` and prediction counters in
  `crates/hekate-sim/src/sim.rs`. Bounded release measurement at seed 11, 5 000
  ticks, three passes: enabled 863.8 µs/tick median (861.4 min), disabled (twin
  `scenarios/phase2/inc2/mixed_mode_profile_v2_no_lateral.json5`) 25.5 µs/tick
  median (25.4 min) — the lateral machinery is 97.1 % of the bounded per-tick
  cost. Per agent-step: 4.150 broad-phase candidates, 0.193 `predict_candidate`
  evaluations, and 1.312 prediction candidate bodies enabled, 4.308 / 0 / 0
  disabled; 108.9 against 2.95 µs per agent-step. Presenter projection (headless,
  `PresentationController::project`, 5 000 frames): 0.735 µs/frame at 7.998
  bodies. End-to-end CLI corroboration at 5 000 ticks/seed 11: 3.651 s enabled
  (730 µs/tick), 0.138 s disabled. Increment 2 budgets (ceiling = measured × 1.10,
  the documented host spread) are recorded in `perf/README.md`: 1 702 µs/tick hour
  window, 950 µs/tick 5 000-tick window, ≤ 4.6 broad-phase candidates/agent-step,
  ≤ 0.21 predictions/agent-step, ≤ 1.44 prediction candidate bodies/agent-step,
  ≤ 28 µs/tick disabled floor, ≤ 0.81 µs/frame presenter. The existing Inc2
  fixture tests pass, the twin validates, and the counters change no trace byte.

All Done-when clauses are met. Remaining: run the five-gate validation and
resolve the node.
