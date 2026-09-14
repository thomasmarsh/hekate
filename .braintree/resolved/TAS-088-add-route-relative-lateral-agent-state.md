---
context_rev: 1
priority: P1
updated: 2026-09-14T00:17:21Z
summary: Add route-relative lateral, target, corridor, and maneuver state to wheeled agents.
---

Parent [[TAS-087-continuous-lateral-motion-and-gap-machinery]].

# Outcome

Eligible single-body wheeled agents carry deterministic current s/d, target
offset or transition, predicted-gap summary, claim identity, and maneuver state
alongside their authoritative world pose.

# Done when

- The stable agent store initializes route coordinates from compiled facility
  projection and records following, preparing, committed, returning, and aborted
  plus the target clearance/horizon and target facility when applicable.
- Passenger cars and narrow agents share one physical-family representation;
  pedestrians and legacy version-1 paths retain their current state and output.
- State changes occur only through the existing controller-stage pipeline and
  stable AgentId order; no hash-map iteration determines behavior.
- Snapshot and canonical trace expose optional route s/d, target offset,
  predicted gap, and maneuver state under an explicit format/version change;
  sparse lifecycle events remain TAS-100 scope.
- Unit tests cover spawn initialization, absent-state compatibility, stable
  storage, serialization, and projection signs in both traversal directions.

# Context

Gated on [[TAS-085-compile-increment-2-policy-and-traversal-semantics]]. Owns
the smallest required seams in crates/tangle-sim/src/agent.rs, stage.rs,
snapshot.rs, sim.rs, and their focused tests. Do not integrate lateral motion,
choose passes, or emit maneuver events.

# Result

TAS-088 is complete. Eligible steering agents now carry authoritative
route-relative tactical state seeded from the compiled facility projection,
exposed through a full snapshot and a versioned trajectory artifact, while world
pose stays collision and output truth and every Increment 1 consumer is
unchanged apart from the declared artifact version bump.

**State representation** (`crates/tangle-sim/src/agent.rs`,
`crates/tangle-sim/src/stage.rs`):

- `stage::ManeuverState` replaces the two-variant `Commitment` record and names
  the five lifecycle states exactly as `docs/schema-v2-contract.md` *Maneuver
  lifecycle* fixes them — `Following`, `Preparing`, `Committed`, `Returning`,
  `Aborted` — with stable lowercase `label()` and `from_label()`. It is the same
  enum the `Tactic` record carries (`Tactic::maneuver_state`, renamed from
  `commitment`), so no state is spelled differently anywhere else. The
  Increment 1 tactic mapping is unchanged (a free/follow tactic is `Preparing`,
  an active control `Committed`), so no trajectory or event changes.
- `agent::RouteState` holds `facility: FacilityId`, `s_m`, `d_m`, the active
  `maneuver`, the optional `target_offset_m`, `target_facility`,
  `predicted_min_clearance_m`, and the mode's resolved `target_clearance_m` and
  `horizon_s`. `RouteState::project` maps a world pose onto a
  `CompiledReferencePath` and mirrors the signed offset into the agent's own
  travel frame; `RouteState::reproject` refreshes only the geometry after
  integration.
- `AgentStore` gains a `route_state: Vec<Option<RouteState>>` column and
  `AgentInit` a `route_state` field, so the store initializes the coordinates at
  spawn and a dead slot keeps them: the column never shifts.

**Spawn and pipeline** (`crates/tangle-sim/src/sim.rs`):

- `Simulation::route_state_for` finds the compiled facility whose reference path
  is the agent's route and that permits its mode, then projects the entry pose.
  The only discriminator is the compiled `AgentFamily`: a `WheeledBox` and a
  `WheeledCapsule` take the one path and a `HolonomicCircle` returns `None`, so
  passenger cars and narrow agents share one physical-family representation with
  no mode or template-id branch. The mode's `CompiledModeTemplate::lateral()`
  seeds the target clearance and horizon when authored and leaves them absent
  otherwise.
