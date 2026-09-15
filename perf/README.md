# Phase 1 Increment 5 performance baseline

This directory records what Phase 1 Increment 5 measures **before** any
optimization: the release-mode end-to-end benchmarks, the always-on
interaction-metrics pass as a share of the tick, and a sampled profile of the
release hot path. It is the slice-E evidence for
[[TAS-042-phase-1-increment-5-release-benchmarks-profiling]].

Everything here is evidence about one host at one time. Wall-clock time differs
between hosts and builds, so **no test asserts any number in this directory** and
no gate reads it. `baselines/phase1/performance.json` stays the Phase 1 baseline;
nothing in this directory replaces or regenerates it.

## Files

- `release-bench.json` — `apps/hekate-cli/tests/release_benchmark.rs` over seven
  scenarios, one simulated hour each: simulated seconds per wall second, agent
  steps per second, canonical trace bytes per simulated hour, and immutable
  run-directory bytes per simulated hour, with every timed pass, the machine,
  and the method. The first six rows are the Phase 1 Increment 5 capture; the
  seventh is the Increment 2 representative mixed-mode profile, documented in
  its own section below. The six Phase 1 rows are byte-identical to the original
  capture (only the seventh row was appended), so the file mixes two capture
  dates: `generated_unix_s` remains the Phase 1 capture's.
- `tick-phases.json` — `scripts/measure-tick-phases.sh`: the
  interaction-metrics pass against the rest of the tick on
  `scenarios/benchmarks/mixed_interaction_v1.json5`, measured by A/B ablation at
  three windows, with the ablation's integrity checks.
- `profiles/mixed_interaction_v1-release.sample.txt` — the raw macOS `sample`
  capture of the release binary running that scenario for 300 000 ticks.
- `profiles/mixed_interaction_v1-release.summary.txt` —
  `scripts/profile-symbols.py` folding that capture into the tick-phase table.
  This is the fallback the profile is read through: `sample`'s own output is a
  mangled call graph, not a table.
- `profiles/mixed_interaction_v1-release.run.txt` — the profiled run's stderr,
  including the trace hash of the run that was sampled.

## Machine

| | |
|---|---|
| OS | macOS 13.7.8 (build 22H730) |
| CPU | Intel(R) Core(TM) i5-7600K CPU @ 3.80GHz, 4 logical CPUs |
| Arch | x86_64 |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1 |
| Build | release profile: no LTO, `debug = false`, the pinned `rust-toolchain.toml` |
| Isolation | single-threaded runs, no CPU pinning, no CPU-isolating sandbox; the host was otherwise idle but not quiet |
| Captured | 2026-09-13 UTC |

## Method: end-to-end release benchmarks

```sh
scripts/bench-release.sh
```

The harness is an ignored test, so the five gates never pay for a wall-clock
measurement; the script builds the release profile and runs it with
`--ignored --nocapture`. Per scenario it warms up 2 000 ticks, measures three
independent passes of the same fixed-step loop with `std::time::Instant` around
the loop only, then runs the recording path once to size the output. Scenario
load, warm-up, artifact writing, and process start-up are outside the measured
interval; the ratios are derived from the **median** pass. Agent steps are live
agents summed over the run, the same quantity
`baselines/phase1/performance.json` reports. Output bytes are the canonical JSON
Lines trace and the immutable run directory the default sampling policy writes
(every event, trajectories sampled at stride 10 ticks up to 100 000 rows), each
extrapolated to one simulated hour.

Measured over one simulated hour (72 000 Standard steps):

| scenario | simulated s per wall s | agent steps/s | µs/tick | trace bytes/hour | run-dir bytes/hour |
|---|---|---|---|---|---|
| `walking_guide_v1` | 391 699 | 76 599 | 0.13 | 1 321 | 5 743 |
| `car_following_v1` | 710 | 215 778 | 70.43 | 237 986 | 2 113 807 |
| `red_light_compliance_v1` | 3 333 | 258 081 | 15.00 | 373 821 | 657 893 |
| `pedestrian_crossing_v1` | 1 240 | 217 399 | 40.31 | 865 973 | 1 548 941 |
| `four_leg_signal_v1` | 1 926 | 199 083 | 25.97 | 1 264 328 | 956 514 |
| `mixed_interaction_v1` | 1 130 | 239 003 | 44.23 | 2 169 350 | 2 064 969 |

