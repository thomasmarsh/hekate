---
context_rev: 1
status: proposed
updated: 2026-09-15T22:15:46Z
summary: Route-feasibility diagnostics for off-tracking under articulation.
next: Extend the model-layer curvature/route-feasibility validation to reject an authored turn a tractor-semitrailer's hitch geometry cannot track.
---

Parent [[tas-5vc5c3cbttnvcztafvns26bcs4-add-hitch-integration-runtime-dispatch-and]].

# Context

`mode_turning_limit_curvature`/the `E_FACILITY_CURVATURE` check (`crates/hekate-model/src/validate.rs`, from commit ccb3d89) validate a `SingleBodyWheeled` mode's authored turn against its wheelbase/steering-angle curvature bound; tas-4ep58y0syjtnwny4bgcg5q41j7 deliberately left the articulated body out of that check. A trailer's off-tracking under a tight turn is a function of each hitch offset and the path's curvature, independent of runtime pose integration.

# Outcome

An authored route a tractor-semitrailer's hitch geometry cannot track within its `articulation_limit_rad` fails model-layer validation with a stable diagnostic, the same way an infeasible `SingleBodyWheeled` turn does today.

# Done when

- A corner-curvature or off-tracking bound derived from each segment's hitch offset and the compiled `articulation_limit_rad` is checked against every authored facility/path an `ArticulatedWheeled` template can use.
- A rejection test analogous to the `E_FACILITY_CURVATURE` case exists for an infeasible authored turn.
- Existing `SingleBodyWheeled` curvature validation is unchanged.
- The five gates pass.
