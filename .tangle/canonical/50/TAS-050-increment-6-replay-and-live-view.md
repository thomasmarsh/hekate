---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice E of Phase 1 Increment 6 produces a recorded replay of the comparison and live-view instructions so a reviewer can inspect geometry, state, decisions, and events visually, reusing the existing replay and viewer/TUI surfaces.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Reuses the existing `replay` command and the viewer/TUI from the terminal
rendering epic ([[TAS-006-bevy-viewer-skeleton]] and Increments 4–5 viewer work).
This is a demonstration and documentation slice: it adds no new simulation
behavior and must not change the kernel or the determinism contract.

# Outcome

- A recorded replay artifact produced from an Increment 6 run.
- Live-view instructions (documented command and options) that let a developer
  watch the same run and inspect geometry, state, decisions, and events.

# Done when

- A recorded replay artifact is checked in or is reproducible by one documented
  command from a checked-in run.
- Live-view instructions exist and are accurate against the current CLI/viewer.
- The recorded replay and live view do not change state-affecting behavior; any
  headless-versus-viewed trace comparison stays equal where the contract
  requires it.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `tangle check`.

# Result

Claimed by `worker` at base hash
`816d7e76d68fdc6ba1c93aad050bed3886e4c29d1fd26557e8c2fbff6cd7a6b8`
(`tangle hash TAS-050`, bare-ID form, FBK-002) with `--lease-seconds 14400`,
and moved to `.tangle/active/` with `git mv` plus `git add` of the destination
(FBK-013). Write set held: `docs/increment6_live_view.md`,
`experiments/increment6_signal_timing_v1/replay/**`, and this node. No file under
`crates/`, `apps/`, `schemas/`, `tests/golden/`, or `baselines/` changed, no
dependency was added, and `EVENT_VERSION` stays 2. `Cargo.toml` and `Cargo.lock`
are unchanged.

## What landed

Two commits, each green on its own:

- `becdf34` **the claim.** `git mv` of this node from `proposed/` to `active/`,
  the destination staged (FBK-013).
- `dedc4ff` **the recorded replay and the live-view instructions.** The checked-in
  capture and `docs/increment6_live_view.md`, described below.

### The recorded replay artifact

The project has no cast/terminal-recording format and no capture-recording
infrastructure, and none was added. The smallest durable replay artifact the
existing tooling supports is a run directory, which `replay` reproduces from its
manifest and streams to stdout. So the recorded replay is one small checked-in
run directory:

`experiments/increment6_signal_timing_v1/replay/ew_priority_seed-1_1200ticks/`
(~76 KB: `manifest.json`, `summary.json`, `metrics.json`, `events.jsonl.gz`,
`trajectories.parquet`). It records the increment's golden run — the
`ew_priority` variant scenario, seed 1, 1200 fixed 50 ms steps at the Standard
preset, one 58 s signal cycle spanning both green phases and both walk intervals
— which is the run `tests/golden/four_leg_pedestrian_ew_priority_v1.trace.*`
already pins, so the recorded replay and the checked-in golden cannot drift
silently. Play it from the repository root:

```sh
cargo run -p hekate-cli -- replay \
  experiments/increment6_signal_timing_v1/replay/ew_priority_seed-1_1200ticks --verify
```

It exits 0, prints `replay trace hash:
a9666e0a5bfca56df071954b07a7e6fdd64d9009c27f6dd41d47c887b78f6def` and `replay
verified: <dir>` to stderr, and its stdout is byte-identical to
`tests/golden/four_leg_pedestrian_ew_priority_v1.trace.jsonl`. The run is
reproducible by one documented `run --run-dir` command, which writes the same
bytes; the `ns_priority` variant's golden run has no checked-in capture and the
document gives the one command to record it.

### The live-view instructions

`docs/increment6_live_view.md` gives the exact commands and options to watch the
same run live and inspect geometry, state, decisions, and events:

- the recorded replay command above, its stdout format, and the regeneration
  command;
- the Bevy viewer, `cargo run -p hekate-viewer --
  scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5 1`, and the
  terminal viewer, `cargo run -p hekate-tui -- --backend ascii
  scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5 1`, both taking
  `[scenario] [seed]`;
- the shared control legend (pause/step/speed, restart, pan/zoom, the `g`/`v`/`b`
  overlays, selection) and what each surface shows for geometry, state, the
  selected agent's `decision`, and the safety-overlay event markers plus the
  inspector's event `links`.

## Determinism

Nothing in `hekate-model`, `hekate-sim`, `hekate-present`, or the CLI changed, so
no state-affecting behavior moved. The live viewers step the same kernel with the
same seed and configuration as the headless commands — the presentation
controller only chooses which completed ticks are observed — so the viewed run is
the headless run, and `replay --verify` on the checked-in capture reproduces the
golden event stream and trace hash.

## Gates

All five gates were run on the final tree:

- `cargo test --workspace --all-features` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.
- `cargo fmt --all --check` — passed.
- `./scripts/check-dependency-direction.sh` — `dependency direction OK`.
- `tangle check` — `graph check: passed (90 nodes)`.

## Node status

Resolved. TAS-045's `next` was advanced to the slice F/G action (commit
`5a91999`), so this node moved to `.tangle/resolved/` and dropped `next`.
The frontier is now slice F/G closeout under TAS-045.