Reading the table:

- Every scenario stays far faster than wall-clock at the coarsest and the finest
  of the six; the slowest is `car_following_v1` at 710 simulated seconds per wall
  second, still 1 200x real time.
- `walking_guide_v1` is the empty-tick floor, not a traffic load: its six
  constant-speed cars leave the 120 m path and nothing replaces them, so 704 of
  its 72 000 ticks step a live agent.
- The run directory is dominated by the sampled trajectories. The canonical
  trace is 0.2–2.2 MB per simulated hour; the run directory is 0.66–2.1 MB per
  simulated hour for five scenarios, and 5.7 KB for the walking skeleton.
- Host variance is real: three passes of one scenario in one capture agreed
  within a few percent for most rows, but re-capturing the whole sweep nine
  minutes later moved individual scenarios by up to 25 % (`red_light_compliance_v1`
  20.4 → 15.0 µs/tick) and moved every row by 5–10 %. Read the table as one
  host's order of magnitude, not as a benchmark contract.

## Method: the interaction-metrics pass against the rest of the tick

```sh
scripts/measure-tick-phases.sh
```

The pass has no switch: `InteractionMetrics::begin_tick` and `observe` are
crate-private and `Simulation::advance_one_tick` always calls them, so the split
is an A/B ablation. The script unpacks `git archive HEAD` into a temporary
directory, writes a throwaway `crates/hekate-sim/examples/tick_phase_bench.rs`
driver there, builds the kernel there, then removes the two `self.metrics.*`
calls from **that copy's** `crates/hekate-sim/src/sim.rs` and rebuilds. It times
the identical bare step loop in both builds, five passes per window, and reports
the minimum and the median. **No file of this repository is modified**: the
script fails if the repository's `crates/` is dirty before or after, and the two
builds must report the same event count and event-stream hash (both did, in every
window).

Limitations, stated rather than hidden: removing the calls lets the compiler
re-codegen the rest of the loop, so "rest" is the rest of the tick as compiled
*without* the pass; the ablation isolates the metrics pass only, not the safety
pass; and the two builds are timed back to back, so the minimum is the less
drift-sensitive statistic.

Measured on `mixed_interaction_v1` (all figures µs per tick, from the median
pass; the artifact also records the ones derived from the minimum):

| window | simulated | tick | rest of tick | metrics pass | pass share | pass / rest |
|---|---|---|---|---|---|---|
| 2 500 ticks | 125 s | 23.95 | 5.97 | 17.98 | 75.1 % | 3.01 |
| 72 000 ticks | 1 h | 47.89 | 14.69 | 33.19 | 69.3 % | 2.26 |
| 300 000 ticks | 4.2 h | 61.77 | 28.54 | 33.24 | 53.8 % | 1.16 |

- **Increment 4's shape is confirmed, its absolute numbers are window-specific.**
  [[TAS-030-phase-1-increment-4-geometry-queries-safety-events]] recorded roughly
  22 µs for the pass against roughly 7 µs for the rest (a 3.1:1 split). At the
  short window this artifact measures 17.98 against 5.97, a 3.0:1 split with the
  same pass-dominated shape but absolutes about 20 % lower on this host and in
  this build.
- **The pass share falls as a run gets longer, because the rest of the tick grows
  faster.** From 125 simulated seconds to 4.2 simulated hours the pass goes
  17.98 → 33.24 µs while the rest goes 5.97 → 28.54 µs, even though the average
  live-agent count barely moves (8.05 in the short window, 9.65 in the long one).
  `AgentStore::len` is the length of its alive-flag vector and a slot is never
  reclaimed — a spawn takes `AgentId::from_index(self.alive.len())`
  (`crates/hekate-sim/src/agent.rs:130`) — so the per-tick loops over
  `0..agents.len()` cost more as a run accumulates agents: the kernel's agent
  loop in `sim.rs`, and the `begin_tick`/`index_bodies` loops in `metrics.rs`,
  `safety.rs`, and `index.rs`. `mixed_interaction_v1` admits 938 agents over its
  hour and keeps 9 alive. The increment's one-hour window is the number to carry
  forward; the short window is not representative of a long run.

