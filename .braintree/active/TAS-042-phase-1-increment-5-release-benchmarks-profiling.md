---
context_rev: 1
priority: P1
updated: 2026-09-13T02:51:00Z
summary: Slice E of Phase 1 Increment 5 records release-mode end-to-end benchmarks, the always-on interaction-metrics cost by ablation and by sampled profile, and the swept-TOI state (corrected: the swept cast is on the tick path only as the safety monitor's contact confirmation, which no behavioral consumer uses) as the increment baseline before any optimization.
next: Verify the five gates on the final tree and resolve the node with per-criterion evidence.
---

# Outcome

Record the increment's performance baseline before optimization. This slice
measures; it must not optimize or rewire anything.

- A reproducible release-mode end-to-end benchmark harness (a script or a
  `--release` binary/bench) that measures, for the representative scenarios:
  simulated seconds per wall second, agent-steps per second, and output bytes
  per simulated hour, with the machine/hardware/OS/toolchain recorded.
- The always-on interaction-metrics cost, measured as its share of the tick
  (the pass versus the rest of the tick) on the mixed benchmark, as the
  Increment 5 baseline. Increment 4 recorded roughly 22 microseconds per tick
  for the pass against roughly 7 microseconds for the rest of the tick; confirm
  or correct that with an independent measurement and record the method.
- The swept time-of-impact state: the swept cast is on the tick path exactly
  once, as the safety monitor's per-tick contact confirmation in
  `safety::scan_pair`, gated behind the near-miss Lipschitz certificate; no
  behavioral consumer (control, admission, signalling, yielding) depends on a
  swept cast, and `time_of_impact` is otherwise called only from fixtures and
  tests. Record that as the baseline; do not wire anything further in.
- Profiler capture(s) of the release-mode hot path with a documented,
  reproducible capture procedure. If no system profiler is available on this
  machine, record a fallback (for example a self-timing breakdown or a sampling
  harness) and state explicitly what could not be captured rather than claiming
  a capture that did not happen.
- A machine-readable performance artifact committed under `baselines/` or a new
  `perf/` directory, additive and not asserted by any test.

No optimization: no kernel behavior change, no metric-value change, no
golden/baseline regeneration, and no dependency change in `tangle-model` or
`tangle-sim`. The canonical trace, trace golden, trace hash golden, and Phase 1
baseline stay byte-identical. `EVENT_VERSION` stays 2.

If the slice outgrows one session, land the benchmark harness and its artifact
first, commit with `Refs TAS-042`, leave the node `active` with a concrete `next`
naming the profiler captures, and report.

# Done when

- A reproducible release-mode benchmark harness runs and writes a
  machine-readable performance artifact with the measured numbers plus the
  machine/hardware/toolchain and the method.
- The interaction-metrics pass cost is measured and recorded as the increment
  baseline, with the measurement method.
- The swept-TOI-is-not-ticked state is recorded as the baseline.
- Profiler capture(s) exist with a documented reproducible procedure, or a
  fallback plus an explicit statement of what could not be captured.
- No optimization or behavior change landed; the canonical trace, goldens, and
  Phase 1 baseline are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

`PHASE_1_PLAN.md` Increment 5 delivers "release-mode end-to-end benchmarks and
profiler captures before optimization", and its performance-test section asks
for agent-steps per second, simulated seconds per wall second, and bytes per
simulated hour. Increment 4 left the always-on interaction-metrics pass and the
unticked swept-TOI capability as recorded residuals; this child measures them as
the baseline and does not optimize or rewire either. `baselines/phase1/` holds
the existing machine performance artifact.

# Result

Landed in one session; nothing was deferred. Claimed by `worker` for 14400 s
with base hash
`e2a9ed625a7315e0869e07fe04b0a3a5bcf3aeefede9af5d6e09610d7383af20`
from `braintree hash TAS-042`; this node moved `.braintree/proposed/` →
`.braintree/active/` as part of the claimed edit. Write set actually used:
`scripts/**` (four new scripts), `perf/**` (new), `apps/tangle-cli/tests/**`
(one new ignored test target), and this node's own transition. `baselines/**`
was not touched at all: the new artifact is additive under `perf/`. No
`Cargo.toml`, no `Cargo.lock`, no kernel/schema/golden/existing-baseline change,
and no new dependency.

## What landed

- `apps/tangle-cli/tests/release_benchmark.rs` — the end-to-end harness. An
  ignored test, so the five gates never pay for a wall-clock measurement, and
  its only assertions are structural (release profile in use, runs completed):
  no test asserts a timing number. It measures six scenarios over one simulated
  hour each, recording simulated seconds per wall second, agent steps per
  second, canonical trace bytes per simulated hour, and immutable run-directory
  bytes per simulated hour, plus the machine, toolchain, and method.
- `scripts/bench-release.sh` — the documented entry point: builds the release
  profile and runs the harness.
- `perf/release-bench.json` — the end-to-end artifact.
- `scripts/measure-tick-phases.sh` — the ablation that splits the tick into the
  interaction-metrics pass and the rest, plus the interaction-metrics artifact
  `perf/tick-phases.json`.
- `scripts/capture-profile.sh` and `scripts/profile-symbols.py` — the sampled
  profiler capture and the fold of a capture into a tick-phase table, plus
  `perf/profiles/mixed_interaction_v1-release.{sample.txt,summary.txt,run.txt}`.
- `perf/README.md` — the files, the machine, each method, its limitations, and
  what could not be captured.

## Measured numbers

Host: macOS 13.7.8 (22H730), Intel Core i5-7600K @ 3.80 GHz, 4 logical CPUs,
x86_64, rustc/cargo 1.98.1, release profile (no LTO, `debug = false`). Captured
2026-09-13 UTC. Full tables and caveats in `perf/README.md`.

End-to-end, one simulated hour (72 000 Standard steps) per scenario:

| scenario | simulated s per wall s | agent steps/s | µs/tick | trace bytes/hour | run-dir bytes/hour |
|---|---|---|---|---|---|
| `walking_guide_v1` | 391 699 | 76 599 | 0.13 | 1 321 | 5 743 |
| `car_following_v1` | 710 | 215 778 | 70.43 | 237 986 | 2 113 807 |
| `red_light_compliance_v1` | 3 333 | 258 081 | 15.00 | 373 821 | 657 893 |
| `pedestrian_crossing_v1` | 1 240 | 217 399 | 40.31 | 865 973 | 1 548 941 |
| `four_leg_signal_v1` | 1 926 | 199 083 | 25.97 | 1 264 328 | 956 514 |
| `mixed_interaction_v1` | 1 130 | 239 003 | 44.23 | 2 169 350 | 2 064 969 |

Interaction-metrics pass on `mixed_interaction_v1`, A/B ablation in a throwaway
copy of HEAD (µs/tick from the median pass):

| window | tick | rest of tick | pass | pass share | pass / rest |
|---|---|---|---|---|---|
| 2 500 ticks (125 s) | 23.95 | 5.97 | 17.98 | 75.1 % | 3.01 |
| 72 000 ticks (1 h) | 47.89 | 14.69 | 33.19 | 69.3 % | 2.26 |
| 300 000 ticks (4.2 h) | 61.77 | 28.54 | 33.24 | 53.8 % | 1.16 |

Increment 4's ~22 µs pass against ~7 µs rest is **confirmed in shape and
corrected in absolute terms**: the short window measures 17.98 against 5.97, a
3.0:1 split where Increment 4 recorded 3.1:1, about 20 % lower in µs/tick on this
host and build. The share is window-dependent because the rest of the tick grows
faster than the pass as the run accumulates agents: `AgentStore::len` is the
alive-flag vector's length and a slot is never reclaimed, so the per-tick loops
over `0..agents.len()` in `sim.rs`, `metrics.rs`, `safety.rs`, and `index.rs`
scan the high-water mark (938 agents admitted, 9 alive, over the hour). The
one-hour window is the increment baseline; the short window is not
representative of a long run.

Sampled profile of the release binary at the 300 000-tick window (`sample`, 1 ms,
8 s, folded by `scripts/profile-symbols.py`): 5 741 tick samples, of which the
interaction-metrics pass is 2 944 (51.3 %) and the rest of the tick 2 797
(48.7 %) — independently agreeing with the ablation's 53.8 % at the same window
within 2.5 points. The largest leaf frame is `BroadPhase::candidates_in_aabb` at
1 913 samples (33 % of the tick), and inside the pass the candidate query is
1 898 of 2 944 samples (64 %) against 458 (16 %) for the TTC bisection. That
**corrects the profiling candidate Increment 4 deferred to this slice**, which
named the TTC bisections as the pass's dominant cost: the dominant cost is the
candidate query the metrics pass and the safety monitor each run every tick.

## Swept time-of-impact state (corrected)

The swept cast is on the tick path exactly once, as the safety monitor's per-tick
contact confirmation in `safety::scan_pair` (`crates/tangle-sim/src/safety.rs:281`,
reached every tick from `SafetyMonitor::observe`), gated behind the near-miss
Lipschitz certificate at `safety.rs:273`. No behavioral consumer — control,
admission, signalling, yielding — depends on a swept cast, so no tick dynamics
depend on a sweep, and `time_of_impact` is otherwise called only from fixtures
and tests (`crates/tangle-sim/tests/swept_queries.rs`, `tests/safety_events.rs`).
The metrics pass uses its own TTC bisection over the same convex-clearance
helpers (`clearance_rate`, `first_fraction`) and does not call `time_of_impact`.

This node and its parent were handed with the premise "no tick consumer uses a
cast, so it is not on the tick path"; that sentence is wrong, and this slice
records the corrected state instead of repeating it. The coordinator owns the
matching correction in [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]];
this slice wired nothing further in and changed no kernel file.

## Preservation

No optimization and no behavior change landed. The delivered tree changes no
file under `crates/`, no golden, and no existing baseline: `baselines/phase1/` is
byte-identical, `tests/golden/` is byte-identical, `EVENT_VERSION` stays 2, and
the five gates pass. The one kernel edit in this slice is the ablation, and it
exists only inside a temporary `git archive HEAD` copy: the script asserts the
repository's `crates/` is clean before and after, and the ablated build reports
the same event count and event-stream hash as the committed build in every
window (`perf/tick-phases.json`, `integrity`).
