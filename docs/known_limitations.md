# Known limitations

This document states what the Phase 1 release demonstration does **not** claim.
It is the honesty half of Increment 6
(`[[TAS-045-phase-1-increment-6-first-useful-release-demonstration]]`): the
comparison report, the convergence evidence, and the golden traces say what the
model did; this file says what may and may not be concluded from it. A claim not
listed under *What the demonstration claims* is not a claim of this project.

Everything below is a property of the committed tree, and the evidence for each
statement is a checked-in artifact or a named source location.

## What the demonstration claims

- **Reproducibility of the canonical event stream.** A completed run directory
  reproduces its recorded `events.jsonl.gz` and trace hash from its own
  `manifest.json` in the supported determinism environment: `replay --verify`
  passes on the manifests behind the checked-in golden traces
  (`apps/hekate-cli/tests/increment6_trace.rs`) and on every run directory the
  reproduction command writes.
- **Reproducibility of the comparison.** One command regenerates
  `comparison_report.json` and `convergence_evidence.json` byte for byte from
  the checked-in spec, seed bank, and scenario sources, with no run artifact
  rewritten (`scripts/reproduce-increment6.sh`).
- **Parallel equals serial.** A batch at `--jobs 8` produces the same per-run
  trace hashes and the same batch manifest bytes as `--jobs 1`; the tick itself
  is never parallelized.
- **Honest aggregation.** Every reported number resolves to the run manifests
  that produced it, and a value that was not applicable or not observed is a
  status with a seed list, never a zero.
- **Fidelity sensitivity is reported, not hidden.** The convergence evidence
  judges every metric of every slice family at Fast (100 ms), Standard (50 ms),
  and Fine (20 ms) over the same seed bank, and states material sensitivity per
  metric for both variants.

## Unvalidated claims (explicitly not made)

1. **No calibration to observed traffic.** Geometry, demand, profiles, and the
   signal plans are authored, not fitted to any intersection. No number in the
   comparison report is a prediction about a real junction, and no absolute
   delay, queue, or flow value is claimed to match field data.
2. **No safety prediction.** Collisions, near misses, violations, time to
   collision, post-encroachment time, and minimum separation are kinematic
   surrogates computed from the model's own trajectories. They are not validated
   against crash records, and no claim of a safety benefit (or harm) follows
   from the comparison's sign. "Collision" here means the kernel's contact
   predicate fired, not an injury or damage outcome.
3. **No design conclusion beyond this model.** The comparison's headline result —
   the east-west through movement's mean control delay is lower under
   `ew_priority` than under `ns_priority` by 10.3 s at the Standard fidelity and
   by 5.8 s at Fine, with the two movements swapping the longer green — is a
   property of this scenario, this demand, and this metric definition v2. It is
   not a recommendation for a real signal plan.
4. **No cross-environment determinism claim.** Reproducibility is claimed for the
   environment the artifacts record: one binary on one platform. The manifests
   pin `event_version` 2, `model_version`, and `build_revision`, and the metric
   artifacts pin `metric_definition_version` 2, so an artifact whose environment
   differs is attributable rather than silently comparable. Bit-identical floats
   across platforms, compilers, or versions are not claimed.
5. **No statistical power claim.** The evidence rests on one ten-seed
   common-random-number bank and reports two-sided 95% Student-t intervals. Ten
   seeds is a demonstration budget, not a power analysis; a paired difference
   whose interval spans zero is not evidence of a direction. Adding seeds is the
   natural next lever and is expected to move the intervals.
6. **No performance claim.** The always-on interaction-metrics pass dominates the
   tick: the recorded release-mode ablation puts it at roughly 18–33 µs per tick
   (54–75% of the tick) on `mixed_interaction_v1` (`perf/tick-phases.json`,
   `perf/profiles/`). No throughput or latency target is claimed for the
   comparison; that residual is owned by
   `[[TAS-051-interaction-metrics-switchable-and-optimized]]`.
7. **No level of service, and no time-lost measure.** See *Still deferred* below.

## Model limitations

- **Agents.** Two modes only: a motor vehicle (an oriented box with a
  longitudinal controller) and a pedestrian (a circle following a route). There
  are no cyclists, no transit vehicles, no heavy or articulated vehicles, and no
  pedestrian groups; those are Phase 2 capabilities, and the corresponding nodes
  are proposed.