## Method: sampled profile of the release hot path

```sh
scripts/capture-profile.sh
```

`scripts/capture-profile.sh` builds the release binary, starts
`scenarios/benchmarks/mixed_interaction_v1.json5` for 300 000 ticks with its
trace going to `/dev/null`, and attaches `sample <pid> 8`, this host's 1 ms
sampler, one second in. `scripts/profile-symbols.py` folds the capture into
`profiles/mixed_interaction_v1-release.summary.txt`: the samples under
`Simulation::step`, every direct child of it with its share, the tick body's own
self time, the interaction-metrics pass as one line, and the frames inside that
pass with their share of it.

Of 5 741 tick samples in that capture:

| frame (subtree) | samples | share of tick |
|---|---|---|
| `hekate_sim::metrics::InteractionMetrics::observe` | 2 786 | 48.5 % |
| `hekate_sim::safety::SafetyMonitor::observe` | 849 | 14.8 % |
| `Simulation::step` self (inlined and non-frame work) | 1 199 | 20.9 % |
| `Simulation::step_vehicle` | 293 | 5.1 % |
| `SafetyMonitor::begin_tick` | 171 | 3.0 % |
| `InteractionMetrics::begin_tick` | 158 | 2.8 % |
| **interaction-metrics pass (`hekate_sim::metrics::*`)** | **2 944** | **51.3 %** |
| rest of the tick | 2 797 | 48.7 % |

The profile's 51.3 % pass share and the ablation's 53.8 % at the same 300 000-tick
window are independent methods agreeing within 2.5 points, which is the
cross-check this slice wanted. The two are not the same measurement: sampling
attributes inlined callee time to the enclosing frame, so the 20.9 % against
`Simulation::step` self includes work belonging to the loops it inlines.

The largest single leaf frame is the broad-phase candidate query,
`hekate_sim::index::BroadPhase::candidates_in_aabb`, at 33 % of the tick
(1 913 samples) — and inside the metrics pass the candidate query
(`SweptBroadPhase::candidate_pairs`) is 1 898 of the pass's 2 944 samples (64 %)
against 458 (16 %) for the TTC bisection (`time_to_collision`). Both within-pass
numbers are rows of the folded table's `within-pass frames` section, which folds
the rows below the pass's frames by terminal symbol name and reports each one's
share of the pass; folding by terminal name is what puts the bisection's inlined
`first_fraction` closure (174 samples) with the bisection (284) rather than
reporting them apart. That **corrects the profiling candidate Increment 4
deferred to this slice**: it named the TTC bisections as the dominant cost of
the pass, and on this evidence the pass's dominant cost is the candidate query
both the metrics pass and the safety monitor run every tick.

## Swept time-of-impact state (baseline)

The swept cast is on the tick path exactly once, as the safety monitor's per-tick
contact confirmation in `safety::scan_pair`
(`crates/hekate-sim/src/safety.rs:281`, reached every tick from
`SafetyMonitor::observe`), gated behind the near-miss Lipschitz certificate at
`safety.rs:273`. No behavioral consumer — control, admission, signalling,
yielding — depends on a swept cast, so no tick dynamics depend on a sweep, and
`time_of_impact` is otherwise called only from fixtures and tests
(`crates/hekate-sim/tests/swept_queries.rs`, `tests/safety_events.rs`). The
metrics pass uses its own TTC bisection over the same convex-clearance helpers
(`clearance_rate`, `first_fraction`); it does not call `time_of_impact`. This
slice wires nothing further in.

The node TAS-042 was handed with the premise "no tick consumer uses a cast, so it
is not on the tick path"; the line above is the corrected record, and this slice's
`# Result` says the same thing.

## What could not be captured

- `sample` is the only profiler used. This host has **no `samply`**, **no
  `cargo instruments`**, and **no `perf`**, so no flamegraph, no sampling beyond
  `sample`'s call graph, and no hardware-counter or off-CPU data was captured.
  `xcrun` and `dtrace` exist; `dtrace` needs root and `xcrun xctrace` writes an
  Instruments bundle whose profiles need Instruments to read, so neither was
  used for this baseline.
