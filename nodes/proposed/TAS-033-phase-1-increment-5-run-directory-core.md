---
context_rev: 1
priority: P1
updated: 2026-09-13T00:07:41Z
summary: Slice B1 of Phase 1 Increment 5 generalizes `run` output into an immutable per-run directory holding a reproducible JSON manifest, a JSON summary, a compressed JSON Lines sparse typed event stream, and a declared sampling policy that bounds output size.
next: Discover the existing `run` output path and artifact format, then implement the immutable run directory (manifest.json, summary.json, compressed event stream) with a declared sampling policy.
---

# Outcome

Generalize the `run` command's output into an immutable per-run directory. A
completed run directory holds:

- `manifest.json` — JSON, self-sufficient to reproduce the run: scenario source
  path and content hash, seed, step, fidelity profile, `event_version`,
  `schema_version`, the simulator/build revision, and the declared sampling
  policy;
- `summary.json` — JSON, the run's aggregate output (counts and the metrics the
  current run already reports), with the metric values it carries traceable to
  the manifest;
- the sparse typed event stream — compressed JSON Lines, one typed record per
  line, in the canonical record order.

The directory is immutable once complete: a run never rewrites a completed run
directory, and a second invocation targeting an existing completed run
directory fails or no-ops without mutating it. The manifest declares the
sampling policy that bounds the output; full trajectories remain opt-in and are
not written by this slice.

The existing canonical trace artifact, its byte format, its golden, the hash
golden, and the Phase 1 baseline must remain unchanged; the run directory is the
enclosing container, and `run`'s existing default behavior stays byte-identical
apart from the added directory.

Constraint: no new dependency in `tangle-model` or `tangle-sim`. A compression
dependency belongs in the CLI/output layer (`apps/tangle-cli`) and must be
reported. No scenario-schema, record-shape, or `EVENT_VERSION` change.

# Done when

- `run` writes an immutable run directory containing `manifest.json` (JSON),
  `summary.json` (JSON), and a compressed JSON Lines sparse typed event stream.
- The manifest is self-sufficient to reproduce the run and records the declared
  sampling policy.
- A completed run directory is never rewritten; a repeat run against it does not
  mutate completed artifacts, covered by a test.
- The existing canonical trace format, trace golden, trace hash golden, and
  Phase 1 baseline are unchanged.
- Tests cover the manifest fields, the compressed event stream round-trip, the
  summary, and the immutability property.
- The five gates pass on the final tree: `cargo test --workspace --all-features`,
  clippy with `-D warnings`, `cargo fmt --all --check`,
  `./scripts/check-dependency-direction.sh`, and `braintree check nodes`.

# Context

Parent [[TAS-031-phase-1-increment-5-experiments-outputs-convergence]].

Slice B of Increment 5 asks for an immutable run directory with manifest,
summary, event stream, and selectively sampled trajectories; JSON for
manifests/summaries, compressed JSON Lines for sparse typed events, and Parquet
for sampled trajectories; and a declared sampling policy bounding output size.

This child owns the manifest/summary/event-stream core and the sampling-policy
declaration. The Parquet sampled-trajectory half is a separate direct child, so
the Parquet dependency does not enlarge this slice. `batch`, `replay`,
aggregation, and the convergence runner consume the layout this child fixes.

# Result

Pending.
