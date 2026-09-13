---
context_rev: 1
priority: P1
updated: 2026-09-13T01:39:18Z
summary: Slice C2a of Phase 1 Increment 5 adds a versioned common-random-number seed-bank artifact and a `seed-bank` CLI command, and lets `batch` run a batch from a seed bank so two variants share the same aligned seeds.
next: Inspect the kernel's seed/stream derivation, then add the SeedBank type + JSON artifact + `seed-bank` command and `batch --seed-bank`, with deterministic-ordering tests.
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
  stays 2; no dependency change in `tangle-model` or `tangle-sim`.

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
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Increment 5 asks for "common-random-number seed banks for A/B comparisons". This
child adds the bank and its `batch` integration; the paired comparison that
consumes it is the next direct child (C2b). The batch manifest already records
each run's seed (TAS-035); this child records the shared bank identity so a
comparison can prove both sides used one bank.

# Result

Pending.
