---
context_rev: 1
updated: 2026-09-12T21:34:01Z
summary: The assigned node arrived with updated: 2026-09-13T00:00:00Z while the host clock at handoff rea
braintree_revision: 0.5.0+gba362e3
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Slice-A worker for TAS-030: claim with the bare-id hash, edit the assigned node (refresh updated, replace next, append the Result section), then release with the starting base hash.
Friction: The assigned node arrived with updated: 2026-09-13T00:00:00Z while the host clock at handoff read 2026-09-12T21:33Z (about two and a half hours ahead). The handoff also required refreshing updated to current UTC ISO-8601. Honoring that literally moves updated backwards, so updated stopped being monotonic and a reader cannot distinguish "the coordinator stamped a day-level placeholder" from "a worker regressed the timestamp." The skill defines updated only as "the current UTC ISO-8601 time on every mutation" and gives no rule for an incoming future timestamp, so the worker had to choose between two instructions.
Improvement: State in the skill (or in the worker handoff template) how to treat an incoming updated that is ahead of the host clock: for example, keep max(now, previous updated) and note the clamp, or require coordinators to stamp the real time rather than a day boundary. A second, smaller gap: the slice write set named a new integration-test file but not the public API that file needs, so I had to widen the crate public surface (BroadPhase, Aabb, BodyShape, body_clearance_m, bodies_intersect, CONTACT_EPSILON_M) to satisfy the test; the skill could note that an integration test requires the tested surface to be genuinely public.
