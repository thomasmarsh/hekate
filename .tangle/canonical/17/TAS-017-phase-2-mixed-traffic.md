---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Coordinate Phase 2 mixed traffic through eight gated increments to a released mixed-mode comparison.
next: [[TAS-020-phase-2-increment-2-lateral-passing-wrong-way]]
---

# Context

Entry gate: the Phase 1 definition of done (`PHASE_1_PLAN.md`) is not yet
satisfied. Only Phase 1 Increment 0 ([[TAS-001-phase-1-increment-0]]) and the
terminal-rendering epic are tracked as nodes; Phase 1 increments 1–6 remain
plan work, so Phase 2 Increment 0 must confirm the gate before any Phase 2
behavior change.

Scope, architecture, validation strategy, risks, and implementation defaults
are fixed by `PHASE_2_PLAN.md`; its "Decisions for implementation" bind every
increment. Each increment decomposes just in time when it takes the frontier,
and pins its resolved predecessor in `# Context` at that point. Passenger-car
and pedestrian behavior stays available as the comparison baseline throughout.

# Outcome

A scenario author can combine passenger cars, pedestrians, bicycles, scooters,
buses, rigid trucks, and one tractor-semitrailer in one continuous world;
assign each mode to ordinary, shared, or prohibited facilities; and observe
overtaking, close passing, wrong-way travel, transit dwell, articulated
off-tracking, and pedestrian group behavior. The release supports comparative
claims about delay, throughput, exposure, and conflicts disaggregated by mode,
with independent physical and operational validation evidence per mode. It does
not claim field calibration or absolute crash prediction.

# Done when

- Passenger cars, pedestrians, bicycles, scooters, buses, rigid trucks, and a
  tractor-semitrailer can share one scenario through composable bodies,
  dynamics, access, and controllers.
- Wheeled agents occupy continuous lateral positions and can perform bounded,
  inspectable passing and lane/facility transitions without teleporting or
  bypassing collision checks.
- Wrong-way travel is a contextual violation over physically connected space
  and produces ordinary interactions plus explicit rule evidence.
- Articulated envelopes, off-tracking, and contacts are tested against
  reference trajectories and swept fixtures.
- Bus stop service conserves passengers and produces separate vehicle and
  person metrics.
- Pedestrian groups coordinate while retaining individual motion, collision,
  and safety state.
- Every supported mode passes physical and isolated operational gates, and
  every material mode pair has a documented interaction disposition.
- The release comparison reports operational and conflict distributions by mode
  and participant pair with reproducible manifests, seed banks, uncertainty
  intervals, and fidelity sensitivity.
- Phase 1 scenarios remain reproducible through their supported source or
  explicit migration path, apart from documented and versioned corrections.
- Model cards state assumptions, parameter sources, validated ranges, known
  failure modes, and incompatible fidelity settings.

Area [[IDX-001-hekate]].