- `advance_physics` (stage 4) reprojects the integrated pose back into the
  facility frame, so the route coordinates track world truth and no target
  offset is ever written to the pose. State changes only through the existing
  controller-stage pipeline in stable `AgentId` order; the facility lookup
  iterates the compiled `Vec<CompiledFacility>` in declaration order, never a
  hash map.
- A version-1 demand (no mode), the walking-skeleton population, and every
  pedestrian carry `None`, so absent state stays absent.

**Output surfaces:**

- `SnapshotDetail::Full` gains `MotionSample::route_state:
  Option<RouteStateSample>` (`crates/tangle-sim/src/snapshot.rs`), the additive
  optional view of the state; the position view is unchanged.
- The trajectory artifact (`apps/tangle-cli/src/trajectories.rs`) appends five
  nullable columns — `route_s_m`, `route_d_m`, `target_offset_m`,
  `maneuver_state`, `predicted_min_clearance_m` — and
  **`TRAJECTORY_FORMAT_VERSION` is bumped 2 -> 3** (one bump for this additive
  column union, the constant `docs/schema-v2-contract.md` *Definition-version
  bumps* assigns to this leaf). Column names, types, order, and nullability are
  checked on read; absent state is `null`, never `0`.
- **No `tests/golden/*.trace.jsonl` or `.sha256` golden changed, and none was
  regenerated.** The canonical trace is the event record; this leaf adds no event
  variant (sparse lifecycle events are TAS-100), so every checked-in trace hash
  is byte-identical. `SCENE_FORMAT_VERSION` and the presenter goldens are
  unchanged: the scene text reads only the fields it already read, and the one
  `MotionSample` literal in `crates/tangle-present/src/scene.rs`'s test module
  gained `route_state: None` to compile.

**Compiled accessors read** (all landed by TAS-085): `CompiledScenario::demand_mode`,
`CompiledScenario::mode_template` -> `CompiledModeTemplate::family()`,
`CompiledModeTemplate::lateral()`, `CompiledFacility::reference_path()`,
`CompiledFacility::permits_mode()`, `CompiledFacility::reference()`,
`CompiledFacility::id()`, and `CompiledReferencePath::project()`. The leaf did
not need `CompiledScenario::traversal_policy`, `transitions`, or
`maneuver_policy`.

**Tests:**

- `crates/tangle-sim/src/agent.rs` unit tests: stable spawn order and columns;
  `projection_mirrors_the_signed_offset_in_the_travel_frame` (forward `+1.5`,
  reverse `-1.5`, so the projection sign is proven in both traversal
  directions); `projection_carries_the_maneuver_policy_and_follows`;
  `route_state_is_stored_per_slot_and_survives_a_despawn` (stable storage);
  `every_maneuver_state_is_recorded_and_labeled` (all five states stored and
  label round-trip).
- `crates/tangle-sim/tests/route_relative_state.rs` (new, 7 tests): spawn
  initialization from the facility projection (s matches progress, d = 0,
  `Following`, target clearance/horizon from the compiled lateral policy, target
  offset/facility/predicted gap absent); pipeline reprojection after integration
  in stable `AgentId` order; the compiled maneuver policy recorded on every
  steering agent; a version-1 population carrying no route state; route state
  exposed only at full detail; a box-body mode sharing the one representation;
  and a mode without `lateral` carrying absent targets.
- `crates/tangle-cli/src/trajectories.rs` tests: declared columns/schema and
  nullability, the version constant, a recorded sample carrying the kernel
  values, the optional-column round trip including absence, and a real narrow
  run recording non-null route coordinates into the artifact.
- `crates/tangle-sim/src/sim.rs` test
  `a_full_snapshot_reports_each_phase1_body_kind_with_no_segments` now also
  asserts every legacy version-1 body carries `route_state == None`.

**Acceptance** (exact commands and observed results):