- **Control.** Signals are fixed-time: authored phases in cycle order, advanced
  by the simulation clock. There is no actuation, no coordination between
  junctions, no adaptive or demand-responsive control, and no emergency
  preemption. Pedestrian signals are the same kind of fixed-time phase.
- **Motion.** Vehicles follow their guide path under the documented IDM
  controller with its bounds and three hard safety caps — the nearest leader's
  rear, a stop line whose control requires a stop, and an occupied crossing the
  vehicle must yield to — so the bounded model is collision-free by construction
  rather than by a collision resolver. A collision event means the contact
  predicate fired; impact dynamics are not simulated, and the kernel's own
  counter of steps where a cap overrode the comfortable profile
  (`Simulation::emergency_cap_steps`) is **not reported in any run artifact**, so
  the comparison cannot show how often the backstop, rather than the profile,
  shaped a trajectory. The anti-overlap cap is a position clamp
  (`new_speed = min(new_speed, gap / dt)`) with no explicit acceleration limit, so
  a centimetre-scale gap implies an unbounded single-step deceleration: during
  low-speed red-queue close-up behind a stopped leader, `four_leg_signal_v1`
  seed 0 commands a worst step of −28.2549 m/s² (tick 1221, agent 24, 11 cap
  steps) against a 2.256 m/s² comfortable brake. **The accepted 32 m/s² ceiling
  is a regression tripwire for this fixture and seed, not a property of the
  kernel**: `apps/hekate-cli/tests/emergency_cap.rs` fails if the measured
  `four_leg_signal_v1` seed-0 worst step exceeds it, while the position clamp
  itself remains unbounded in principle; the controlled `car_following_v1`
  benchmark engages no cap step, so the residual is scoped to signalized queue
  formation. There is no lane changing, no lateral passing, no wrong-way
  movement, and no narrow-facility behavior.
- **Interaction metrics are bounded readings.** A pair is observed only inside
  the 20 m interaction range, time to collision has a 5 s horizon, a near miss
  is a separation at or below 1 m, and the reported resolutions are 1 µs and
  1 µm. A conflict outside those bounds is not reported, and a false negative
  inside them is bounded by the metric definition's rules, not measured.
- **Time to collision and post-encroachment time carry no mode-pair slices.**
  TTC and PET are reported by movement pair and over the run, but not by mode
  pair: at metric definition v2 only minimum separation carries the
  `vehicle_vehicle` / `vehicle_pedestrian` / `pedestrian_pedestrian` slices.
  Adding per-mode-pair TTC and PET is a future metric-definition revision.
- **Delay is defined by recorded state, not by a reference.** Stopped delay
  accumulates while the kernel's queue predicate holds (speed at or below
  1 mm/s, `QUEUE_STOP_SPEED_MPS`); control delay accumulates over the agent's
  recorded signal-compliance decision. Neither is time lost against a free-flow
  or reference-speed baseline, because no such baseline exists in the model.
- **Queues are agent counts, not spatial lengths.** `maximum_queue_length_agents`
  counts agents holding an open stopped state; no spatial queue length in metres
  is reported.
- **Post-encroachment time needs two occupancies.** PET derives from conflict
  region occupancy intervals, so a pair that never both occupy a region has no
  PET — reported as not observed, not as zero.
- **Whole-batch aggregates are means over seeds and agents.** A run-level metric
  such as `operational.run.mean_control_delay_s` averages over every served agent
  in every seed. It is not an agent-weighted or vehicle-hours-weighted measure,
  and a small aggregate difference can hide a large movement-level one — as it
  does here.

## Boundaries accepted for this increment

The Increment 5 residuals `[[TAS-045-phase-1-increment-6-first-useful-release-demonstration]]`
dispositioned are settled as follows, and none of them gates this increment:

- **The always-on interaction-metrics pass is unoptimized.** Owned by
  `[[TAS-051-interaction-metrics-switchable-and-optimized]]` with the recorded
  ablation as its baseline. Out of scope here; the comparison's absolute run
  time is therefore not a claim.
- **Aggregation disaggregation was narrower than the run artifact's** (slice-F
  finding F6). Closed in `[[TAS-048-increment-6-run-and-comparison-report]]`:
  `mode_event_slices` and `agent_movement_slices` carry the event families by
  mode and movement, and this increment's convergence report mirrors those
  families per metric.
