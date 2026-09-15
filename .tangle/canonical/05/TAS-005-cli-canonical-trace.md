---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Add the headless CLI that emits a canonical trace and hashes it.
---

# Context

Depends on [[TAS-003-scenario-source-parse]] at context_rev 1.
Depends on [[TAS-004-sim-fixed-step-kernel]] at context_rev 1.

# Outcome

`hekate-cli run` produces a deterministic canonical trace from the
walking-skeleton scenario, suitable for golden tests and hash comparison.

# Done when

- Canonical event serialization uses stable field and event ordering.
- Running the same seed twice yields the same trace hash.
- A checked-in golden trace hash fails loudly on unintended serialization or
  behavior change.

# Result

Implemented in `apps/hekate-cli`:

- `src/lib.rs` — `load_scenario` reads, parses, validates, and compiles a JSON5
  file (`LoadError` reports read, parse, and every validation diagnostic), and
  re-exports the trace API.
- `src/trace.rs` — `canonical_trace(scenario, config, ticks) -> Trace` runs the
  kernel and returns the exact bytes plus a lowercase-hex SHA-256.
- `src/main.rs` — `run` subcommand with `--seed`, `--ticks` (default 250),
  `--output` (`-` for stdout), and `--hash-file`; the hash is printed to stderr.

Trace format: UTF-8 JSON Lines, one compact record per line with a fixed field
order, terminating newline. A `run` header carries `scenario_id`,
`schema_version`, `seed`, `step_s`, and `ticks`; each event line carries
`kind`, `tick`, `event`, `agent`, `path` plus the variant's own fields
(`distance_m` for `spawned`, `reason` for `despawned`, spelled `exited_path`);
a `summary` footer carries `ticks`, `spawned`, `despawned`, `remaining`. Events
are grouped by ascending tick and, within a tick, ascending agent index; a step
attributes its events to the tick it completes. The hash is SHA-256 over these
exact bytes. Records are closed structs, never maps, so field order is part of
the contract.

Golden fixtures are checked in at
`tests/golden/walking_guide_v1.trace.jsonl` and
`tests/golden/walking_guide_v1.trace.sha256` for `seed 0`, `250` ticks.
`apps/hekate-cli/tests/golden_trace.rs` regenerates the trace from the
checked-in scenario, compares the bytes to the golden trace, checks the hash,
and runs twice to confirm the same seed hashes identically.

Evidence: `cargo test --workspace --all-features` passes, including 4
`hekate-cli` unit tests and 2 golden-trace integration tests;
`cargo clippy --workspace --all-targets --all-features -- -D warnings` is
warning-free; `cargo fmt --all --check` passes; `cargo build --workspace
--release` succeeds; `scripts/check-dependency-direction.sh` reports
`dependency direction OK`. Added the pinned `sha2 = "=0.10.9"` workspace
dependency; no Bevy type enters `hekate-model`, `hekate-sim`, or `hekate-cli`.

Parent [[TAS-001-phase-1-increment-0]].
