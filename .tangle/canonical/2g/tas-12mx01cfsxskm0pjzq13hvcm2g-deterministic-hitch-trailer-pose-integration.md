---
context_rev: 1
status: resolved
updated: 2026-09-15T22:59:44Z
summary: Deterministic hitch/trailer pose integration and admit wiring for ArticulatedWheeled.
---

Parent [[tas-5vc5c3cbttnvcztafvns26bcs4-add-hitch-integration-runtime-dispatch-and]].

# Context

`Simulation::try_admit` (`crates/hekate-sim/src/sim.rs:3423`) dispatches only on `AgentFamily::WheeledCapsule`/`WheeledBox` today and spawns one `position`/`heading_rad` per agent (`AgentInit`); there is no per-segment pose representation anywhere in `hekate-sim`. `ModeBodySource::ArticulatedChain`, `BodySegment::hitch_offset_m`, and `AgentBody::ArticulatedChain::articulation_limit_rad` are authorable and compilable (tas-4ep58y0syjtnwny4bgcg5q41j7); nothing yet reads them at runtime.

# Outcome

An admitted `AgentFamily::ArticulatedWheeled` agent carries one pose per chain segment; the tractor (lead) segment integrates exactly as a `WheeledBox` does today, and each trailing segment's pose is deterministically driven off the segment ahead of it through its authored `hitch_offset_m`. A jackknife (relative hitch angle beyond the compiled `articulation_limit_rad`) is a typed, observable event, never a silent clip.

# Done when

- `Simulation::try_admit` spawns an `ArticulatedWheeled` agent from its own template, mirroring the `WheeledBox`/`WheeledCapsule` spawn wiring (`sample_mode_template_profile` precedent).
- Every trailing segment's pose is a deterministic function of the segment ahead of it and its authored hitch offset; re-running the same seed reproduces byte-identical segment poses.
- A jackknife beyond `articulation_limit_rad` raises a typed event/diagnostic rather than clipping the body through a boundary.
- At least one straight-path and one constant-radius fixture (analogous to `heavy_isolated_v2`/`heavy_turning_v2`) admits a `tractor_semitrailer` and holds every segment pose inside a declared tolerance of its analytic or high-resolution reference trajectory.
- Existing `WheeledBox`/`WheeledCapsule`/`HolonomicCircle` spawn and update paths stay byte-identical (inc2/inc3 goldens unchanged).
- The five gates pass.

# Result

`Simulation::try_admit` now routes a template whose compiled family is
`AgentFamily::ArticulatedWheeled` through a new `sample_articulated_chain_profile`
(`crates/hekate-sim/src/profile.rs`), which draws the lead segment's shared
`VehicleProfile` (read by entry admission, collision, and the longitudinal
controller exactly like a `WheeledBox`'s) plus every segment's own sampled
length/width/hitch-offset and the chain's articulation limit, in one documented
draw order from the same two per-agent streams. Right after `AgentInit` is
pushed, `try_admit` sets `AgentStore::articulated[agent]` (a new, additive
`Vec<Option<ArticulatedState>>` column, `None` for every other agent) and
overrides `body_kind` to `BodyKind::ArticulatedChain`; the lead segment's
`position`/`heading_rad` stay the columns `advance_physics` already writes for
a `WheeledBox`, untouched by any new code.

Kinematics (`crates/hekate-sim/src/articulated.rs`, new module): each segment
exposes a fixed body-frame **rear reference point** (half its length behind its
centre, along its own heading — for the lead segment, its assumed unauthored
fifth-wheel location) and, for a trailing segment, a **kingpin**
(`hitch_offset_m` behind its own front edge) that must coincide exactly with
the rear reference point of the segment ahead. This is the classical
single-axle "kingpin trailer" model: the rear reference point is a
non-holonomic axle with no lateral slip, giving the standard heading-rate
equation `theta' = (v_h . perp(theta)) / L` (`v_h` the finite-differenced
velocity of the driving rear-reference point, `L = length - hitch_offset`).
`Simulation::advance_articulated`, called once per tick in `step_agent` right
after `advance_physics`, integrates this with explicit Euler and then
**exactly** repins the segment's centre to the driving segment's new rear
reference point using the new heading — so the physical joint never drifts,
only the heading carries first-order integration error. Nothing here reads an
RNG, so the same seed reproduces byte-identical segment poses. A chain enters
the world already aligned (`ArticulatedState::spawn`), matching every fixture's
initial condition.

Jackknife: `Event::ArticulationLimitExceeded { agent, hitch_index, angle_rad,
limit_rad, exceeding }` is a new, additive `Event`/`EventKind` variant, edge-
triggered exactly like `Event::Collision`'s `contacting` flag (once when a
hitch's absolute body-heading angle crosses above `articulation_limit_rad`,
once when it falls back inside) rather than repeated every tick — a
deliberate refinement of the node's minimal "emit every tick while exceeded"
suggestion, kept for consistency with every other `Event` variant's documented
edge-triggered contract. It lands under `EVENT_VERSION` 3 unbumped, following
the `ClosePass` precedent: a new variant no pre-existing scenario can ever
emit, so no golden trace is regenerated by adding it. `Simulation::snapshot`'s
existing `MotionSample::segments: Vec<BodySegmentSample>` field (already
documented as "an articulated chain later fills one pose per segment") is now
populated for an articulated agent: the lead segment's pose first, then every
trailing segment's, in chain order — this is how fixtures and any future
renderer read chain poses, with no new accessor added.

