---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T07:26:00Z
summary: Prove stream isolation and preserve the Phase 1 baseline.
---

Parent [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]].

# Outcome

Adding an unrelated agent or demand stream cannot perturb another agent maneuver
or wrong-way draws, and the Phase 1 baseline artifacts are unchanged.

# Done when

- Stream-isolation tests add unrelated car and narrow demand and reversed
  declaration order without changing owned maneuver draws or unaffected agent
  traces.
- A falsification probe demonstrates the suite catches draw-order coupling or an
  unkeyed maneuver choice.
- Existing demand, profile, compliance, and perception streams and Phase 1
  baseline artifacts remain unchanged.

# Context

Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns stream isolation
and baseline preservation; reproducibility is the sibling slice.

# Result

Delivered the stream-isolation slice in
`apps/hekate-cli/tests/inc2_determinism.rs`; no production or scenario artifact
changed, so `baseline.rs`, `narrow_determinism.rs`, the Phase 1 goldens and
baselines, and the sim stream suites stay byte-identical.

- `the_inc2_maneuver_stream_stays_isolated_from_unrelated_car_and_narrow_demand`
  compiles a scenario whose focus facility `a` (with its reverse continuance
  `left`) authors `maneuver_policy.wrong_way` and carries a fully non-compliant
  `rider`, beside an unrelated passenger-car road and an unrelated bicycle lane;
  the focus rider's wrong-way entry is driven in-process through
  `Simulation::request_wrong_way_entry`, the only seam that consumes the keyed
  `maneuver` draw. Three runs at seed 7 over 1200 steps (60 s) — focus demand
  alone, focus plus an unrelated car source and an unrelated narrow source
  declared after it, and the same two added sources with their own order
  reversed — must agree on the focus source's own arrival series (9 riders, same
  ticks), on the canonical trace lines of the focus agents admitted before the
  added demand's first id, and on those agents' keyed `maneuver_draw` values.
- `the_stream_isolation_check_flags_unrelated_demand_declared_before_the_focus`
  is the falsification probe: it declares the same unrelated car and narrow
  sources *before* the focus source, moving the focus source's dense demand index
  (the key its `demand` substream is derived from) and admitting unrelated agents
  first, and asserts the same comparisons report the moved arrival series, the
  moved requested agent, its moved trace lines, and its moved keyed draw.

Scope recorded rather than hidden: agent ids are assigned by admission order
(`crates/hekate-sim/src/sim.rs`'s `try_admit` uses `self.agents.len()`), so once
unrelated agents are admitted, the focus agents admitted after them take a
different id and therefore a different per-agent `profile` draw. That is not
draw-order coupling — the focus source's `demand` substream is unchanged, which
the arrival-series comparison proves — so the trace comparison is scoped to the
focus agents admitted before the first unrelated agent (ids 0, 1, 2 here), the
agents the added demand genuinely does not affect. The `maneuver` draw itself is
`pub fn maneuver_draw(root_seed, agent, ordinal)`, a pure function of the seed,
the stable agent id, and the agent's own decision ordinal
(`crates/hekate-sim/src/wrong_way.rs`); the added sources cannot reach it.

Verified on this commit: `cargo fmt --all --check` clean; `cargo clippy -p
hekate-cli --all-targets --all-features -- -D warnings` clean; `cargo test -p
hekate-cli --test inc2_determinism` 7 passed (2 new); `cargo test -p hekate-cli
--test baseline` 4 passed; `cargo test -p hekate-cli --test narrow_determinism` 4
passed; `cargo test -p hekate-sim --test signal_compliance --test pedestrian_flow
--test pedestrian_compliance` 26 passed; `cargo test -p hekate-sim --lib rng::` 8
passed. The node's clause 3 (existing demand/profile/compliance/perception
streams and Phase 1 baseline unchanged) is carried by those unchanged suites; no
file under `scenarios/benchmarks/*_v1` or `baselines/phase1/` was touched.

## Gate evidence

Full five-gate run on the delivered commit (`1e655d5`), gate worker `gate134`:

| gate | command | exit | wall | result |
| --- | --- | --- | --- | --- |
| fmt | `cargo fmt --all --check` | 0 | 1 s | clean |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1 s | clean (`Finished dev profile`, no warnings) |
| test | `cargo test --workspace --all-features` | 0 | 315 s | 1016 passed, 0 failed, 1 ignored across 90 suites |
| deps | `./scripts/check-dependency-direction.sh` | 0 | 1 s | `dependency direction OK` |
| graph | `tangle check` | 0 | 1 s | `graph check: passed (195 nodes)` |

Done-when disposition:

- Clause 1 (isolation tests add unrelated car and narrow demand and reversed
  declaration order without changing owned maneuver draws or unaffected agent
  traces) is **met** by
  `the_inc2_maneuver_stream_stays_isolated_from_unrelated_car_and_narrow_demand`
  (`apps/hekate-cli/tests/inc2_determinism.rs`), which passed in the workspace
  run: the three runs agree on the focus source's arrival series, on the
  unaffected focus agents' canonical trace lines, and on their keyed `maneuver`
  draws, and the run is proven non-empty (the request is admitted, the added
  sources admit agents, the compared prefix records an `opposing_traversal`, and
  distinct focus agents hold distinct draws). The trace comparison is scoped to
  the focus agents admitted before the added demand's first id, the boundary the
  node's `# Result` records: agent ids are admission order, so a focus agent
  admitted later takes a different id and a different per-agent `profile` draw.
- Clause 2 (falsification probe catches draw-order coupling or an unkeyed
  maneuver choice) is **met** by
  `the_stream_isolation_check_flags_unrelated_demand_declared_before_the_focus`,
  which declares the unrelated sources before the focus source, moves the focus
  source's dense demand index, and requires the same comparisons to report the
  moved arrival series, requested agent id, trace lines, and keyed draw; the
  unkeyed-choice half is the distinct-draws assertion inside clause 1's test.
- Clause 3 (existing demand, profile, compliance, and perception streams and
  Phase 1 baseline artifacts unchanged) is **met**: `git show --stat 1e655d5`
  lists only `apps/hekate-cli/tests/inc2_determinism.rs` and this node, so no
  scenario, baseline, golden, production, or schema path moved, and the
  workspace run passes `baseline.rs`, `narrow_determinism.rs`, `seed_bank.rs`,
  `replay.rs`, `inc2_trace.rs`, the hekate-sim flow/compliance suites
  (`signal_compliance`, `vehicle_flow`, `pedestrian_flow`,
  `pedestrian_compliance`), and the `rng::` named-stream unit tests unchanged.
  `crates/hekate-sim/src/rng.rs` documents the `perception` stream as belonging
  to a later increment and never derives it, so no perception draw exists to
  change.
