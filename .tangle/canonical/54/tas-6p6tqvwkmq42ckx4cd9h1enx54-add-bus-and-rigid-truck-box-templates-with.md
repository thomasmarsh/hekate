---
context_rev: 1
status: resolved
updated: 2026-09-15T20:55:19Z
summary: Add bus and rigid-truck box templates with heavy profile envelopes and per-template spawn.
---

Parent [[TAS-021-phase-2-increment-3-heavy-articulated-vehicles]].

# Context

`PHASE_2_PLAN.md` "Buses and rigid trucks" (around line 177) fixes the intent: oriented rectangular bodies with mode-specific dimensions, wheelbase, turning limits, acceleration/braking envelopes, and desired-speed profiles, whose controllers reuse the ordinary following, signal, priority, yielding, and lateral logic rather than a new dynamics law.

Today every box mode is placed from the `passenger_car` template: `Simulation::try_admit` samples `self.scenario.profiles()` (`crates/hekate-sim/src/sim.rs:3451-3455`), and only the template whose id is `passenger_car` reaches the v1 profile view (`crates/hekate-model/src/compiled.rs:4011`, `:4103`). This child owns the heavy templates and that spawn wiring. Authored wheelbase and turn feasibility belong to the next child; articulation is out of scope.

# Outcome

`bus` and `rigid_truck` are authored mode templates with heavy box dimensions and heavy IDM envelopes; they compile and validate, and an admitted bus or rigid truck drives with its own body length/width and sampled dynamics rather than the passenger car's.

# Done when

- `bus` and `rigid_truck` authored templates compile and validate, with their required heavy profile params checked in `crates/hekate-model/src/validate.rs` (`required_profile_params`).
- An admitted bus/rigid-truck agent's `body_length_m`/`body_width_m` and sampled dynamics come from its own template, and the `passenger_car` spawn path stays byte-identical so the existing inc2 goldens do not shift.
- `scenarios/phase2/inc3/heavy_isolated_v2.json5` runs a straight bus and rigid-truck fixture, with a test pinning their dimensions and a bounded drive.
- `schemas/scenario-source.schema.json` is regenerated and the schema drift test passes.
- The five gates pass.

# Result

A wheeled box mode template now spawns its own authored body and dynamics.
`crates/hekate-sim/src/profile.rs` gains `sample_mode_template_profile`, which
reads a template's `AgentBehaviorProfile` and its `AgentBody::Box` ranges and
draws in exactly `sample_profile`'s order (speed, length, width, time gap,
maximum acceleration, comfortable braking from the `profile` stream; compliance
from the `compliance` stream), and `Simulation::try_admit` routes a template
whose compiled family is `AgentFamily::WheeledBox` to it. The capsule path and
the version-1 path (`sample_profile(self.scenario.profiles(), …)`) are
unchanged, and no model schema, `BodyKind`, or template id participates.

Determinism: the `passenger_car` template is itself a wheeled box, so its spawn
now goes through the new sampler and must sample what the version-1 view samples
for it. The inc2 goldens are byte-identical — `cargo test -p hekate-cli --test
inc2_trace -- --ignored` (13 passed) and `--test inc2_determinism -- --ignored`
(2 passed) reproduce the checked-in artifacts — and a new unit test,
`profile::tests::a_template_sampled_passenger_car_matches_the_version_1_view`,
pins the equality directly on **non-degenerate** ranges, which the inc2 fixtures
cannot observe because they author constant (`min == max`) profiles: swapping two
draws makes that test fail.

Fixture and evidence: `scenarios/phase2/inc3/heavy_isolated_v2.json5` authors one
straight 600 m 4.0 m-wide lane carrying `passenger_car` (4.5 m x 1.8 m),
`bus` (12.0 m x 2.55 m), and `rigid_truck` (9.5 m x 2.5 m), each with a constant
heavy envelope (bus 9.0 m/s, 0.9 m/s^2, 1.8 m/s^2, 1.6 s; rigid truck 8.0 m/s,
0.8 m/s^2, 1.6 m/s^2, 1.8 s), one through movement, and one demand source per
mode. `apps/hekate-cli/tests/inc3_heavy.rs` runs it at seed 0 for 400 ticks
(20 s simulated at the 0.05 s Standard step; 0.11 s wall) and holds every live
vehicle to its own template: each realized body
length identifies exactly one compiled template, its width and sampled
dynamics lie inside that template's ranges, all three templates are admitted,
no speed leaves `[0, desired]`, and no two bodies on the path overlap. Made to
sample the version-1 view again, the test fails with every box mode realizing
the car's 4.5 m body.

Two forced-closure edits outside the slice's original write set, both caused by
the change and approved before they were made:

- `docs/benchmark-matrix.md`, `docs/benchmark-matrix.json`, and
  `apps/hekate-cli/tests/benchmark_matrix.rs` move `heavy_isolated_v2` from §7.3
  planned to §7.2 checked-in (a new `checked_in_increment_3` group the gate now
  reads), exactly as the Increment 1 and 2 fixture slices did; the gate requires
  a planned path not to exist on disk, and the fixture path is the one the matrix
  names.
- `crates/hekate-sim/tests/narrow_no_car_defaults.rs`: TAS-070's falsification
  probe constructed the version-1 substitution with a **rate** demand on a box
  mode, which this change makes impossible — every valid movement demand's mode
  is `single_body_wheeled` and both families now sample their own template. The
  probe is rebuilt on the still-reachable construction: a `population` demand
  (the version-1 placement `Simulation::new` puts on the guide path from
  `v2_to_v1_view`'s `passenger_car` body) on a facility whose only permitted mode
  authors 1.8 m x 0.7 m, so the agents' realized bodies are the car profile's
  and the check still reports them. Note for TAS-070's owner: the check's
  per-agent body-length/width/dynamics comparison sits behind a `no narrow
  profile` `continue`, so no agent without a narrow profile reaches it, and the
  probe's car-substitution evidence is the check's family/no-narrow-profile
  violations plus the realized body read from the snapshot — as it was before
  this slice.

Validation on the final tree: `cargo fmt --all --check` clean; `cargo test -p
hekate-sim` 34 suites green (incl. `--lib` 257); `cargo test -p hekate-cli`
green; `cargo test -p hekate-cli --test benchmark_matrix` 5 green; `cargo test -p
hekate-sim --test narrow_no_car_defaults` 2 green; the model schema drift tests
green with no schema change (no `crates/hekate-model` edit); `tangle check
--allow-pending-advance TAS-021-phase-2-increment-3-heavy-articulated-vehicles`
passed.

Remaining scope, deliberately out of this child: authored wheelbase, turning and
curvature feasibility, articulation, transit/occupancy, lateral maneuvers, and
the heavy following, articulated reference, and articulated conflict fixtures
(`heavy_following_v2` stays planned).
