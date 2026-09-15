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

- `release-bench.json` — `apps/hekate-cli/tests/release_benchmark.rs` over six
  scenarios, one simulated hour each: simulated seconds per wall second, agent
  steps per second, canonical trace bytes per simulated hour, and immutable
  run-directory bytes per simulated hour, with every timed pass, the machine,
  and the method.
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
