---
context_rev: 1
status: resolved
updated: 2026-09-15T22:02:18Z
summary: Author tractor-semitrailer geometry and make the articulated body and motion compilable.
---

Parent [[TAS-021-phase-2-increment-3-heavy-articulated-vehicles]].

# Context

The component layer already carries `BodyKind::ArticulatedChain`, `BodySegment`, `AgentBody::ArticulatedChain`, `AgentMotion::ArticulatedWheeled`, `AgentFamily::ArticulatedWheeled`, and a `derive_family` arm (`crates/hekate-model/src/components.rs:23-165`, `:977`), but nothing is authorable or compilable: `ModeBodySource`, `MotionKind`, `compiled_body`, and `compiled_motion` exclude them (`crates/hekate-model/src/source.rs:623`, `:651`; `crates/hekate-model/src/mode_template.rs:343`, `:364`).

# Outcome

A tractor-semitrailer mode template is authorable (segment lengths/widths, hitch offsets, articulation limit) and compiles to the existing `ArticulatedChain`/`ArticulatedWheeled` components with validation.

# Done when

- `ModeBodySource` and `MotionKind` gain the articulated variants, and the compile and validate paths accept them with geometry and articulation-limit validation.
- A checked tractor-semitrailer geometry fixture compiles and the schema drift test passes.
- Runtime integration and swept collision stay out of scope and are the next child's; no runtime behavior change is claimed here.
- The five gates pass.

# Result

`ModeBodySource::ArticulatedChain { segments: Vec<ArticulatedSegmentSource>, articulation_limit_rad: ProfileRangeSource }` and `MotionKind::ArticulatedWheeled` are now authorable (`crates/hekate-model/src/source.rs`), with a new `ArticulatedSegmentSource { length_m, width_m, hitch_offset_m: Option<ProfileRangeSource> }`. The compiled component layer gained the fields it was missing to carry this losslessly: `BodySegment` gained `hitch_offset_m: Option<ProfileRange>` (`None` for the lead segment, `Some` for every trailing one) and `AgentBody::ArticulatedChain` gained `articulation_limit_rad: ProfileRange` (`crates/hekate-model/src/components.rs`). `compiled_body`/`compiled_motion`/`compiled_profile` in `crates/hekate-model/src/mode_template.rs` map the new source variants onto these components; the articulated-wheeled profile reuses `AgentBehaviorProfile::wheeled` (Increment 3 claims no new dynamics), matching `required_profile_params`'s new `(_, MotionKind::ArticulatedWheeled)` arm in `crates/hekate-model/src/validate.rs`.

Validation added in `crates/hekate-model/src/validate.rs`:
- `body_envelope_width_m` gained an `ArticulatedChain` arm (widest segment width).
- Two new `DiagnosticCode` variants: `ModeArticulatedSegments` (`E_MODE_ARTICULATED_SEGMENTS`) covers segment count `< 2`, a lead segment authoring a hitch offset, a trailing segment omitting one, and any non-finite/non-positive/inverted segment `length_m`/`width_m`/`hitch_offset_m`; `ModeArticulationLimit` (`E_MODE_ARTICULATION_LIMIT`) covers a non-finite, inverted, or out-of-`(0, pi)` `articulation_limit_rad` — `pi` excluded because at `pi` a trailing segment folds flat back onto the one ahead of it (a jackknife, not a bound on one). `mode_turning_limit_curvature`/`facility_curvature_diagnostics` were deliberately left untouched: off-tracking and corner curvature under articulation stay the next child's, per scope.
- 7 new unit tests in `validate.rs` (`a_valid_articulated_chain_is_accepted`, `an_articulated_chain_needs_at_least_two_segments`, `the_lead_segment_must_not_author_a_hitch_offset`, `a_trailing_segment_must_author_a_hitch_offset`, `an_inverted_segment_dimension_is_rejected`, `the_articulation_limit_must_be_inside_zero_pi`) exercise every new rejection path plus the accept path.

Fixture and test: `scenarios/phase2/inc3/tractor_semitrailer_v2.json5` authors one `tractor_semitrailer` mode template — a two-segment `articulated_chain` (6.0 m tractor, no hitch offset; 13.6 m semitrailer, 1.2 m hitch offset), widths 2.5 m/2.55 m, and a 0.9 rad articulation limit — with no path/portal/facility/demand, since this node authors no runtime-reachable scenario. `crates/hekate-model/tests/articulated_chain_geometry.rs` (3 tests) proves the document parses, `validate_v2` returns no diagnostics, and `compile_mode_template` produces `AgentFamily::ArticulatedWheeled`/`AgentMotion::ArticulatedWheeled` with the segment count, each segment's length/width/hitch-offset, and the articulation limit pinned by assertion (so a regression in `compiled_body` fails this test, not a later one).

Existing call sites updated for the additive fields (all compile-only churn, no behavior change): `crates/hekate-model/src/compiled.rs` (`car_profile_from_template`/`pedestrian_profile_from_template` v1-downgrade paths fold `ArticulatedChain` into their existing zero-range wildcard arms), and `BodySegment::new`/`AgentBody::ArticulatedChain` construction sites in `crates/hekate-model/tests/agent_components.rs` and `crates/hekate-model/tests/mode_template_compilation.rs`.

`schemas/scenario-source.schema.json` regenerated via `cargo run -p hekate-model --example generate-schema`; `docs/benchmark-matrix.json`/`.md` were deliberately left untouched — inspected both, and their `checked_in_increment_*` entries are fixtures a `hekate-sim`/`hekate-cli` benchmark run consumes (declared `CC-*` references and cell tolerances), while `tractor_semitrailer_v2.json5` is a `hekate-model`-only compile/validate fixture with no facility, demand, or runtime path, so it names no benchmark cell to register.

Gate results on the final tree: `cargo test --workspace --all-features` all green (0 failed across every suite, incl. `hekate-model --lib` 85, `hekate-sim --lib` 257); `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean; `cargo fmt --all --check` clean; `./scripts/check-dependency-direction.sh` OK; `tangle check` passed (227 nodes, +1 for the new child below).

New child: `tas-5vc5c3cbttnvcztafvns26bcs4` ("Add hitch integration, runtime dispatch, and swept collision for articulated-wheeled"), created via `tangle node decompose --parent TAS-021-...`, which also advanced `TAS-021`'s `next` to it. It owns deterministic hitch/trailer pose integration, `Simulation::try_admit` dispatch for `AgentFamily::ArticulatedWheeled`, segment-level broad/narrow-phase collision, swept queries, off-tracking/corner-curvature-under-articulation diagnostics, and a runtime fixture.

Friction: none blocking. One judgment call worth flagging for the coordinator: the node text suggested naming the fixture test file "analogous to `mode_template_compilation.rs`, or a small addition there"; I chose a dedicated `articulated_chain_geometry.rs` file instead, matching the `narrow_mode_templates.rs`/`walking_scenario.rs` convention of one file per checked-in fixture rather than growing the general-purpose compilation test file further.
