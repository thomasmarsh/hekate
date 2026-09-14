---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:12Z
summary: Bound maneuver-prediction error against analytic and fine-step executed clearance.
next: [[TAS-125-build-analytic-and-high-resolution-clearance-ref]]
---

Parent [[TAS-103-increment-2-acceptance-evidence-and-presenters]].

# Outcome

Analytic and high-resolution reference cases quantify the error between each
accepted maneuver's predicted minimum clearance and the clearance executed by
the production integrator at Fast, Standard, and Fine presets.

# Done when

- Straight constant-velocity, bounded lateral shift, curved reference, and
  body-shape pair cases have hand-computable or high-resolution reference
  minima for front, rear, side, and swept clearance.
- Production prediction and executed sampled/swept minima are compared under
  the benchmark matrix's fixed horizon and preset cadence without widening the
  declared tolerance.
- Tests include a coarse endpoint case that would miss an inside-step minimum,
  so the evidence falsifies an endpoint-only implementation.
- Failures report predicted, executed, reference, error, preset, pair, and
  maneuver state; every value is finite and deterministic.
- The focused suite and cargo test --workspace pass with no tolerance change.

# Context

Gated on [[TAS-090-predict-maneuver-corridors-and-clearance]] and
[[TAS-089-integrate-bounded-single-body-steering]]. Owns a focused predictor
reference integration test under crates/tangle-sim/tests and only the smallest
production correction the test reveals. Do not author broad scenario fixtures,
events, metrics, or presenters.

Gates my test artifact enters: Cargo's tangle-sim integration-test discovery and
the workspace warnings-as-errors test gate.

# Slices

- [[TAS-125-build-analytic-and-high-resolution-clearance-ref]] Analytic and high-resolution reference cases.
- [[TAS-126-compare-production-prediction-to-executed-cleara]] Production-vs-executed comparison and endpoint probe.
