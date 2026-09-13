---
context_rev: 1
priority: P1
updated: 2026-09-13T00:54:01Z
summary: Slice A2b of Phase 1 Increment 5 adds a `replay` CLI command that reproduces a recorded run's canonical event stream from its immutable manifest, verifies it against the recorded artifact on request, and fails non-zero on a reproducibility mismatch.
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

Slice A2b landed the read-only `replay` command on top of the B1 run-directory
manifest. No scenario-schema change, no record-shape change, `EVENT_VERSION`
stays 2, no new dependency, and no golden, baseline, or scenario file changed.

## What landed

- `apps/tangle-cli/src/replay.rs` (new) owns reproduction.
  `replay_run_directory` (`:144`) reads the run directory's `manifest.json`
  completion marker, re-loads the recorded scenario source, checks its content
  hash against the manifest's, and re-runs `canonical_run` with the recorded
  seed, `step_s`, and `ticks`, returning the reproduced canonical trace. With
  `verify`, `verify_reproduction` (`:207`) decompresses the recorded
  `events.jsonl.gz` and compares it byte for byte with the reproduction, then
  compares the reproduced trace hash with the manifest's recorded stream hash;
  `describe_difference` (`:261`) reports the first differing offset and both
  lengths.
- Failure is a diagnostic, not a panic: a missing directory (`:37`), an
  interrupted directory without the completion marker (`:45`), a scenario source
  whose content hash changed (`:80`), a manifest that names another stream
  layout (`:99`), and a reproduction mismatch (`:125`, naming the artifact and
  what differs) each carry the path and the values that disagree.
- `apps/tangle-cli/src/main.rs` adds `replay <RUN_DIR> [--verify]` (`:82`,
  `:366`, `:381`). The reproduced stream is written to stdout only after
  reproduction and verification succeed, so a refused replay emits no stream;
  the reproduced trace hash, and `replay verified: <dir>` under `--verify`, go
  to stderr. `REPLAY_LONG_ABOUT` (`:123`) documents usage, output, and exits.
- `apps/tangle-cli/src/lib.rs` exports `replay_run_directory` and `ReplayError`
  (`:33`); `apps/tangle-cli/tests/replay.rs` (new) is the contract suite.

Replay opens only the run directory and the recorded scenario source; it writes
no artifact, creates no directory, and never mutates a completed run.

## Evidence

`cargo test -p tangle-cli --test replay` (7 passed) and
`cargo test -p tangle-cli --lib` (29 passed, including the 3 new unit tests in
`replay.rs`):

- `replay_reproduces_the_recorded_stream_and_writes_no_artifact`
  (`tests/replay.rs:180`) — `replay` on a fresh run exits 0, stdout equals the
  checked-in trace golden and the decompressed recorded stream, stderr names the
  trace hash golden, and both the scratch directory's entries and the run
  directory's per-file content hashes are unchanged.
- `verify_accepts_a_fresh_reproduction_and_names_the_recorded_hash`
  (`tests/replay.rs:221`) — `replay --verify` exits 0 and names the manifest's
  recorded stream hash, which equals the checked-in hash golden.
- `verify_rejects_a_tampered_recorded_stream` (`tests/replay.rs:251`) — a
  re-gzipped stream with one altered record fails (`exit 1`), names the artifact
  and the first differing offset, emits no stdout, and leaves the manifest
  byte-identical.
- `verify_rejects_a_manifest_whose_recorded_hash_disagrees`
  (`tests/replay.rs:293`) — a manifest whose recorded hash is zeroed while its
  recorded stream still holds the run fails on the hash comparison and mutates
  no byte of the stream.
- `verify_rejects_a_manifest_that_names_another_stream`
  (`tests/replay.rs:337`) — a manifest naming a different stream file is refused
  rather than verified against another artifact.
- `replay_rejects_a_scenario_source_that_changed_since_the_run`
  (`tests/replay.rs:366`) — a comment appended to the recorded source after the
  run fails the content-hash check (`exit 1`), names the recorded hash, and
  mutates no run artifact.
- `replay_rejects_a_missing_or_incomplete_run_directory`
  (`tests/replay.rs:407`) — an absent directory reports `does not exist`, and a
  run directory whose `manifest.json` was removed reports `incomplete`.
- Unit: `an_absent_run_directory_is_reported_as_missing` (`replay.rs:284`),
  `a_directory_without_a_manifest_is_reported_as_incomplete` (`replay.rs:300`),
  `a_stream_difference_names_its_first_offset` (`replay.rs:319`).

## Claim and write set

Base hash `85f095cd2dee598b1d12dd73d69d57b2001e76bf8e808dfdde6a6ea6f981548d`
from `braintree hash TAS-036`, claimed by `worker` for 14400 s. Write set:
`apps/tangle-cli/src/**`, `apps/tangle-cli/tests/**`, and this node's own
frontier transition (`nodes/proposed/` to `nodes/active/`, then to
`nodes/resolved/`). Every changed, created, and moved path stayed inside it.

## Dependencies reported

