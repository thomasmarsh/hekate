---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Slice C2a of Phase 1 Increment 5 adds a versioned common-random-number seed-bank artifact and a `seed-bank` CLI command, and lets `batch` run a batch from a seed bank so two variants share the same aligned seeds.
---

# Outcome

Add a durable common-random-number (CRN) seed bank and the CLI surface to use
it. A seed bank is a versioned JSON artifact (`seed_bank_version: 1`) holding an
ordered list of seeds that two variants of the same scenario both run, so the
per-seed runs are paired and a paired A/B comparison cancels the between-seed
variance the two variants share.

Requirements:

- A `seed-bank` CLI command writes a deterministic seed bank (from an explicit
  seed list or a start/count/stride request) to a named path or stdout. The
  output is byte-reproducible: no clock, no randomness, ascending or explicitly
  requested order, documented.
- `batch` accepts the seed bank (`--seed-bank <FILE>`) as an alternative to
  `--seeds`, so two batches over the same bank share exactly the same seeds in
  exactly the same order. Record the seed bank's identity (path and content
  hash) in the batch manifest so a comparison can prove both sides used one
  bank.
- Inspect the kernel's seed derivation and document how a seed maps to the
  run's named RNG streams (whether a bank needs per-stream sub-seeds, or the
  single run seed already determines every stream). If the kernel already
  derives every stream from one seed, the bank is an ordered seed list and the
  common-random-number property follows from running both variants at the same
  seed; state the assumption and its limit (a variant that changes the number or
  order of draws breaks alignment).
- Existing `batch --seeds` behavior is preserved; the canonical trace, goldens,
  and Phase 1 baseline stay byte-identical; no schema change; `EVENT_VERSION`
  stays 2; no dependency change in `hekate-model` or `hekate-sim`.

If the slice outgrows one session, land the seed-bank type and command first
(with tests), commit with `Refs TAS-039`, leave the node `active` with a concrete
`next` naming the `batch` integration, and report.

# Done when

- A `seed-bank` command writes a versioned, deterministic seed bank, covered by
  a byte-reproducibility test.
- `batch` can run from a seed bank, records the bank's path and content hash in
  its manifest, and preserves existing `--seeds` behavior, covered by a test.
- Two batches from one bank share the same seeds in the same order, covered by a
  test.
- The documentation of the kernel seed-to-stream derivation and the CRN
  assumption is in the node result.