- **Convergence used one global 5% relative tolerance.** Replaced in `TAS-048` by
  a per-metric tolerance — relative 5% plus one whole unit for a countable
  metric — and the outcome is now reported per metric for both variants.

Still deferred, and explicitly not claimed:

- **Level of service.** No implementation anywhere in the workspace;
  `[[DEF-005-metric-definition-v2]]` does not claim it, exactly as v1 did not.
  The comparison therefore reports delay, queue, and flow values without an LOS
  grade.
- **A free-flow delay reference.** No reference-speed baseline exists in the
  model, so no time-lost measure is reported.
- **Everything Phase 2.** Facilities and narrow modes, lateral passing and
  wrong-way movement, heavy and articulated vehicles, transit service,
  pedestrian groups and pairwise interaction, and mixed-mode safety outputs are
  proposed nodes under `IDX-001` and are not part of this demonstration.

## Boundaries of the evidence itself

- **The variants differ only in scenario data.** The independent variable is the
  fixed-time signal plan (the green split and the walk intervals coordinated to
  it); geometry, paths, portals, regions, rules, demand, and profiles are
  controlled data. The comparison therefore says nothing about geometry
  alternatives. `scenarios/benchmarks/offset_junction_v1.json5` demonstrates
  that a geometrically different junction is authored through scenario data
  alone, but it is not compared here.
- **Most metrics are materially sensitive between the Standard and Fine steps.**
  The verdict reads the standard-to-fine refinement, and the convergence evidence
  finds 168 of 254 metrics materially sensitive for `ew_priority` (174 of 254 for
  `ns_priority`). Absolute values move with the step in both directions: the
  whole-run mean control delay rises from 14.73 s at Standard to 21.13 s at Fine
  (+43%), the counted event total from 1804 to 2736 records (+52%), while the
  collision count falls from 5.9 to 2.6. A value read at one fidelity must
  therefore not be compared with a value read at another, and count metrics
  should not be read as converged at the Standard fidelity. Refining the step
  resolves more of the stop-and-go the profile produces; the Fast fidelity is
  coarser still.
- **All three selected findings are directionally stable at Fine.** The east-west
  and north-south through movements' mean control delay keep their sign at every
  fidelity, with a difference of roughly -10 s and +13 s at Standard and -6 s and
  +10 s at Fine, and the whole-run mean control delay keeps its sign too, with a
  difference under one second at every fidelity (0.20 s at Fast, 0.91 s at
  Standard, 0.99 s at Fine). The aggregate difference is an order of magnitude
  smaller than the movement-level one, so a run-level mean cannot stand in for
  the movement it averages.
- **The convergence evidence is a bounded projection.** It states each fidelity's
  seed table once per variant rather than repeating per-seed pairing for every
  metric, which is what keeps the artifact at about 1.2 MB for 508 metrics; the
  per-metric pairing lists the full convergence report carries add several times
  that. The full per-seed pairing is regenerable by the documented command, and
  the comparison report carries the per-seed run tables.
- **Run directories are not checked in.** The comparison's Standard runs are
  244 KB each, the Fast-fidelity runs 160 KB, and the Fine-fidelity runs 512 KB,
  at the declared sampling policy (events: all; trajectories: every 10th tick,
  capped at 100000 samples). They are regenerable from the checked-in spec and
  seed bank and gitignored, and full trajectories remain an opt-in
  (`--full-trajectories`) that no artifact here takes.
- **The comparison report's per-run manifest hashes cannot be re-derived.** The
  per-seed `manifest_sha256` values the comparison report carries name runs under
  `experiments/increment6_signal_timing_v1/runs/`, which is gitignored, so they
  cannot be recomputed from checked-in files; their attribution rests on the
  report's construction and `scripts/reproduce-increment6.sh`.
- **The golden traces cover the canonical event stream only.** The checked-in
  goldens are the canonical traces of the two variants at seed 1 over 1200 steps
  (one 58 s signal cycle) plus the walking skeleton's trace. Sampled
  trajectories, the scene projection, and the renderer cell/Kitty fixtures have
  their own goldens, and the Increment 6 variants are not part of the scene
  golden.
