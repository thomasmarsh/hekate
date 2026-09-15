---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T06:51:58Z
summary: Reproduce every Increment 2 fixture and golden its transitions.
---

Parent [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]].

# Outcome

Every Increment 2 fixture reproduces its decisions, claims, events, metrics, and
trace hash at fixed seeds and required presets.

# Done when

- Each passing and wrong-way fixture repeats identically at fixed seeds and
  required presets; a declared seed bank reproduces per-seed batch artifacts and
  replay verifies their event streams.
- Golden traces cover every maneuver transition and wrong-way interval lifecycle,
  including stable simultaneous-claim ordering.

# Context

Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-108-prove-increment-2-reproducibility-and-stream-isolation]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the
reproducibility and goldens; stream isolation is the sibling slice.

# Result

Delivered, node left active: six fixtures reproduced at both required presets, a
declared seed bank, twelve trace goldens, and two suites.

- Per-preset occurrence is re-pinned by **root seed only**;
  `scenarios/phase2/inc2/inc2_seed_bank.json` declares `{11, 48, 102}`,
  reproduced with `hekate-cli seed-bank`. Arrivals are a per-tick Bernoulli
  thinning of a Poisson process, so the benchmark seed 0 a fixture is authored
  at does not admit its pair at Fine (TAS-130's own measurement held: at Fine the
  leader is admitted and the follower is not). A search over seeds 0..119 running
  each fixture's declared plan at Standard and Fine pinned, per fixture, the
  seeds that admit it at both presets, and the bank is the sorted union of the
  pinned ones — 102 for `narrow_passing_v2`, `motor_passing_narrow_v2`,
  `narrow_passing_unsafe_v2`, and `motor_lane_change_boundary_v2`, 48 for
  `motor_lane_change_v2`, 11 for `narrow_wrong_way_v2`. No schema field, fixture,
  or landed seam was touched; the search harness was temporary and is not
  checked in.
