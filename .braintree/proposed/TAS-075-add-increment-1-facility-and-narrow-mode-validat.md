---
context_rev: 1
updated: 2026-09-13T18:29:05Z
summary: Add Increment 1 facility and narrow-mode validation rules with stable diagnostics.
next: Add Increment 1 facility and narrow-mode validation rules with stable diagnostics to validate_v2.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

`validate_v2` enforces the Increment 1 rules against the compiled facility and
mode-template geometry: geometric containment of a facility in the traversable
world, usable width after the body envelope and clearance, reference-path
curvature against the mode's turning limits, connector continuity, directional
reachability, mode-to-facility access, legal versus physically possible routes,
and spawn clearance for the largest eligible body. It also fixes the narrow
wheeled family's profile-parameter set (including steering response and
lateral-clearance preference) so a `bicycle`/`scooter` template is accepted
exactly when its profiles are complete and well-formed. Every rule has a stable
diagnostic code.

# Done when

- Each new Increment 1 validation rule has a stable `DiagnosticCode`, a rejecting
  fixture that triggers it, and an accepting fixture that does not.
- A malformed facility (outside the world, too narrow for the largest eligible
  body, curvature beyond a mode's turning limit, a discontinuous connector, an
  unreachable direction, or an access violation) is rejected with the naming
  diagnostic, and a legal-but-physically-impossible route is distinguished from an
  illegal one.
- `validate_v2` remains total on every existing version-2 fixture; `cargo test -p
  tangle-model` passes with no Phase 1 regression.

# Context

Gated on [[TAS-074-compile-version-2-facility-and-connector-shapes]]. Per
`PHASE_2_PLAN.md` *Schema version 2* (validation list) and *Validation strategy*.
Read [[TAS-073-extend-the-version-2-schema-contract-with-increm]] (the exact
field sets this validates), [[TAS-062-version-2-source-shapes]], and
[[TAS-067-mode-template-compilation]]. Owns `crates/tangle-model/src/validate.rs`
(including the shared `required_profile_params` the template compiler reads), so
[[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]] gates on this leaf.
