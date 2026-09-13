---
context_rev: 1
priority: P1
updated: 2026-09-13T11:48:58Z
summary: Slice D of Phase 1 Increment 6 lands the reproduction and honesty evidence - golden traces, Fast/Standard/Fine convergence evidence with material sensitivity reported, a known-limitations document, and a one-command reproduction of the comparison.
next: Land golden traces, Fast/Standard/Fine convergence evidence with material sensitivity reported, a known-limitations document, and a one-command reproduction command.
---

# Context

Parent [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]].

Consumes the comparison report and per-metric convergence tolerance from
[[TAS-048-increment-6-run-and-comparison-report]]. The Increment 5 runner
(`TAS-041`) already produces a machine-readable sensitivity report; this slice
records the Fast/Standard/Fine evidence for the Increment 6 comparison and makes
the material-sensitivity outcome explicit. Golden traces follow the Increment 4/5
determinism contract exercised by `replay --verify`.

# Outcome

- Golden traces for the canonical event stream (and, where already supported, the
  sampled trajectory or scene golden), regenerated deliberately if and only if a
  reported change requires it.
- Fast/Standard/Fine convergence evidence for the comparison, recording which
  selected findings are directionally stable at Fine fidelity and which are
  materially sensitive, per metric.
- A known-limitations document naming unvalidated claims, model limitations, and
  accepted boundaries (including the Increment 5 residuals dispositioned by
  [[TAS-045-phase-1-increment-6-first-useful-release-demonstration]]).
- A one-command reproduction of the comparison from the checked-in spec and seed
  bank.

# Done when

- Golden traces exist and are checked in; any regeneration is deliberate and
  reported, and `replay --verify` passes on the same manifest.
- Convergence evidence for Fast/Standard/Fine exists as a machine-readable
  artifact plus a human-readable summary; material sensitivity is reported per
  metric rather than hidden.
- The known-limitations document names the unvalidated claims and the residual
  limitations carried from Increments 4–5.
- One documented command (or checked-in script) reproduces the comparison from
  the checked-in spec and seed bank.
- The five gates pass: `cargo test --workspace --all-features`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, `./scripts/check-dependency-direction.sh`, and
  `braintree check`.