- `cargo test -p tangle-sim` — 150 lib + 7 `route_relative_state` + all
  existing suites pass, 0 failed (e.g. `narrow_longitudinal` 1,
  `narrow_mode_no_branch` 3, `mixed_interaction` 6).
- `cargo test --workspace` — 77 suites pass, 0 failed, including
  `tangle-cli` (`golden_trace` 2, `baseline` 4, `migration_regression` 4,
  `increment6_trace` 2, `trajectories` 10, `run_directory` 11),
  `tangle-present` (`scene_golden` 2, `v2_fixtures` 3), and the sim suites. No
  golden regenerated, so every checked-in trace hash is unchanged.
- `scripts/check-dependency-direction.sh` — `dependency direction OK` (no new
  dependency; no filesystem, UI, or wall-clock type enters the kernel).
- `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
  --all-features` — clean; `cargo fmt --all -- --check` — clean.
- `braintree check` while TAS-088 was `proposed` — `graph check: passed (154
  nodes)`.

## Remaining scope (recorded, not deferred to a new node)

- `RUN_MANIFEST_VERSION` (`apps/tangle-cli/src/run_dir.rs`): the contract's
  version-bump table assigns the manifest-shape bump to this leaf, but the value
  it records — the resolved lateral-decision cadence and prediction horizon —
  is run-config/fidelity data owned by TAS-087/TAS-089, and `run_dir.rs` is
  outside this leaf's write set. The manifest still records
  `TrajectoryArtifact.format_version = TRAJECTORY_FORMAT_VERSION` (now 3), so the
  artifact version is not silently stale; the manifest-shape bump landed not in
  this session and has no owner yet. It is recorded here as ledger scope.
- The maneuver lifecycle transitions (`following -> preparing -> committed ->
  returning`/`aborted`, claim arbitration, prediction) remain
  [[TAS-089-integrate-bounded-single-body-steering]],
  [[TAS-090-predict-maneuver-corridors-and-clearance]], and
  [[TAS-091-resolve-gap-claims-and-maneuver-transitions]]; this leaf supplies the
  state representation they drive and never selects a maneuver.
- The `maneuver_kind`, `perceived_rule`, and `opposing_direction` trajectory
  columns and the sparse lifecycle events remain TAS-100/TAS-102 scope under the
  version this leaf lands.

## Friction (for the session FBK; no FBK node was created)

- Attempted: satisfy the `# Done when` phrase "Snapshot and canonical trace
  expose optional route s/d ... under an explicit format/version change".
  Friction: the canonical trace (`tests/golden/*.trace.jsonl`) is an event-only
  record; per `docs/schema-v2-contract.md` *Metrics and output* the per-agent
  optional columns live in the sampled snapshot and the trajectory artifact, and
  the sparse state records are the separate event union TAS-100 owns. The
  Done-when conflates the two surfaces, so the leaf implemented the snapshot and
  the trajectory artifact (and bumped `TRAJECTORY_FORMAT_VERSION`) and left the
  event surface to TAS-100. Improvement: name the trajectory artifact explicitly
  in the Done-when, or state that "canonical trace" means the run's durable
  per-agent artifact.
- Attempted: land the whole definition-version closure the contract assigns this
  leaf. Friction: the contract's bump table gives TAS-088 the
  `RUN_MANIFEST_VERSION` bump, but its recorded values are run-config data from
  other leaves and `apps/tangle-cli/src/run_dir.rs` is not in this leaf's write
  set. Improvement: assign the manifest bump to the leaf that lands the
  cadence/horizon config, or include `run_dir.rs` in the write set.
- Attempted: retrofit the two-variant `Commitment` record into the five-state
  maneuver lifecycle. Friction: the contract says the same tactical record
  carries the five states and "no state is spelled differently", but its
  Increment 1 `Preparing` for a free-flow tactic reads as `following` under the
  new state definitions; which mapping is intended is not stated. This leaf kept
  the Increment 1 mapping (no output change) and left the lifecycle unification
  to TAS-091. Improvement: state in the contract which phase-1 tactic maps to
  `following`.