- `apps/hekate-cli/tests/inc2_determinism.rs` (5 tests): every fixture repeats its
  canonical trace bytes and hash twice at its pinned seed at Standard 0.05 s and
  Fine 0.02 s (Fine at `fidelity_ticks`' equal horizon); a non-banked seed
  diverges; the bank declares exactly the pinned seeds and each pin admits its
  fixture's maneuver at *both* presets (one overtake interval with a whole
  lifecycle for the three autonomous fixtures, two crossings around a slower
  leader for `motor_lane_change_v2`, a prevented crossing with
  `BoundaryForbidden` for `motor_lane_change_boundary_v2`, four corridor
  decisions with an open and a closed opposing interval and the occupied
  corridor's contact for `narrow_wrong_way_v2`); and a batch over the bank
  reproduces per-seed hashes and compressed event streams across two runs with
  every hash equal to `canonical_trace` and the manifest naming the declared
  bank. The three autonomous fixtures' driven bytes are also asserted equal to
  `canonical_trace`'s, so their Standard runs are CLI-reproducible.
- `apps/hekate-cli/tests/inc2_trace.rs` (4 tests) and twelve files under
  `tests/golden/inc2/` (`<id>.{standard,fine}.trace.{jsonl,sha256}`): golden bytes
  plus SHA-256 per fixture per required preset, the golden header cross-checked
  against the recorded scenario, seed, step, and tick count, and the coverage
  test reading the transitions back out of the checked-in bytes.
- `scripts/regen-goldens.sh` now regenerates the Increment 2 goldens
  (`UPDATE_GOLDENS=1 cargo test -p hekate-cli --test inc2_trace`), and the three
  explicit `hekate-cli run` commands that write the CLI-recordable Standard
  goldens byte for byte are in that suite's module docs.

Golden coverage, exactly:

- The Standard goldens of the three autonomous fixtures are written by
  `hekate-cli run` byte for byte; the suite proves it against the run
  directory manifest (seed, ticks, 0.05 s, standard fidelity, source path,
  stream hash) and `replay --verify`. Those three have CLI-run coverage.
- The Fine goldens, and both goldens of the two change-of-lane fixtures and the
  wrong-way fixture, are recorded in-process through `TraceRecorder`, which the
  trace contract documents as producing the canonical bytes and hash: the CLI
  exposes neither a `--step` flag nor either request seam. The wrong-way
  lifecycle golden is produced by the in-process harness that requests the entry
  and captures the canonical trace bytes, exactly as the TAS-131 suite drives it.
  There is **no production caller of `request_wrong_way_entry`**, so no CLI-run
  coverage is claimed for the wrong-way fixture, and none for the lane changes.
- The goldens carry the maneuver lifecycle edges (attempt, commit, returning,
  following), the pass intervals, the permitted crossings, and the wrong-way
  open/close opposing-traversal records with their legal and violating cases.

Not covered here, recorded rather than hidden:

- **Simultaneous-claim ordering.** No checked-in fixture puts two agents'
  maneuver transitions on one tick at any pinned seed, which
  `no_inc2_golden_records_two_agents_maneuvering_on_one_tick` asserts, so these
  goldens carry no tie-break and none was manufactured by authoring a scenario.
  TAS-127's unit and fixed-seed integration evidence remains the ordering
  evidence for this clause.
- **The prohibited-boundary rejection is not in the event stream.**
  `crates/hekate-sim/src/sim.rs`'s `Attempt::BoundaryForbidden` discards the
  intent and records the reason on route state with no transition, so no
  `maneuver` record is emitted for it. That fixture's golden proves the prevented
  crossing by the absence of any maneuver edge and any handoff; the reason itself
  is asserted in-process.
- **Preset and workspace gates.** Only the focused commands below were run;
  `cargo test --workspace` and `cargo clippy --workspace` are deferred to the
  node's next action. Fast (0.1 s) is not a required preset for the
  `CC-OVERTAKE`/`CC-OPPOSE` cells and is not covered.
- Inc 1 suites (`narrow_determinism.rs`, `seed_bank.rs`, `replay.rs`),
  `migration_regression.rs`'s `FROZEN` list, the Phase 1 goldens and baselines,
  and every existing file under `scenarios/phase2/inc2/` are untouched; the seed
  bank is the only addition to that directory.

Verified: `cargo fmt --all --check` clean; `cargo clippy -p hekate-cli
--all-targets --all-features -- -D warnings` clean; `cargo test -p hekate-cli
--test inc2_determinism` 5 passed, `--test inc2_trace` 4 passed, `--test
seed_bank` 9 passed, `--test replay` 8 passed, `--test migration_regression` 4
passed; `./scripts/check-dependency-direction.sh` reports `dependency direction
OK`; `tangle check` reports `graph check: passed (194 nodes)` on the delivered
commit and 195 after this session's feedback node under
[[IDX-002-tangle-feedback]].

## Gate evidence

Full five-gate run on the delivered commit (`f6e50f3`), gate worker `gate133`:

| gate | command | exit | wall | result |
| --- | --- | --- | --- | --- |
| fmt | `cargo fmt --all --check` | 0 | 1 s | clean |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 3 s | clean (`Finished dev profile`, no warnings) |
| test | `cargo test --workspace --all-features` | 0 | 316 s | 1014 passed, 0 failed, 1 ignored across 90 suites |
| deps | `./scripts/check-dependency-direction.sh` | 0 | 1 s | `dependency direction OK` |
| graph | `tangle check` | 0 | 1 s | `graph check: passed (195 nodes)` |

Done-when disposition:

- Clause 1 (fixed-seed reproduction at required presets, declared seed bank,
  per-seed batch artifacts, replay-verified event streams) is **met** by
  `apps/hekate-cli/tests/{inc2_determinism,inc2_trace}.rs`:
  `the_inc2_seed_bank_batch_reproduces_every_per_seed_hash_and_event_stream`,
  `every_inc2_fixture_reproduces_its_trace_hash_at_both_required_presets`, and
  `the_cli_run_and_replay_reproduce_the_standard_goldens_of_the_autonomous_fixtures`
  (which runs `replay --verify` and compares the reproduced stream to the
  checked-in golden bytes).
- Clause 2 (goldens cover every maneuver transition and the wrong-way interval
  lifecycle, including stable simultaneous-claim ordering): the golden coverage
  is **met** by the twelve checked-in trace goldens; the **simultaneous-claim
  ordering sub-clause is delegated** to [[TAS-127-prove-deterministic-simultaneous-claim-resolutio]]
  (resolved: unit tests across all insertion permutations plus a fixed-seed
  integration test), not to a golden. No checked-in Inc 2 fixture records two
  agents maneuvering on one tick, and
  `no_inc2_golden_records_two_agents_maneuvering_on_one_tick` asserts that
  negative.