Two runtime-reachable fixtures (`scenarios/phase2/inc3/
tractor_semitrailer_straight_v2.json5`, `..._turning_v2.json5`), each the
`tractor_semitrailer` chain (6.0 m tractor, 13.6 m semitrailer, 1.2 m kingpin
setback, 0.9 rad limit — the same geometry `tractor_semitrailer_v2.json5`
authors) as the only vehicle on its path, proven by
`crates/hekate-sim/tests/inc3_articulated.rs` (2 tests): on the straight
fixture every admitted agent's trailer heading and offset behind the tractor
match an exact closed form at every tick, regardless of speed profile; on the
80 m-radius turning fixture (120 chords — finer than `heavy_turning_v2`'s 24,
because the trailer's heading responds to each chord's heading discontinuity,
not only its position sagitta), the physical hitch joint and the compiled
limit are checked for every admitted agent every tick, and the first-admitted
(always free-flowing) agent's settled hitch angle is held within 0.01 rad of
the classical single-axle steady-state off-tracking prediction `delta +
asin(L / R_hitch)` (`delta` the phase correction between the tractor's own
rigidly-offset rear-reference point and its body heading — the test's own
comment derives it; measured residual ~0.003 rad).

Forced-closure edit outside the slice's original write set, required and
approved before being made: `crates/hekate-model/src/validate.rs`'s demand-
choice match accepted only `(Movements, SingleBodyWheeled)` and `(Routes,
HolonomicWalking)`; a `rate`+`movements` demand assigned to an
`ArticulatedWheeled` mode — the only way to admit a chain via demand at all —
always failed `E_DEMAND_CHOICE_MISMATCH`. Extended the match to accept
`(Movements, SingleBodyWheeled | ArticulatedWheeled)`, reusing
`validate_demand_movements` unchanged (it was already motion-kind-agnostic).
This does not touch `mode_turning_limit_curvature`/`facility_curvature_diagnostics`
(both stay gated on `wheelbase_m`, which no articulated template authors), so
off-tracking/corner-curvature validation stays the sibling node's, per scope.

Every existing exhaustive `match` over `hekate_sim::Event`/`EventKind` across
`hekate-present`, `hekate-cli`, `hekate-tui`, and `hekate-viewer` gained the
new arm (compile-time forced, not a design choice); `apps/hekate-cli/src/
trace.rs`'s `EventRecord` gained the new variant's fields
(`hitch_index`/`angle_rad`/`limit_rad`/`exceeding`), appended last per its own
"no earlier variant's line changes byte" convention, with a pinned
serialization test case.

Validation on the final tree: `cargo test --workspace --all-features` green
(0 failed across every one of 102 reported suites, including `hekate-sim
--lib` 260 (up from 257) and the new `inc3_articulated` suite's 2);
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
`cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh` OK;
`tangle check` passed. `WheeledBox`/`WheeledCapsule`/`HolonomicCircle` spawn
and update code paths are untouched by this change (only additive columns and
an additive post-`advance_physics` call were added), so inc2/inc3 goldens are
unaffected — no golden file was touched.

Friction: the demand-choice validation gap above was not visible from the
node text or its cited precedent alone; it surfaced only when attempting to
author a real runtime-reachable fixture, which is exactly the kind of gap the
node's own "if this turns out too big" escape hatch anticipates, though here
the fix was small enough to absorb inside this slice rather than deferring it.
