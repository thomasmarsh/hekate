---
context_rev: 1
status: proposed
updated: 2026-09-15T20:10:01Z
summary: Author tractor-semitrailer geometry and make the articulated body and motion compilable.
next: Add the articulated-chain body source and articulated-wheeled motion to authoring, compile, and validation, with a checked tractor-semitrailer geometry fixture.
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
