---
context_rev: 1
status: resolved
updated: 2026-09-15T21:27:39Z
summary: Add authored wheelbase and heavy turning-limit feasibility diagnostics.
---

Parent [[TAS-021-phase-2-increment-3-heavy-articulated-vehicles]].

# Context

`PHASE_2_PLAN.md` (around line 181) fixes a conservative swept-turn feasibility check and an explicit runtime inability to proceed. Today the turning limit is only `steering_rate_max_rad_s.max / speed_mps.max` (`crates/hekate-model/src/validate.rs:2341`), and `docs/schema-v2-contract.md` (around line 816) defers an authored wheelbase to Increment 3.

This child follows the heavy box templates child and needs them in place.

# Outcome

An authored per-template wheelbase participates in the turning-limit curvature, an infeasible authored turn fails validation or produces the explicit inability-to-proceed result, and a constant-radius heavy fixture proves the boundary.

# Done when

- `wheelbase_m` is authorable per mode template, validated with range checks, and reflected in the regenerated schema and drift test.
- The turning-limit curvature derives from the wheelbase and steering limit.
- A constant-radius heavy fixture exercises a turn within and beyond the limit; an infeasible turn fails validation or yields the explicit inability-to-proceed diagnostic; bodies never clip boundaries.
- The five gates pass.

# Result

Delivered the whole child: authored axle geometry, the derived turning limit,
and the constant-radius heavy fixture that proves it.

**Authored fields.** `mode_templates[].wheelbase_m` and
`mode_templates[].steering_angle_max_rad` are optional, coupled
`ProfileRangeSource` ranges on `ModeTemplateSource`
(`crates/hekate-model/src/source.rs`). They are additive and absent by default,
so every Increment 0/1/2 template keeps its exact behavior. The generated
`schemas/scenario-source.schema.json` gains both optional properties (regenerated
with `cargo run -p hekate-model --example generate-schema`; the schema drift
tests pass), and the literal constructions in `crates/hekate-model/src/migrate.rs`
and `crates/hekate-model/tests/mode_template_compilation.rs` carry `None`.

**Why a steering-angle seam.** The Done-when's "steering limit" had no authored
angle: the existing `steering_rate_max_rad_s` is a heading rate, and
`|kappa| <= rate / v` is independent of wheelbase, so it cannot yield a
wheelbase-dependent curvature. The slice authors the minimal companion
primitive — a maximum steering angle — exactly as `PHASE_2_PLAN.md` (line 147
names the steering-angle limit; line 185 names articulated steering limits) and
`docs/schema-v2-contract.md` (line 814: the plan's steering-angle limit is
represented by derivative limits *because* no axle geometry exists) anticipate.
The turning limit is now `min(steering_rate_max / speed_max,
tan(steering_angle_max) / wheelbase)` (`mode_turning_limit_curvature`), using the
least-capable range end (longest wheelbase, smallest angle).

**Authored-curve check.** An authored `paths[].points` reference is a polyline,
so `reference_max_abs_curvature` reads zero along it even when its vertices trace
a curve. A mode that authors axle geometry is therefore also held to
`authored_max_corner_curvature`, the direction change at each interior vertex
over the mean adjacent segment length (the true `1 / radius` for points on a
circle); modes without axle geometry keep the Increment 1 rule unchanged, so no
Increment 1/2 fixture moves.

**Validation.** Two stable codes, `E_MODE_WHEELBASE` and
`E_MODE_STEERING_ANGLE`, reject a non-finite/non-positive/inverted range, an
angle outside `(0, pi/2)`, axle geometry on a non-wheeled motion, and either
field authored without its companion.

**Fixture and evidence.** `scenarios/phase2/inc3/heavy_turning_v2.json5` authors
two concentric quarter-circle guide paths (bus R = 40 m, rigid truck R = 44 m)
and a `bus`/`rigid_truck` template with authored geometry; each facility's corner
curvature `1 / R` is inside its mode's limit, so it validates and runs. It is
registered as checked in (`docs/benchmark-matrix.md` §7.2,
`docs/benchmark-matrix.json`) and the slow migration gate
(`migration_regression::every_scenario_migrates_or_is_a_valid_version_2_document`)
passes on it. `crates/hekate-sim/tests/inc3_heavy_turning.rs` pins that every
heavy body corner stays inside the facility half-width on the constant radius
(the no-curb-clip contract; the centre-only corridor check is not enough for a
chord body), and that a longer authored wheelbase (20 m) makes the same fixture
curve infeasible with `E_FACILITY_CURVATURE`. `validate.rs` unit tests cover the
coupling/range rules, the within/beyond-limit arcs, and the chord curvature.

**Gates** on the final tree: `cargo fmt --all --check` clean;
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
`cargo nextest run --workspace --all-features` 1059 passed / 35 skipped;
`cargo test --doc --workspace --all-features` green;
`./scripts/check-dependency-direction.sh` OK; `tangle check` clean.

**Out of this child:** articulated geometry/runtime (the next child) and the
coordinator-owned `docs/schema-v2-contract.md` Increment 3 rows; the paragraph
at line 814 ("no authored wheelbase before Increment 3") is now superseded and
needs the coordinator's Increment 3 section.
