---
context_rev: 1
updated: 2026-09-13T14:15:14Z
summary: Three coordination gaps. (1) The prescribed slice plan bundled several deliverables per letter,
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Coordinated Phase 1 Increment 6 as one fresh async worker per prescribed slice (A-E), blocking with bg_wait and using the two-phase parent-next handshake.
Friction: Three coordination gaps. (1) The prescribed slice plan bundled several deliverables per letter, so slice C (runs + comparison report + F6 aggregation + per-metric tolerance) and slice D (convergence-report extension + Fast/Standard/Fine evidence + golden traces + limitations doc + one-command reproduction + a live defect fix) each ran 20-48 minutes; the user flagged the slices as excessively large and the work only fit after several commits and a resume. (2) The async run wrapper's default 30-minute wall clock marked a run that had already committed all of its work as 'failed' (FBK-010 class), and immediate bg_wait re-entry kept returning 'attention required' from a stale needs-attention state even with stopOnAttention:false, so a genuine blocking wait took several attempts. (3) Each slice's parent-next handshake needed a coordinator commit plus a contact_supervisor reply, and one interrupted run needed a separate resume; roughly two extra round trips per slice.
Improvement: Size each prescribed slice as one durable outcome (or require the coordinator to split any slice that would land more than one independently verifiable deliverable), document that the parent-next advance can be folded into the resolving worker's own commit when its write set names the parent, and give bg_wait an explicit wait-through-stale-attention default so a completed-but-timed-out run is reconstructed from its log instead of re-run.
