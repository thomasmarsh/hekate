---
status: proposed
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Compare production prediction to executed clearance with an endpoint probe.
next: Compare production prediction and executed minima under the matrix tolerance and add a coarse-endpoint falsification case.
---

Parent [[TAS-104-bound-predicted-versus-executed-clearance]].

# Outcome

Production predicted minima are bounded against executed sampled and swept minima
under the benchmark matrix tolerance, with a case that falsifies an endpoint-only
implementation.

# Done when

- Production prediction and executed sampled or swept minima are compared under
  the benchmark matrix fixed horizon and preset cadence without widening the
  declared tolerance.
- A coarse endpoint case would miss an inside-step minimum, so the evidence
  falsifies an endpoint-only implementation.
- Failures report predicted, executed, reference, error, preset, pair, and
  maneuver state; the focused suite and cargo test --workspace pass.

# Context

Extends [[TAS-104-bound-predicted-versus-executed-clearance]]; reads
[[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the comparison
and the endpoint probe; the reference cases are the sibling slice.