- The release profile has no debug info and no forced frame pointers, so
  `sample` resolves leaf PCs and symbol names but not inlined frames or line
  numbers, and its stack attribution is the call graph the capture shows. The
  numbers above are therefore an attribution, not a line-level profile.
- No allocation-count, cache-miss, or instruction-count measurement was taken.
- The sweep is one host, one toolchain, one seed, unpinned; the within-capture
  and between-capture spread recorded above is the honest error bar.

## Regenerate

From the repository root, each command is self-contained and bounded:

```sh
scripts/bench-release.sh            # perf/release-bench.json
scripts/measure-tick-phases.sh      # perf/tick-phases.json (builds two kernels in a temp dir)
scripts/capture-profile.sh          # perf/profiles/* (macOS only)
```

## Method: the Increment 2 representative mixed-mode profile

`scenarios/phase2/inc2/mixed_mode_profile_v2.json5` is the Increment 2
representative mixed-mode **evidence workload** ([[TAS-110]]): the three
Increment 2 wheeled modes — passenger cars, bicycles, and scooters — share one
two-way corridor as two adjacent one-way 7.5 m lanes, so the kernel's lateral
tactics (`overtake` for the car, `pass` for the bicycle and scooter) and the
opposing traversal run together at one declared density. It is not the final
mixed-mode release fixture (`mixed_release_v2`, Increment 7) and no row of
`docs/benchmark-matrix.md` names it. Its seventh row in `release-bench.json` is
the row this section reads.

### Machine

| | |
|---|---|
| OS | macOS 13.7.8 (build 22H730) |
| CPU | Intel(R) Core(TM) i5-7600K CPU @ 3.80GHz, 4 logical CPUs |
| RAM | 64 GB |
| Arch | x86_64 |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1 (797e8a9bc 2026-08-05) |
| Build | release profile: no LTO, `debug = false`, the pinned `rust-toolchain.toml` |
| Captured | 2026-09-15 UTC |

The same host as the Phase 1 capture above, so the comparison below is a
same-hardware comparison; no cross-hardware equivalence is claimed.

### Method

The seventh row runs the same `release_benchmark` harness as the six rows above:
one simulated hour (72 000 Standard steps at 0.05 s), a 2 000-tick warm-up, three
independent timed passes from a fresh `Simulation` each, then one un-timed
recording pass. `std::time::Instant` brackets the fixed-step loop only, and the
ratios are derived from the **median** pass.

### Measured (one simulated hour)

| pass | wall s | µs/tick | sim s per wall s |
|---|---|---|---|
| 1 | 111.431 | 1 547.65 | 32.307 |
| 2 | 111.422 | 1 547.53 | 32.309 |
| 3 | 111.502 | 1 548.64 | 32.286 |
| **min** | **111.422** | **1 547.53** | **32.309** |
| **median (headline)** | **111.431** | **1 547.65** | **32.307** |

Companion quantities from the same row: 702 894 agent steps (9.76 live agents on
average, since agent steps are live agents summed over the run), 6 308 agent
steps/s, 460 spawned / 449 despawned / 11 remaining (the corridor flows),
770 144 canonical trace bytes per simulated hour, and 3 033 055 run-directory
bytes per simulated hour — 2 952 056 of them the sampled `trajectories.parquet`.

### Dominant measured work (headline)

`scripts/capture-profile.sh scenarios/phase2/inc2/mixed_mode_profile_v2.json5 300000 8`
attached `sample` to 300 000 ticks of the release binary. Of 5 737 tick samples,
5 491 (**95.7 %**) fall inside `hekate_sim::sim::Simulation::predict_candidate`:

| frame (subtree) | samples | share of tick |
|---|---|---|
| `hekate_sim::sim::Simulation::predict_candidate` | 5 491 | 95.7 % |
| `hekate_sim::metrics::InteractionMetrics::observe` | 106 | 1.8 % |
| `alloc::vec` (`spec_from_iter_nested`, `hekate_sim::sim`) | 72 | 1.3 % |
| `hekate_sim::prediction::predict_with_band_bounds` | 30 | 0.5 % |
| `hekate_sim::safety::SafetyMonitor::observe` | 19 | 0.3 % |
| `hekate_sim::sim::Simulation::predict_maneuver` | 14 | 0.2 % |
| interaction-metrics pass (`hekate_sim::metrics::*`) | 107 | 1.9 % |
| rest of the tick | 5 630 | 98.1 % |