- The canonical trace, trace golden, trace hash golden, Phase 1 baseline, and
  all goldens are unchanged; `EVENT_VERSION` stays 2.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `tangle check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Increment 5 asks for "common-random-number seed banks for A/B comparisons". This
child adds the bank and its `batch` integration; the paired comparison that
consumes it is the next direct child (C2b). The batch manifest already records
each run's seed (TAS-035); this child records the shared bank identity so a
comparison can prove both sides used one bank.

# Result

Claimed by `worker` at base hash
`a639e3139cacb546671cd70b5f41697964a10bca59b4ef85337bcae0828c85ba`
(`tangle hash TAS-039`, bare-ID form) with `--lease-seconds 14400`. Write set:
`apps/hekate-cli/src/**`, `apps/hekate-cli/tests/**`, `apps/hekate-cli/Cargo.toml`
and `Cargo.lock` (both unused: no dependency was added), and this node's own
frontier transition (`nodes/proposed/` → `nodes/active/`). Every changed,
created, and moved path stayed inside that set. No kernel file was edited, and
no file under `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/`
changed.

## What landed

- `apps/hekate-cli/src/seed_bank.rs` (new) owns the artifact. `SeedBank` holds
  `seed_bank_version` (always 1) and the ordered `seeds`; `SeedBank::from_seeds`
  builds the ascending unique list from an explicit request, and
  `SeedBank::from_range(start, count, stride)` builds the arithmetic one.
  `read_seed_bank` parses a file, refuses another version, an empty list, a
  duplicate seed, or a list that is not ascending, and returns `LoadedSeedBank`
  with the SHA-256 of the file's exact bytes. `SeedBankReference` is the bank
  identity a manifest records. Ordering is documented in the module: the bank is
  strictly ascending and unique, so equal seed sets produce byte-identical
  banks and the content hash identifies the pairing; the writer reads no clock
  and no randomness.
- `apps/hekate-cli/src/main.rs` adds the `seed-bank` command (`--seeds` xor
  `--start`/`--count`/`--stride`, `--output` defaulting to `-`), and
  `batch (--seeds | --seed-bank <FILE>)` now takes exactly one seed source. A
  bank is read and validated before any run starts and is recorded at the path
  the command named rather than a canonicalized one.
- `apps/hekate-cli/src/batch.rs` adds `BatchRequest.seed_bank` and
  `BatchManifest.seed_bank: Option<SeedBankReference>`. The field is
  `#[serde(default, skip_serializing_if = "Option::is_none")]`, so a `--seeds`
  batch serializes exactly as it did before the field existed and an old
  `batch.json` still reads.
- `apps/hekate-cli/tests/seed_bank.rs` (new) is the contract suite;
  `tests/batch.rs` and `tests/aggregate.rs` gained the additive `seed_bank:
  None` field in the request and manifest literals they build.
- `apps/hekate-cli/Cargo.toml` and `Cargo.lock` are unchanged.

## Kernel seed-to-stream derivation (read only)

`crates/hekate-sim/src/rng.rs` derives every named stream from the root seed:
`derive_stream(root_seed, name, id)` mixes FNV-1a of the stream name, the root
seed, and the stable identifier through SplitMix64 and seeds one ChaCha20
stream with it. `RunConfig` carries exactly one root seed (`config.rs:16`). The
derived streams are `demand` per vehicle demand-source index
(`sim.rs:279`), `pedestrian_demand` per pedestrian demand-source index
(`sim.rs:290`), and the mode-neutral `profile` and `compliance` per agent id
(`sim.rs:1526`, `sim.rs:1527`, `sim.rs:1596`, `sim.rs:1597`).

So a bank needs no per-stream sub-seeds: **the single run seed already
determines every named stream**, and the kernel's own stream-isolation tests
pin that adding a draw on one stream cannot reshuffle another. Common random
numbers therefore follow from running two variants at the same seed: for every
stream and identifier, both variants draw the same value at the same draw
position. The limit is that the alignment is positional and
identifier-keyed: a variant that changes the number or order of draws on a
stream, or that renames a stream, adds a demand source, or renumbers agents,
desynchronizes the correspondence at the same seed. A seed bank equalizes the
seeds, not the number or order of draws, so a paired comparison is valid only
while every paired run makes the same draws in the same order.

## Evidence

`apps/hekate-cli/tests/seed_bank.rs` (9 tests):

- `seed_bank_output_is_byte_reproducible_across_invocations` compares the bytes
  of repeated invocations for an explicit and an arithmetic request.
- `a_seed_bank_round_trips_through_a_file` reads the written file back through
  `read_seed_bank`, checks the content hash against the file bytes, and checks
  that writing to a path and to stdout are the same bytes.
- `a_malformed_duplicated_or_unsorted_bank_is_refused` covers descending,
  duplicated, empty, future-versioned, non-JSON, and missing banks.
- `batch_runs_a_seed_bank_and_records_its_identity` runs a two-seed bank, checks
  the manifest's seeds, run directories, run manifests, and bank reference, and
  checks the same runs as an equivalent `--seeds 5,8` batch.
- `two_batches_from_one_bank_share_the_same_seeds_in_the_same_order` runs two
  batches over one bank and compares the bank identity, the seed order, and
  every per-position trace hash.
- `batch_still_runs_an_explicit_seed_list` pins the preserved `--seeds`
  behavior, including the absence of a `seed_bank` field in `batch.json`.
- `a_batch_needs_exactly_one_seed_source` and
  `an_unreadable_or_invalid_bank_is_refused_and_runs_nothing` cover the usage
  errors and the refusal to start a batch from a bad bank.
- `an_unusable_seed_bank_request_is_refused` covers zero count, zero stride,
  both seed sources, `--start` without `--count`, no request, and a repeated
  seed.

The module's own 5 unit tests cover ascending output, refused duplicates and
empty lists, arithmetic stepping, zero/overflow refusal, and that a descending
list is refused rather than reordered.

## Preservation

No scenario schema, event record, run manifest, or run directory artifact
changed; `EVENT_VERSION` stays 2. An explicit-seed batch writes exactly the
bytes it wrote before this slice, and the golden, hash-golden, and Phase 1
baseline tests are untouched and still pass.

# Resolution

Every `# Done when` criterion holds on the committed tree (implementation in
`33e301a`), verified by rerunning the five gates on that tree:

1. **A `seed-bank` command writes a versioned, deterministic seed bank, covered
   by a byte-reproducibility test.** `main.rs:518` (`seed_bank`) builds the bank
   from `--seeds` (`SeedBank::from_seeds`, `seed_bank.rs:153`) or
   `--start`/`--count`/`--stride` (`SeedBank::from_range`, `seed_bank.rs:167`)
   and writes it through the crate's JSON writer (`main.rs:711`), which reads no
   clock and no randomness; `main.rs:491` (`SeedBankArgs`) pins the request
   shape and `seed_bank.rs:52` (`SEED_BANK_VERSION = 1`) the artifact version.
   `tests/seed_bank.rs:137`
   (`seed_bank_output_is_byte_reproducible_across_invocations`) compares the
   bytes of repeated invocations, and `tests/seed_bank.rs:181`
   (`a_seed_bank_round_trips_through_a_file`) round-trips the file through
   `read_seed_bank` and checks the recorded content hash against the file bytes.
2. **`batch` can run from a seed bank, records the bank's path and content hash
   in its manifest, and preserves existing `--seeds` behavior, covered by a
   test.** `main.rs:444` reads and validates the bank before any run starts and
   builds the `SeedBankReference` from the path as given and the bank's content
   hash; `batch.rs:205` is the manifest field and `batch.rs:258` is where
   `run_batch` records it. `tests/seed_bank.rs:292`
   (`batch_runs_a_seed_bank_and_records_its_identity`) asserts the manifest's
   `seed_bank`, `seeds`, run directories, and run manifests, and that the same
   seeds given explicitly produce the same per-run trace hashes;
   `tests/seed_bank.rs:409` (`batch_still_runs_an_explicit_seed_list`) pins that
   `--seeds` still writes an ascending manifest with no `seed_bank` field at all.
3. **Two batches from one bank share the same seeds in the same order, covered
   by a test.** `batch.rs:274` (`normalize_seeds`) keeps the manifest ascending
   and `batch.rs:205` records the bank, so two batches over one bank name the
   same bank hash and the same seed list. `tests/seed_bank.rs:361`
   (`two_batches_from_one_bank_share_the_same_seeds_in_the_same_order`) runs two
   batches over one bank and compares the bank identity, the seed order, and
   every per-position trace hash.
4. **The documentation of the kernel seed-to-stream derivation and the CRN
   assumption is in the node result.** The `# Result` section "Kernel
   seed-to-stream derivation (read only)" names
   `derive_stream(root_seed, name, id)` (`crates/hekate-sim/src/rng.rs`), the
   four derived streams (`crates/hekate-sim/src/sim.rs:279`, `:290`, `:1526`,
   `:1527`, `:1596`, `:1597`), the conclusion that one root seed already
   determines every stream so no per-stream sub-seed is needed, and the limit
   that the alignment is positional and identifier-keyed, so a variant that
   changes the number, order, or naming of draws breaks the pairing at the same
   seed.
5. **The canonical trace, trace golden, trace hash golden, Phase 1 baseline,
   and all goldens are unchanged; `EVENT_VERSION` stays 2.** `33e301a` touches
   only `apps/hekate-cli/src/{batch.rs,lib.rs,main.rs,seed_bank.rs}`,
   `apps/hekate-cli/tests/{aggregate.rs,batch.rs,seed_bank.rs}`, and this node;
   no file under `tests/golden/`, `baselines/`, `scenarios/`, or `schemas/`, and
   no kernel crate, changed. `EVENT_VERSION` stays 2
   (`crates/hekate-sim/src/event.rs:66`), no dependency was added
   (`apps/hekate-cli/Cargo.toml` and `Cargo.lock` are untouched), and the golden,
   hash-golden, and baseline guards (`tests/golden_trace.rs`, `tests/baseline.rs`)
   passed in the gate run below.
6. **The five gates pass on the final tree** (rerun on `33e301a`, 2026-09-13:
   `cargo test --workspace --all-features` passed with 523 tests and 0 failures;
   `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
   `cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh`
   printed `dependency direction OK`; `tangle check nodes` printed
   `graph check: passed (68 nodes)`).

Outcome complete: the seed bank artifact, its `seed-bank` writer, the
`batch --seed-bank` consumer that records the bank's identity, and the kernel
derivation note are on the committed tree.
