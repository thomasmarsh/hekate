---
context_rev: 1
status: proposed
updated: 2026-09-15T14:20:37Z
summary: Prune non-load-bearing workspace test work without weakening coverage.
next: Audit each test binary for wasteful or duplicate work and record a remove/gate/keep decision per candidate.
---

Parent [[TAS-137-cut-the-rust-dev-loop-and-workspace-test-wall]].

# Outcome

The workspace suite spends its wall time on behavior that matters: redundant,
non-load-bearing, or disproportionate test work is removed or gated, and the
remaining suite still protects the same behavior.

# Done when

- Each audited test binary/module has a recorded decision (keep, remove, gate)
with the evidence that justifies it.
- Removed or gated work names the behavior that a kept test still protects, so
no meaningful coverage is lost.
- Workspace gate wall time and test count are measured before and after using
the cargo-nextest runner adopted in the sibling node.
- `cargo nextest run --workspace` passes with no new ignores that mask a real
failure.

# Context

Orientation for the audit, measured 2026-09-14 in TAS-137: 148 s over 83 test
binaries. The audit separates behavior coverage (model/sim correctness,
determinism and goldens, schema, CLI/TUI/presenter contracts) from
implementation-detail or duplicated assertions.
