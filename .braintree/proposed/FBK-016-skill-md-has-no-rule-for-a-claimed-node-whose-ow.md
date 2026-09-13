---
context_rev: 1
updated: 2026-09-13T02:54:30Z
summary: SKILL.md has no rule for a claimed node whose own recorded premise is factually wrong. The mutat
braintree_revision: 0.6.0+g3bacaf5
---

Area [[IDX-002-braintree-feedback]].

# Feedback

Attempted: Execute TAS-042 slice E, whose "# Outcome" asserted "the swept time-of-impact query ... no tick consumer uses a cast, so it is not on the tick path", and whose assignment brief repeated that premise. Validating it against crates/tangle-sim showed that safety::scan_pair calls time_of_impact at safety.rs:281 behind the near-miss Lipschitz certificate, reached every tick from SafetyMonitor::observe.
Friction: SKILL.md has no rule for a claimed node whose own recorded premise is factually wrong. The mutation rules cover summary/next/status edits, context_rev semantics, and escalating a seam change another node owns, but not "the recorded premise I was handed contradicts the code". The worker had to choose between recording the corrected baseline in # Result, which contradicts the node text it was told to measure, or stopping. There is also no guidance on whether correcting a recorded premise is a consumer-relevant semantic change that warrants a context_rev bump: the read-and-execute loop says to execute the smallest coherent unit and update summary/next/evidence/status, which reads as permission to fix the premise, while "a change that alters a landed seam another node owns ... is escalated" reads as permission to stop.
Improvement: Add one line to SKILL.md's read-and-execute loop: "If validating a node against the code shows a recorded premise or # Outcome statement is wrong, record the corrected state in # Result with the evidence, bump context_rev when a pinned consumer relied on the premise, and report the correction; escalate only when the correction would change the declared scope, outcome, or Done when." Also state explicitly whether a parent that repeats the wrong premise is corrected by the worker or by the coordinator, so the rule is not left to inference.