The pass is only 1.9 % here: on this workload the dominant work is the
predictor's candidate scan, not the metrics pass. Its top self-time frames are
the per-candidate clearance primitives — `closest_point_on_box` (1 275),
`body_contact_normal` (1 188), `box_box_least_overlap_axis` (944),
`body_clearance_m` (828), `sincos_stret` (493) — which is why `predict_candidate`
scales with the **population**, not with the tick count.

### Cost curve and the unsupported density

Bounded release-mode windows on the same binary (`hekate-cli run <fixture>
--ticks N --output /dev/null`, min of two passes):

| ticks | simulated minutes | wall s | µs/tick |
|---|---|---|---|
| 2 000 | 1.7 | 2.02 | 1 010 |
| 10 000 | 8.3 | 17.42 | 1 742 |
| 30 000 | 25.0 | 44.67 | 1 489 |
| 60 000 | 50.0 | 86.29 | 1 438 |
| 72 000 (harness hour row) | 60.0 | 111.43 | 1 548 |

The 300 000-tick profiled run took about 8 min 20 s (~500 s, inferred from the
artifact mtimes), i.e. ~1.67 ms/tick. Within the **flowing** regime the per-tick
cost is therefore roughly flat at 1.4–1.7 ms: the corridor reaches a bounded
steady-state population (the hour admits 460 bodies and keeps 11), so cost does
not grow without bound.

**The pathological, unsupported density is the earlier shared-facility draft.**
The fixture's first draft put all six modes on one shared 10 m two-way facility
with passenger cars at 300 arrivals/h per direction (960 arrivals/h in total). At
that load the facility did not flow — 2 000 ticks admitted 28 bodies and released
none — the live population grew with the clock, and per-tick cost grew
**superlinearly** with it, measured on the release binary:

| ticks | wall s | µs/tick |
|---|---|---|
| 500 | 0.22 | 440 |
| 1 000 | 2.12 | 2 120 |
| 2 000 | 11.6 | 5 800 |
| 4 000 | 37.2 | 9 300 |

95 % of that cost was inside `Simulation::predict_candidate`. A uniform
hour-long sweep of that density is therefore **infeasible** — the one-hour pass
alone would take hours, not the 111 s the reshaped workload takes — which is why
the workload was reshaped to the per-direction lanes above. The 960 arrivals/h
shared-facility density is labelled **unsupported**: no completed hour-long run
backs any performance claim above the flowing density. The reshaped workload
still exposes the same dominant work, because `predict_candidate` scans the
candidate population every tick.

### Instrumented counters, presenter frame time, and the lateral ablation

The performance counters the profile reads are additive and non-behavioural: the
kernel gains a broad-phase candidate counter in `crates/hekate-sim/src/index.rs`
and a prediction counter in `crates/hekate-sim/src/sim.rs`, exposed through
`Simulation::performance_counters()` and `Simulation::reset_performance_counters()`.
They are thread-scoped and never serialized; no decision, event, trace byte,
golden, or metric reads them, so a run is byte-identical with and without them.
The bounded release measurement runs the profile and its lateral-disabled twin
for 5 000 ticks at the fixture's seed-bank entry 11 after a 500-tick warm-up,
three timed passes each:

```sh
cargo test --release -p hekate-sim --test performance_counters -- --ignored --nocapture
```

| quantity | enabled | disabled (no lateral) |
|---|---|---|
| µs/tick, min / median | 861.4 / 863.8 | 25.4 / 25.5 |
| µs/tick, three passes | 863.8, 866.1, 861.4 | 25.4, 25.5, 25.8 |
| agent steps (5 000 ticks) | 39 658 | 43 244 |
| agent steps / tick | 7.932 | 8.649 |
| broad-phase candidates | 164 585 | 186 284 |
| broad-phase candidates / agent-step | 4.150 | 4.308 |
| `predict_candidate` evaluations (`predictions`) | 7 672 | 0 |
| predictions / agent-step | 0.193 | 0.000 |
| prediction candidate bodies | 52 032 | 0 |
| prediction candidate bodies / agent-step | 1.312 | 0.000 |
| µs per agent-step | 108.9 | 2.95 |

