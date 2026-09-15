---
context_rev: 1
status: proposed
updated: 2026-09-15T21:28:31Z
summary: Two gaps. (1) The only authored steering limit was `steering_rate_max_rad_s`, a heading rate...
tangle_revision: 1.0.0+ga6de4e4
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Implemented the TAS-021 frontier child "add authored wheelbase and heavy turning-limit", whose Done-when requires "the turning-limit curvature derives from the wheelbase and steering limit".
Friction: Two gaps. (1) The only authored steering limit was `steering_rate_max_rad_s`, a heading rate whose Increment 1 rule `|kappa| <= rate / v` is mathematically independent of wheelbase, so that derivation could not be satisfied from the existing seam; the decomposition named no steering-angle primitive even though PHASE_2_PLAN.md line 147 names a "steering-angle" limit, so the worker had to invent the minimal `steering_angle_max_rad` seam and justify it. (2) `reference_max_abs_curvature` reads an authored `paths[].points` chord polyline as zero curvature, so "an infeasible authored turn fails validation" was unreachable for the only path geometry the schema authors; the node did not flag the missing analytic-arc/compiled-curvature path.
Improvement: Have decomposition reject a Done-when that names a derivation input which is not an existing seam and not listed as a new primitive to author, and add a repo note that authored reference paths are curvature-free polylines so any turning-feasibility criterion must state how authored curvature is measured (for example discrete vertex curvature).
