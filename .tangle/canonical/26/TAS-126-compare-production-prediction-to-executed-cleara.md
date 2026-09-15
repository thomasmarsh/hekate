---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T03:41:36Z
summary: Compare production prediction to executed clearance with an endpoint probe.
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

# Result

Test-only. `crates/hekate-sim/tests/prediction.rs` gained a TAS-126 section
(3 new tests, 15 in the suite): no production file changed.

- **Straight constant-velocity** (front/rear/side bodies, closed form): the
  predicted and executed minima are the closed-form 18.0, 5.0, and 0.6 m at
  every preset, so `|predicted - executed| = 0` exactly at Fast and Standard and
  below 5e-12 m at Fine.
- **Bounded lateral shift** (1.5 m across a 7 m band, side body closing): the
  worst `|predicted - executed|` is 0.0139 m at Fast, 0.0071 m at Standard, and
  0.0028 m at Fine, each class and the swept minimum meeting the declared
  `T-O1 <= 0.10 m` without any widening, and the executed minimum within the
  same 0.10 m of the `1/4096 s` fine-step reference `T-O1` is declared from.
- **Coarse-endpoint probe**: at the coarsest Fast step a body passing the
  holding agent at 200 m/s keeps the whole near-pass inside one executed step, so
  the endpoint-only fold reports 2.5 m (2.85 m from the body alone) while the
  exact swept minimum inside the step is 1.0 m and the prediction is 1.0 m. The
  endpoint-only fold breaks `T-O1` by 1.5 m against both the prediction and the
  reference, and the probe is repeated at the Standard preset; an endpoint-only
  implementation is therefore falsified, while the sampled-and-swept fold holds.
- Failures report predicted, executed, reference, error, preset, pair, and
  maneuver state (the prediction's verdict).

Verification: `cargo test -p hekate-sim --test prediction` (15 passed),
`cargo test -p hekate-sim` (all suites pass), `cargo fmt --all`,
`RUSTFLAGS="-D warnings" cargo clippy -p hekate-sim --all-targets` (clean). The
fewer-than-0.02 m measured worst case leaves the bound unexercised near its
limit, and a pass straight through the agent's centre inside one coarse step
would exceed `T-O1` because `tick_minimum_clearance_m` reports the overlap
boundary rather than the penetration depth; that case is outside the matrix's
overtaking cadences and no production change was needed or made.

## Gate validation (fresh gate worker)

Five gates run from the repo root on `e2e0079` (`test(sim): bound predicted
versus executed clearance`), in order:

| gate | command | exit | wall | result |
| --- | --- | --- | --- | --- |
| fmt | `cargo fmt --all --check` | 0 | 1.0s | clean |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1.0s | clean |
| test | `cargo test --workspace --all-features` | 0 | 201.0s | 982 passed, 0 failed, 1 ignored, 86 suites |
| dependency direction | `./scripts/check-dependency-direction.sh` | 0 | 1.0s | `dependency direction OK` |
| graph | `tangle check` | 0 | 0.0s | `graph check: passed (194 nodes)` |

No production file changed, so no mechanical fix was needed; the artifact is
test-only (`crates/hekate-sim/src/` untouched) and `tests/prediction.rs` holds
15 tests.