Reading:

- **Broad-phase candidate volume is population-driven, not lateral-driven.**
  4.15 candidates per agent-step enabled against 4.31 disabled: the 5 000-tick
  broad-phase work tracks the live population, not the lateral subject.
- **Prediction work is the tactical cadence.** The enabled run evaluates the
  predictor 0.193 times per agent-step — an agent that carries a lateral route
  state predicts on about one step in five — and scans 1.312 candidate obstacle
  bodies per agent-step. The disabled twin never evaluates it (0), because no
  agent declares a lateral tactic.
- **The lateral machinery is 97.1 % of the bounded per-tick cost** (median
  863.8 against 25.5 µs/tick; 108.9 against 2.95 µs per agent-step). On this
  workload the whole Increment 2 lateral subject — tactic selection, the
  predictor's candidate scan, and commitment — is essentially the tick, which
  matches the sampled profile's 95.7 % in `predict_candidate`.
- **The disabled twin admits more agent steps** (43 244 against 39 658) because
  without passing a car queues behind a slower leader over the whole corridor;
  the per-tick comparison is at nearly equal live population (8.65 against
  7.93 agent steps per tick).
- **This is a bounded-window ablation, not the hour.** The 5 000-tick window is
  250 simulated seconds of the flowing regime, not the one-hour pass; it measures
  the same corridor with and without the lateral subject at one fixed seed and
  window. Only the enabled hour is benchmarked.

End-to-end corroboration, `hekate-cli run` over the same 5 000 ticks and seed 11
with the trace to `/dev/null`: enabled 3.651 s wall (730 µs/tick including
scenario load and trace writing), disabled 0.138 s (27.6 µs/tick). Trace hashes
`5bbf685ef4dcae740ff464d51c75c768910ded57548879227ad18d7349395f9a` (enabled) and
`a3798fe57441643071bae5ea3e6fdd5457df96965330d23ff93dbd196b6d2dc6` (disabled).

Presenter frame time is headless: no Bevy viewer starts and no backend draws. The
shared, backend-agnostic projection `PresentationController::project` is timed
alone over 5 000 distinct populated snapshot pairs of the same 5 000-tick window
(the snapshots are built outside the timed interval):

```sh
cargo test --release -p hekate-present --test presenter_frame_time -- --ignored --nocapture
```

| quantity | measured |
|---|---|
| µs/frame | 0.735 |
| bodies/frame | 7.998 |
| frames | 5 000 |

The projection costs well under a microsecond per frame at this population —
four orders of magnitude below the ~16 ms a 60 Hz frame allows — so a viewer's
frame budget is dominated by rendering, not by this shared layer.

### Increment 2 budgets

