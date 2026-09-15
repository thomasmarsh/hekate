---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T07:26:00Z
summary: Prove Increment 2 trace reproducibility and maneuver-stream isolation.
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Every Increment 2 fixture reproduces its decisions, claims, events, metrics, and
trace hash, and adding an unrelated agent or demand stream cannot perturb
another agent's maneuver or wrong-way draws.

# Done when

- Each passing and wrong-way fixture repeats identically at fixed seeds and
  required presets; a declared seed bank reproduces per-seed batch artifacts
  and replay verifies their event streams.
- Golden traces cover every maneuver transition and wrong-way interval
  lifecycle, including stable simultaneous-claim ordering.
- Stream-isolation tests add unrelated car, narrow demand, and reversed
  declaration order without changing owned maneuver draws or unaffected agent
  traces.
- A falsification probe demonstrates the suite catches draw-order coupling or
  an unkeyed maneuver choice.
- Existing demand, profile, compliance, and perception streams and Phase 1
  baseline artifacts remain unchanged.

# Context

Gated on [[TAS-106-check-in-increment-2-passing-fixtures]] and
[[TAS-107-check-in-contextual-wrong-way-fixtures]]. Owns focused CLI and
hekate-sim determinism suites, an Increment 2 seed bank, and Increment 2-only
goldens. Do not change production behavior except for a coupling defect proved
by the tests.

Gates my seed-bank and golden artifacts enter: scenario fixture enumeration,
CLI golden/fixture discovery, and cargo test --workspace.

# Slices

- [[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]] Fixture reproducibility and goldens.
- [[TAS-134-prove-stream-isolation-and-preserve-the-phase-1]] Stream isolation and Phase 1 baseline.

# Result

Both slices resolved: [[TAS-133-reproduce-every-increment-2-fixture-and-golden-i]]
(`f6e50f3`) and [[TAS-134-prove-stream-isolation-and-preserve-the-phase-1]]
(`1e655d5`). The TAS-134 gate run on a clean tree is fmt, clippy, `cargo test
--workspace --all-features` (1016 passed, 0 failed, 1 ignored, 90 suites),
dependency direction, and `tangle check` all green; TAS-133's gate run is in its
own `# Result`.

Done-when disposition:

- Clause 1 (fixed-seed reproduction at both required presets, a declared seed
  bank reproducing per-seed batch artifacts, replay-verified event streams) is
  **met** by TAS-133's `apps/hekate-cli/tests/{inc2_determinism,inc2_trace}.rs`
  and `scenarios/phase2/inc2/inc2_seed_bank.json`: every fixture repeats its
  canonical trace bytes and hash at Standard and Fine at its pinned bank seed,
  the bank batch over Standard reproduces every per-seed hash and compressed
  event stream, and `replay --verify` reproduces the Standard goldens of the
  three CLI-recordable autonomous fixtures. The CLI exposes neither a step flag
  nor a wrong-way request seam, so the two change-of-lane fixtures and the
  wrong-way fixture carry in-process rather than CLI-run replay coverage; TAS-133
  records that boundary rather than hiding it.
- Clause 2 (golden traces cover every maneuver transition and wrong-way interval
  lifecycle, including stable simultaneous-claim ordering): the transition and
  lifecycle half is **met** by the twelve checked-in goldens under
  `tests/golden/inc2/`, whose coverage test reads the edges back out of the
  bytes. The simultaneous-claim-ordering sub-clause is **delegated** to
  [[TAS-127-prove-deterministic-simultaneous-claim-resolutio]] (resolved), whose
  unit tests over all insertion permutations plus a fixed-seed integration test
  are the ordering evidence: no checked-in Increment 2 fixture records two
  agents maneuvering on one tick, and the negative is asserted.
- Clauses 3, 4, and 5 (unrelated car/narrow/reversed-order demand does not
  perturb owned draws or unaffected traces; a falsification probe catches
  draw-order coupling or an unkeyed maneuver choice; existing demand, profile,
  compliance, and perception streams and Phase 1 baseline artifacts unchanged)
  are **met** by TAS-134's two tests in
  `apps/hekate-cli/tests/inc2_determinism.rs` and by the workspace suite, whose
  green run leaves every scenario, baseline, golden,
  `crates/hekate-sim/src/rng.rs`, and other stream consumer untouched. See
  TAS-134's `# Result` and its gate evidence for the measured
  transcript and the one recorded scope boundary (focus agents admitted after
  the added demand take a different id, so only the agents admitted before it
  are compared).
