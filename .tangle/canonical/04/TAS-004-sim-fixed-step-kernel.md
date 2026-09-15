---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Implement the fixed-step kernel with deterministic constant-speed agents, snapshots, and typed events.
---

# Context

Depends on [[TAS-003-scenario-source-parse]] at context_rev 1.

# Outcome

`hekate-sim` exposes the small kernel API and advances agents at constant speed
along the guide path using the authoritative fixed-step clock, emitting typed
spawn/despawn events and intentionally lossy snapshots.

# Done when

- `step()` advances exactly one configured tick and never reads wall-clock time.
- Hot agent state is stored in dense, stable-order arrays with no hash-map
  iteration in state-affecting logic.
- `hekate-sim` has no Bevy, window, wall-clock, or filesystem dependency.
- Unit tests cover time and tick arithmetic and constant-speed path progress.

# Result

Implemented in `crates/hekate-sim`:

- `time.rs` — `SimTime` is derived only from `tick * step`; `step()` advances
  exactly one configured tick and never reads wall-clock time.
- `config.rs` — `RunConfig` carries the root seed and a `Seconds` fixed step,
  defaulting to the Standard 50 ms preset.
- `agent.rs` — `AgentId` plus struct-of-arrays `AgentStore` with stable
  spawn-order slots; despawn marks a slot dead rather than reordering it.
- `event.rs` — typed `Event::{Spawned, Despawned}` with
  `DespawnReason::ExitedPath`.
- `snapshot.rs` — lossy `Snapshot`/`AgentSample`/`MotionSample` observer view
  gated by `SnapshotDetail::{Position, Full}`.
- `sim.rs` — `Simulation::{new, step, time, snapshot, finish}`, `StepOutput`
  borrowing a reused event buffer, `RunSummary`, and `InitError`.

The initial population is placed immediately along the first guide path at
`vehicle_spacing_m` (centre-to-centre), and the first `step()` announces it with
`Spawned` events before advancing the clock. Vehicles hold constant speed,
despawn at the path end, and `spawn_capacity` rejects a population whose front
bumper would overrun the path. The API is the one planned in `PHASE_1_PLAN.md`
and consumed by [[TAS-005-cli-canonical-trace]] and
[[TAS-006-bevy-viewer-skeleton]].

Evidence: `cargo test --workspace --all-features` passes 18 `hekate-sim` unit
tests plus the crate doctest, covering tick arithmetic, constant-speed path
progress, spawn/despawn ordering, deterministic observations, and a
wall-clock-delay invariance test; `cargo clippy --workspace --all-targets
--all-features` is warning-free; `cargo fmt --all --check` passes; the release
build succeeds; `scripts/check-dependency-direction.sh` reports
`dependency direction OK`, confirming no Bevy dependency reaches `hekate-sim`.

Parent [[TAS-001-phase-1-increment-0]].
