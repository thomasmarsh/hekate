---
context_rev: 1
status: proposed
updated: 2026-09-15T20:10:01Z
summary: Add authored wheelbase and heavy turning-limit feasibility diagnostics.
next: Author the per-template wheelbase, derive the turning-limit curvature from it, and add the constant-radius feasibility fixture and inability-to-proceed diagnostic.
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
