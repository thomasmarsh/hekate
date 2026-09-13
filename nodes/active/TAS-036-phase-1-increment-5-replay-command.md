---
context_rev: 1
priority: P1
updated: 2026-09-13T00:47:33Z
summary: Slice A2b of Phase 1 Increment 5 adds a `replay` CLI command that reproduces a recorded run's canonical event stream from its immutable manifest, verifies it against the recorded artifact on request, and fails non-zero on a reproducibility mismatch.
next: Read the B1 run-directory manifest API and implement `replay <RUN_DIR> [--verify]`, then test reproduction, recorded-stream mismatch, and a stale scenario-hash failure.
---

# Outcome

Add a `replay` command to the Tangle CLI alongside `validate`, `run`, and
`batch`. `replay <RUN_DIR>` reads a completed run directory's `manifest.json`,
re-loads the recorded scenario source, checks its content hash against the
manifest, and re-runs the simulation with the recorded seed, tick count, and
step. It emits the reproduced canonical event stream (compressed JSON Lines
decompressed to canonical record order) to stdout.

With `--verify`, replay also checks the reproduction against the recorded
artifacts: the reproduced canonical trace hash must equal the manifest's stream
hash, and the reproduced record stream must equal the recorded `events.jsonl.gz`
byte-for-byte. Any mismatch — a tampered recorded stream, a changed scenario
source, a wrong seed or step — exits non-zero with a diagnostic naming the
mismatch. A successful replay exits zero.

The command reads only the immutable run directory and the scenario source; it
writes no artifact and mutates nothing.

This is the deterministic-reproduction surface the plan needs for a recorded
replay: it proves the same manifest reproduces the same canonical event stream.

# Done when

- `replay <RUN_DIR>` reproduces the run's canonical event stream from the
  manifest and exits zero on a healthy run directory.
- `--verify` compares the reproduction against the recorded stream and trace
  hash; a mismatch exits non-zero with an actionable message.
- Tests cover: a fresh run replayed to the same trace hash and stream; a
  tampered recorded stream failing verification; a manifest whose recorded
  scenario content hash no longer matches the source failing; a non-existent or
  incomplete run directory failing.
- No artifact is written and no run directory is mutated.
- The canonical trace format, trace golden, trace hash golden, Phase 1 baseline,
  and all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice B1 fixed the immutable manifest provenance (scenario source path and
content hash, seed, ticks, step, stream hash); slice A2a added `batch`. This
sibling adds the fourth CLI command the plan names and is the reproduction check
the Increment 6 determinism gate builds on. It re-runs the kernel rather than
re-emitting the recorded events alone, so a replay exercises the current binary
against the recorded manifest.

No new dependency is expected; `flate2` (B1) already decodes the event stream.

# Result

In progress. Base hash `85f095cd2dee598b1d12dd73d69d57b2001e76bf8e808dfdde6a6ea6f981548d`
from `braintree hash TAS-036`, claimed by `worker` for 14400 s. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`, and this node's own frontier
transition (`nodes/proposed/` to `nodes/active/`, then to `nodes/resolved/`).
