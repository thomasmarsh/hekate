# Phase 1 baseline

This directory freezes the observable behavior of the current Phase 1 kernel and
records whether the Phase 1 definition of done — the Phase 2 entry gate — is
satisfied. It is the first deliverable of Phase 2 Increment 0
([[TAS-018-phase-2-increment-0-baseline-extension-contract]]).

## Files

- `baseline.json` — deterministic manifest: scenario content hash, model and
  event versions, and the canonical trace hash and run counts at the Fast,
  Standard, and Fine fidelity presets. A test compares it byte for byte against a
  fresh capture, so an unexplained diff is a regression.
- `performance.json` — non-normative wall-clock measurement on one machine at one
  time. It is evidence, not a gate, and differs between hosts and builds.
- `entry-gate.md` — the Phase 1 definition-of-done checklist with evidence and a
  verdict on whether Phase 2 may begin.

## Capture contract

`duration_s` is held constant across presets, not the tick count. Fast, Standard,
and Fine differ in physics step, so fixing ticks would compare different simulated
horizons. The default duration of 12.5 s is exactly 250 Standard steps at the
50 ms step, so the Standard preset hash equals the checked-in golden trace
`tests/golden/walking_guide_v1.trace.sha256`.

The manifest is deterministic and safe to commit. Wall-clock time, build profile,
host OS, and CPU architecture live only in `performance.json`.

## Regenerate

From the repository root:

```sh
cargo run -p tangle-cli -- baseline scenarios/walking/walking_guide_v1.json5 \
  --seed 0 --duration-s 12.5 \
  --output baselines/phase1/baseline.json \
  --performance baselines/phase1/performance.json
```

Then run `cargo test -p tangle-cli` to confirm the checked-in manifest matches.

## Convergence

The walking skeleton is deliberately trivial, so convergence is about which
quantities survive a step change, not statistical agreement. With duration held
constant, all three presets spawn, despawn, and leave behind identical agent
counts; the canonical trace hashes differ only because despawn ticks move with
the step size. The manifest records both facts in `convergence`.

## Scope limit

This baseline covers only what Phase 1 has implemented: constant-speed cars on a
single guide path. It is a baseline of the current kernel, not evidence that the
Phase 1 definition of done is met. See `entry-gate.md`; the next behavior-changing
Phase 2 increment is gated until the Phase 1 gaps close.
