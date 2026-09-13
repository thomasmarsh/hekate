---
context_rev: 1
priority: P1
updated: 2026-09-13T14:34:51Z
summary: Prove every Phase 1 acceptance scenario migrates and reproduces its frozen baseline.
next: Add the migration regression suite over every Phase 1 acceptance scenario and the checked-in baseline.
---

Parent [[TAS-057-schema-v2-migration-and-provenance]].

# Outcome

The Increment 0 gate over schema provenance: every Phase 1 acceptance scenario
runs through either its original reader or the explicit migration path and
reproduces the frozen baseline, and any intentional baseline change carries a
minimal regression fixture and a versioned explanation.

# Done when

- Every checked-in Phase 1 acceptance scenario is exercised through its supported reader or the migration path.
- Migrated runs reproduce the frozen Phase 1 baseline hashes except for declared, versioned corrections.
- Any intentional baseline change has a minimal regression fixture and a versioned explanation recorded with it.
- `cargo test` and `baselines/phase1` checks pass.

# Context

Gated on [[TAS-063-v1-to-v2-migration-and-cli]].
Gated on [[TAS-064-migration-provenance]].
