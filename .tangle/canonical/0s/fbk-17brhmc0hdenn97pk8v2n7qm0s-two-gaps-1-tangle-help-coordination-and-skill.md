---
context_rev: 1
status: proposed
updated: 2026-09-15T16:17:51Z
summary: Two gaps. (1) tangle help coordination and SKILL.md govern claims, leases, and git worktrees...
tangle_revision: 1.0.0+ga6c1089
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Orchestrated TAS-137 (Rust dev-loop and workspace test wall time) through Tangle: decomposed the legacy numeric TAS-137 into three crypto-id children, ran parallel read-only scouts plus per-slice implementation and build/test agents, and used tangle node decompose, advance, and check throughout.
Friction: Two gaps. (1) tangle help coordination and SKILL.md govern claims, leases, and git worktrees but say nothing about non-worktree shared build resources: a read-only recon scout and a separate build agent ran concurrently, and the scout's cargo command blocked about four minutes on the target-directory and package-cache lock the build agent held, forcing an interrupt and revive; even cargo metadata or list can block. (2) A performance Done-when authored as an absolute wall-clock threshold (materially below 191.62 s) was unfalsifiable on a shared 4-core host whose gate wall varied 194-410 s under background load: the work cut total CPU about 8 percent and removed a 99 s serial pole, yet the threshold could not be shown met because the gate was CPU-bound at its parallel floor.
Improvement: Add a Shared build/test resources subsection to references/coordination.md: treat the Cargo target directory and package cache as an exclusive resource, allow at most one cargo-invoking agent per checkout at a time (route all cargo through a single build agent), and note that even metadata or list commands can block. In references/authoring.md, advise expressing performance acceptance as total resource use (CPU seconds) or a same-session A/B measurement rather than an absolute wall-clock threshold that depends on host cores and ambient load.