None. `flate2`, already a `tangle-cli` dependency from B1, decodes the recorded
stream; `apps/tangle-cli/Cargo.toml` and `Cargo.lock` are unchanged.

## Preservation

No file under `tests/golden/`, `baselines/`, `scenarios/`, `schemas/`, or the
kernel crates changed; `EVENT_VERSION` stays 2
(`crates/tangle-sim/src/event.rs:66`). Replay composes the existing
`canonical_run` and reads the run-directory layout unchanged, so it cannot alter
canonical bytes, and the golden, hash-golden, baseline, and run-directory suites
still pass.

## Interpretation and deferrals

- **Where replay locates the source.** It re-loads
  `manifest.scenario.source_path` exactly as the run recorded it, resolved
  against the replay process's working directory; it does not search or rewrite
  the path. A run recorded from a relative path replays from the directory that
  path was relative to.
- **What `--verify` compares.** The recorded `events.jsonl.gz` decompressed,
  byte for byte, against the re-run's canonical bytes, plus the reproduced trace
  hash against `manifest.stream.uncompressed_sha256`. It compares stream
  content, not the compressed container, so a re-encoded gzip of the same
  records still verifies while any record change fails. It does not re-check the
  applied sampling policy or the trajectory artifact: reproduction needs only
  the recorded seed, step, and tick count, and trajectories never enter the
  canonical stream.

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`807a59f`), verified by rerunning the five gates on that tree:

1. **`replay <RUN_DIR>` reproduces the run's canonical event stream from the
   manifest and exits zero on a healthy run directory.** `replay.rs:144`
   (`replay_run_directory`) reads the manifest, `:158` (`read_manifest`) requires
   the `manifest.json` completion marker, `:181` (`reproduce`) re-loads the
   recorded source and re-runs `canonical_run` with `manifest.seed`,
   `manifest.step_s`, and `manifest.ticks`, and `main.rs:381` (`replay`) writes
   the returned canonical bytes to stdout. `tests/replay.rs:180` asserts exit 0
   and stdout equal to the checked-in trace golden and to the decompressed
   recorded stream; `tests/replay.rs:221` asserts the same under `--verify`.
2. **`--verify` compares the reproduction against the recorded stream and trace
   hash; a mismatch exits non-zero with an actionable message.** `replay.rs:207`
   (`verify_reproduction`) compares the decompressed recorded stream byte for
   byte, then the reproduced hash with `manifest.stream.uncompressed_sha256`;
   `replay.rs:261` (`describe_difference`) locates the first differing offset;
   `replay.rs:125` (`Mismatch`) names the artifact and the difference, and
   `main.rs:381` returns `ExitCode::FAILURE` before writing any stdout.
   `tests/replay.rs:251` (tampered stream), `:293` (recorded hash disagrees), and
   `:337` (another stream descriptor) each assert exit 1, an empty stdout, and a
   naming diagnostic.
3. **Tests cover a fresh run replayed to the same trace hash and stream; a
   tampered recorded stream; a scenario content-hash mismatch; and a
   non-existent or incomplete run directory.** `tests/replay.rs:180`,
   `:221`, `:251`, `:366`, and `:407` cover exactly those four cases, with the
   content-hash case at `:366` asserting both hashes in the message and the
   directory case at `:407` covering the missing directory and the removed
   completion marker.
4. **No artifact is written and no run directory is mutated.** Replay takes
   `&Path` and opens files only; `tests/replay.rs:180` compares the scratch
   directory's entries and the run directory's per-file content hashes before
   and after a successful replay, and `:251`, `:293`, and `:366` compare hashes
   before and after a refused replay. A post-implementation smoke run in the
   repository root confirmed `replay <dir>` stdout is byte-identical to the
   `run --output` trace and that `--verify` exits 0 on a fresh run.
5. **The canonical trace format, trace golden, trace hash golden, Phase 1
   baseline, and all goldens are unchanged; `EVENT_VERSION` stays 2.** The
   implementation commit touches only `apps/tangle-cli/src/{lib.rs,main.rs,replay.rs}`
   and `apps/tangle-cli/tests/replay.rs` plus the node move; no file under
   `tests/golden/`, `baselines/`, `scenarios/`, `schemas/`, or the kernel crates
   changed, and `EVENT_VERSION` stays 2
   (`crates/tangle-sim/src/event.rs:66`). The golden, hash-golden, baseline, and
   run-directory guards still pass, and `tests/replay.rs:180` compares the
   reproduction with the checked-in JSONL golden and `:221` with the hash
   golden.
6. **The five gates pass on the final tree** (rerun on `807a59f`, 2026-09-13:
   `cargo test --workspace --all-features` passed with 489 tests and 0 failures;
   `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
   `cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh`
   printed `dependency direction OK`; `braintree check nodes` printed
   `graph check: passed (62 nodes)`).

Outcome complete: the replay command, its checked reproduction against the
recorded stream and trace hash, its refusal diagnostics, and the gate evidence
are on the committed tree. Aggregation and the convergence runner stay with
siblings under [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].
