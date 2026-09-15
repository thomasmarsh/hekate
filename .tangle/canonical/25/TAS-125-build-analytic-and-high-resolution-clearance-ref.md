---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T03:20:00Z
summary: Build analytic and high-resolution clearance reference cases.
next: Run the five-gate validation and resolve.
---

Parent [[TAS-104-bound-predicted-versus-executed-clearance]].

# Outcome

Each accepted maneuver family has a hand-computable or high-resolution reference
minimum for front, rear, side, and swept clearance.

# Done when

- Straight constant-velocity, bounded lateral shift, curved reference, and
  body-shape pair cases have hand-computable or high-resolution reference minima
  for front, rear, side, and swept clearance.
- Every reference value is finite and deterministic.

# Context

Extends [[TAS-104-bound-predicted-versus-executed-clearance]]; reads the sim
seams in [[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the
reference cases; the production comparison is the sibling slice.

# Result

Test-only. New `crates/hekate-sim/tests/prediction_reference.rs` (5 tests):
no production file changed.

- **Straight constant-velocity**: the case holds offset zero, so the corridor is
  exactly `x = 50 + 10 t`; front 18.0 m, rear 5.0 m, side 0.6 m, swept 0.6 m,
  each asserted at 1e-9 with its limiting object, time of minimum, and closing
  speed.
- **Bounded lateral shift** (1.5 m across a 7 m band): the reference
  re-integrates the same bounded step at `1/4096 s`, 26x finer than the
  production fine step (`0.05 / DEFAULT_SUBDIVISIONS`), and folds exact and
  swept minima over it. The side body's reference minimum equals the closed-form
  lateral gap at the deepest sample to 1e-4 m, and the predictor reproduces every
  class within 3e-3 m.
- **Curved reference** (offset 0.6 m outside a 1000 m arc, 1 s horizon): the
  band edge is the route-relative constant-width boundary; the reference minimum
  is inside its derived chord-drift and heading-lag sandwich, and the predictor
  reproduces it within 5e-4 m.
- **Body-shape pairs**: box/box, box/circle, circle/circle, and circle/box,
  including a pair that penetrates by 0.1 m; all closed-form at 1e-9.
- Every case asserts the reference integration and the predictor are
  bit-identical on a repeated run and that every reported value is finite.

Verification: `cargo test -p hekate-sim --test prediction_reference` (5 passed),
`cargo test -p hekate-sim --test prediction` (12 passed),
`cargo test -p hekate-sim` (all suites pass), `cargo fmt --all`,
`RUSTFLAGS="-D warnings" cargo clippy -p hekate-sim --all-targets` (clean).

Remaining scope: production prediction versus the executed minimum at each
fidelity preset, and the coarse-endpoint falsification probe, are
[[TAS-126-compare-production-prediction-to-executed-cleara]]'s slice.