Derived from the measured baseline with the documented 5–10 % host spread as the
margin (ceiling = measured × 1.10), stated per window because the per-tick cost
is window-specific. These are regression ceilings for later optimization work
against **this** baseline: a pass below its ceiling is not a target, and no single
pass is a budget (each row's spread is in the tables above).

| budget | measured baseline | ceiling (+10 %) |
|---|---|---|
| per-tick wall, hour window | 1 547.65 µs/tick median (min 1 547.53; pass spread 0.04 %) | ≤ 1 702 µs/tick |
| per-tick wall, 5 000-tick window | 863.8 µs/tick median (min 861.4; pass spread 0.55 %) | ≤ 950 µs/tick |
| broad-phase candidates / agent-step | 4.150 | ≤ 4.6 |
| predictions / agent-step | 0.193 | ≤ 0.21 |
| prediction candidate bodies / agent-step | 1.312 | ≤ 1.44 |
| lateral-disabled per-tick floor | 25.5 µs/tick median | ≤ 28 µs/tick |
| presenter frame | 0.735 µs/frame | ≤ 0.81 µs/frame |

The bounded rows are seed 11 and the hour row is the harness root seed 0; the
budgets are per named window on this fixture and host and do not transfer to the
Phase 1 scenarios or to another machine.

### Comparison against the frozen Phase 1 baseline

Same host, same harness, same release profile; the six rows above are the frozen
Phase 1 Increment 5 capture and the seventh is this Increment 2 row. The new row
is by far the slowest of the seven:

| quantity | Increment 2 row | Phase 1 rows |
|---|---|---|
| µs/tick | 1 547.65 | 0.13–70.43 |
| sim s per wall s | 32.31 | 710–391 699 |
| agent steps/s | 6 308 | 76 599–258 081 |
| trace bytes/hour | 770 144 | 1 321–2 169 350 |
| run-dir bytes/hour | 3 033 055 | 5 743–2 113 807 |

The new row is 22.8× `car_following_v1` (67.91 µs/tick) and 35.1×
`mixed_interaction_v1` (44.06 µs/tick) per tick, and its run directory is the
largest of the seven. The sharper comparison is per live agent: the new row runs
at 9.76 live agents and ~159 µs per live agent per tick, while
`mixed_interaction_v1` runs at 10.57 live agents and ~4.2 µs per live agent — a
~38× per-agent gap, which is the always-on `predict_candidate` scan the lateral
and opposing machinery adds.

`baselines/phase1/performance.json` is the frozen Increment 0 artifact; it is a
**debug-profile** 125/250/625-tick walking-skeleton measurement, not a release
mixed-mode run, so it is not the reference here and is not comparable.

### Profiler and rerun commands

```sh
scripts/capture-profile.sh scenarios/phase2/inc2/mixed_mode_profile_v2.json5 300000 8
python3 scripts/profile-symbols.py perf/profiles/mixed_mode_profile_v2-release.sample.txt
```

The first builds the release binary, runs 300 000 ticks with the trace to
`/dev/null`, attaches `sample <pid> 8`, and writes the three files below; the
second folds the capture into the phase table.

```sh
scripts/bench-release.sh
cargo test --release -p hekate-cli --test release_benchmark -- --ignored --nocapture
```

The harness rewrites `release-bench.json` from scratch (506 s wall, exit 0); to
reproduce this file's mixed provenance, keep the six Phase 1 rows and take only
the `mixed_mode_profile_v2` row.

### Raw artifacts (SHA-256)

| artifact | SHA-256 |
|---|---|
| `perf/release-bench.json` | `a39ab749b3f7892945b363d56e7295ed244584ef2b79ecf3b6ba71be33419bd4` |
| `perf/profiles/mixed_mode_profile_v2-release.sample.txt` | `61afc13087cd23d6c995cd6d77e8376616f0db907987e5124d3305e14b33f5a7` |
| `perf/profiles/mixed_mode_profile_v2-release.summary.txt` | `7a2a3913eaec7ad8536549955b318a6f07413154d98f2a9a494a262995ab1550` |
| `perf/profiles/mixed_mode_profile_v2-release.run.txt` | `7fc1065b93759b25a4a3dc83c9c5f9e124cb8411d4ab9fd4d245be1e33976d05` |
| `scenarios/phase2/inc2/mixed_mode_profile_v2.json5` | `2ded1590994b0060621855095f3626ddc2eb848531e59345353ccb35ae76eb6a` |
| `scenarios/phase2/inc2/mixed_mode_profile_v2_no_lateral.json5` | `12aafb1b5bb3f3bdb499d0e2d9ff44940260f365757b13a97465f60361a63d76` |

The profiled run's canonical trace hash is
`9add5358208c42ed1d4a43bbccb71356f41cbb56b03e2b99e0b4867b66f36baa`.

### Limitations

- The capture is `sample` only, on the host described above, unpinned and not
  repeated under load; the within-capture spread of the three timed passes is
  tight (0.04 %) but the host is otherwise idle and not quiet.
- The standalone-window cost curve includes process start-up and scenario load;
  those are negligible above about 2 000 ticks but are not part of the harness's
  measured interval.
- The 300 000-tick profiled run's wall time is inferred from the artifact mtimes,
  not timed by the harness.
- The release profile has no debug info and no frame pointers, so the capture is
  an attribution to frames, not a line-level profile.
- No headed viewer measurement was taken: presenter frame time is the headless
  shared projection above (0.735 µs/frame), not a Bevy frame.
- The disabled-versus-enabled lateral comparison is a bounded 5 000-tick
  ablation twin at seed 11, not an hour-window A/B; only the enabled hour is
  benchmarked, and the twin's hour cost is not measured.
- The profile is one seed (root seed 0, the harness's) and one run per window.
