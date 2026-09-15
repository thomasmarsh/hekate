# Live view: an Increment 6 run

How to watch an Increment 6 run and inspect its geometry, state, decisions, and
events, and how to play back the recorded run. The increment's comparison is two
fixed-time signal-timing variants of one four-leg car/pedestrian intersection,
and this document covers both the recorded replay and the two live viewers.

Nothing here changes the kernel or the determinism contract. The live viewers
step the same kernel with the same seed and configuration as the headless
commands, and the presentation controller only chooses which completed ticks are
observed, so what you watch is the run the evidence was produced from.

## The run

- Variant scenarios:
  `scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5` and
  `scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5`.
- Shared seed bank: `experiments/increment6_signal_timing_v1/seed_bank.json`.
- Reference run: seed `1`, `1200` fixed `50 ms` steps (one 58 s signal cycle,
  spanning both green phases and both walk intervals) at the Standard preset.
  Seed 1 is one of the experiment's bank seeds and this is the increment's
  golden run.

## Play the recorded replay

The durable recorded run is checked in at

```
experiments/increment6_signal_timing_v1/replay/ew_priority_seed-1_1200ticks/
```

holding `manifest.json`, `summary.json`, `metrics.json`, `events.jsonl.gz`, and
`trajectories.parquet`. From the repository root:

```sh
cargo run -p hekate-cli -- replay \
  experiments/increment6_signal_timing_v1/replay/ew_priority_seed-1_1200ticks --verify
```

`replay` re-loads the recorded scenario source, checks its recorded content hash,
re-runs the kernel with the recorded seed, step, and tick count, and writes the
reproduced canonical event stream to stdout: a `run` header line, one JSON-Lines
record per event, and a trailing `summary` line. It writes nothing to disk and
never mutates the recorded directory. With `--verify` it also compares the
reproduction byte for byte against `events.jsonl.gz` and compares the trace hash
against the manifest's recorded hash, printing `replay trace hash: <hash>` and
`replay verified: <dir>` to stderr and exiting `0`; a mismatch exits non-zero
and names the first differing byte or the hash disagreement. Drop `--verify` to
stream the run without checking it.

The recorded stream hash for this capture is
`a9666e0a5bfca56df071954b07a7e6fdd64d9009c27f6dd41d47c887b78f6def`, the same
value `tests/golden/four_leg_pedestrian_ew_priority_v1.trace.sha256` pins. The
`ns_priority` variant's golden run hashes to
`35221e91d3631bed9caedd0fe60eafdef7387d7a1b4b49469862e3013ea6cf79`
(`tests/golden/four_leg_pedestrian_ns_priority_v1.trace.sha256`); it has no
checked-in capture, so record it with the command below to replay it.

Regenerate the checked-in capture in one command into a fresh run directory; its
stream hash must equal
`a9666e0a5bfca56df071954b07a7e6fdd64d9009c27f6dd41d47c887b78f6def`:

```sh
cargo run -p hekate-cli -- run \
  scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5 \
  --seed 1 --ticks 1200 \
  --run-dir /tmp/increment6_ew_priority_seed-1_1200ticks \
  --output -
```

Swapping the scenario for `..._ns_priority_v1.json5` and pointing `--run-dir` at
a fresh directory records the other variant. A completed run directory is never
rewritten, so regenerate into a new or emptied path.

## Watch the run live

The Bevy desktop viewer:

```sh
cargo run -p hekate-viewer -- \
  scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5 1
```

The character-cell terminal viewer (Kitty graphics when the terminal supports it
and `auto` probes true):

```sh
cargo run -p hekate-tui -- --backend ascii \
  scenarios/experiments/four_leg_pedestrian_ew_priority_v1.json5 1
```

Both take `[scenario] [seed]` positionally, so `1` selects the recorded run's
seed. Add `--release` to either `cargo run` for a smoother picture. The terminal
viewer has `--help`; the Bevy viewer does not read any flags and treats its first
argument as the scenario path. Run the second variant for the comparison by
passing `scenarios/experiments/four_leg_pedestrian_ns_priority_v1.json5 1`.

### Controls

The keys are the same in both viewers; the Bevy viewer also accepts a mouse.

| Key            | Action                                             |
| -------------- | -------------------------------------------------- |
| `space`        | pause / resume                                     |
| `.`            | advance one fixed step                             |
| `1` / `2` / `3`| speed 1x / 4x / maximum                            |
| `r` (`R`)      | restart with the same seed                         |
| `n` (`N`)      | restart with the next seed                         |
| `g` (`G`)      | toggle the authored geometry overlay               |
| `v` (`V`)      | toggle per-agent velocity vectors                  |
| `b` (`B`)      | toggle the safety/event overlay                    |
| `w a s d`      | pan the viewport (terminal also accepts the arrows)|
| `+` / `-`      | zoom in / out (terminal)                           |
| `tab`          | select the next agent (terminal)                   |
| click / `esc`  | inspect a body / clear the selection               |
| `q`            | quit (terminal)                                    |

### What to inspect

- **Geometry.** The authored paths, crossings, conflict regions, and their
  geometry are drawn by default (`g` hides and re-shows them). Pan and zoom to
  see how the two approaches and the two crosswalks sit relative to each other.
- **State.** The status strip carries the scenario id, simulated seconds, tick,
  speed, and running/paused state, plus the seed, live agent count,
  spawned/despawned totals, and the selected agent. Select an agent to see its
  full state in the inspector: position, heading, speed, path, distance along
  it, body length x width, route (movement), profile, and intent.
- **Decisions.** The selected agent's inspector ends with a `decision` line — the
  agent's current decision — so you can watch it change as the agent approaches
  the signal or crosswalk. The terminal shows the same fields on one line.
- **Events.** The safety overlay is on by default (`b` toggles it) and draws from
  the frame's projected safety data: a marker ring per recent event (larger rings
  for heavier records — collision, near miss, violation, entry/exit, queue,
  yielded, control transition, in their severity colors), an emphasis ring on
  each body in a strong event, and white link rings on the other participants of
  the selected agent's recent events. The inspector's `links` line (the terminal
  shows the latest `link`) names the events the selected agent took part in;
  follow an agent id to inspect the other participant.

## Related

- `docs/known_limitations.md` — what the comparison does and does not claim.
- `scripts/reproduce-increment6.sh` — reproduces the comparison, the convergence
  evidence, the goldens, and `replay --verify` from a clean checkout.
- `PHASE_1_PLAN.md` — the Increment 6 outcome and its gate.
