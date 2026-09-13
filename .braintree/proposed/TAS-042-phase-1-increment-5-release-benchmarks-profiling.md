---
context_rev: 1
priority: P1
updated: 2026-09-13T02:35:00Z
summary: Slice E of Phase 1 Increment 5 records release-mode end-to-end benchmarks and profiler captures BEFORE any optimization, including the always-on interaction-metrics cost and the unticked swept-TOI state as the increment baseline.
next: Read the existing performance baseline and CLI release path, then add a reproducible release-mode benchmark harness and profiler captures with a machine-readable artifact.
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
- The swept time-of-impact state: the query exists and is exercised by the
  safety/metrics passes and fixtures, but no tick consumer uses a cast, so it is
  not on the tick path. Record that as the baseline; do not wire it in.
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

Pending.
