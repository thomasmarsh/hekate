# Tangle Phase 1 Implementation Plan

**Status:** Proposed  
**Date:** September 11, 2026  
**Scope:** Phase 1, “Trustworthy kernel,” from [VISION.md](VISION.md)

## Outcome

Phase 1 will deliver a graphics-first but headless-capable microscopic intersection simulator. A developer will be able to load a hand-authored scenario, watch cars and pedestrians move through it in a top-down debug viewer, pause and step the simulation, replay the same run exactly, run replications without graphics, and compare two configurations using operational and conflict metrics.

The first visible simulation should exist in the first implementation increment. It is an observability tool, not the simulation host or source of truth.

## Recommendation

Use Rust for the simulator libraries and executables, and Bevy for the viewer. Use the GPU through Bevy's `wgpu` renderer for drawing, but keep Phase 1 simulation compute on the CPU. Keep outputs friendly to later Python, R, or notebook analysis rather than requiring analysis to be written in Rust.

This split fits the goals better than making the simulator a Bevy application:

- Rust provides predictable performance, explicit data ownership, good native tooling, and a credible path to large batch workloads.
- Bevy provides a fast route to a cross-platform, GPU-backed 2D viewer, input, cameras, text, and debug overlays.
- A plain Rust kernel can be run directly by a CLI, tests, benchmarks, or the viewer without wall-clock or window-system dependencies.
- CPU simulation is substantially easier to replay, inspect, test, and validate. Phase 1 scaling should come from cache-conscious state and parallel independent replications, not GPU compute.
- The boundary leaves room for later GPU accelerators. Any accelerator can implement the same operation as a reference CPU path and be checked with differential tests before its results are trusted.

As of this plan, pin Rust 1.98.1 and Bevy 0.19.1 for the initial workspace rather than following floating versions. Review upgrades deliberately at milestone boundaries. Bevy supports fixed schedules and headless applications, but Tangle's authoritative clock should still belong to the kernel rather than Bevy's wall-clock-driven schedule.[^bevy-fixed][^bevy-headless]

## Phase 1 product slice

### Included

- Continuous 2D world coordinates in SI units, using `f64` in the kernel.
- Declarative, versioned intersection scenarios made from general geometry and movement primitives.
- Passenger cars and pedestrians.
- Fixed-time signals and explicit movement priority.
- Simple replaceable longitudinal car control and pedestrian waypoint/collision-avoidance control.
- Contextual red-light running and pedestrian signal noncompliance.
- Broad-phase spatial indexing, exact body queries, swept collision checks, and typed safety events.
- Throughput, delay, queues, violations, collisions, TTC, PET, and minimum separation.
- Reproducible runs, named random streams, manifests, trajectory export, and parallel batch replication.
- A top-down Bevy viewer with live controls, inspection, overlays, and replay.

### Deferred

- GPU simulation, distributed execution, and simulation-wide multithreaded mutation.
- Bicycles, scooters, buses, trucks, articulated bodies, and mode transfers.
- Lane changing, overtaking, free lateral negotiation, and detailed perception error.
- Real-map import, visual scenario editing, automatic calibration, crash-rate prediction, and design optimization.
- Photorealistic art, 3D assets, vehicle suspension, and contact-response physics.

## Architecture

The dependency direction is the most important Phase 1 decision:

```text
scenario.json5
      |
      v
+----------------+      +----------------+      +------------------+
| tangle-model   |----->| tangle-sim     |----->| snapshots/events |
| parse/validate |      | step(dt)       |      | metrics          |
+----------------+      +----------------+      +------------------+
                              |                         |
                    +---------+---------+               |
                    |                   |               |
                    v                   v               v
              +------------+     +---------------+  analysis tools
              | tangle-cli |     | tangle-viewer |
              | headless   |     | Bevy + wgpu   |
              +------------+     +---------------+
```

`tangle-sim` must not depend on Bevy, a window, wall-clock time, or the filesystem. The viewer may depend on every lower layer. This one-way dependency is enforced in the workspace layout and CI.

### Proposed workspace

```text
Cargo.toml
rust-toolchain.toml
apps/
  tangle-cli/       # validate, run, batch, inspect manifests
  tangle-viewer/    # live view and replay using Bevy
crates/
  tangle-model/     # source schema, validation, stable IDs, compiled scenario
  tangle-sim/       # state, clocks, controllers, interactions, events, metrics
scenarios/
  walking/          # tiny development scenarios
  benchmarks/       # versioned correctness and performance scenarios
schemas/            # generated and checked-in JSON Schema
tests/
  golden/           # canonical traces and expected summaries
```

Do not split controllers, geometry, metrics, or I/O into more crates during Phase 1. Keep module seams inside `tangle-sim` until independent compilation or reuse is demonstrated.

### Kernel API

The initial public surface should remain small:

```rust
pub struct Simulation { /* private */ }

impl Simulation {
    pub fn new(
        scenario: CompiledScenario,
        config: RunConfig,
    ) -> Result<Self, InitError>;

    pub fn step(&mut self) -> StepOutput<'_>;
    pub fn time(&self) -> SimTime;
    pub fn snapshot(&self, detail: SnapshotDetail) -> Snapshot;
    pub fn finish(self) -> RunSummary;
}
```

`step()` advances exactly one configured physics tick. It never reads elapsed wall time. `StepOutput` exposes typed events without transferring the whole world. `Snapshot` is an intentionally lossy observer model, not a serialization of internal state.

The live viewer owns a `Simulation`, accumulates presentation time, calls `step()` zero or more whole ticks, and interpolates between two snapshots for smooth rendering. Pause, single-step, and speed controls therefore cannot change simulation results.

The replay viewer consumes recorded observations and uses the same render-side snapshot representation. A replay does not rerun agent decisions.

### Numeric and data choices

- Use `glam::DVec2` and `f64` in the kernel; cast into Bevy's `f32` transforms only at the viewer boundary.[^glam]
- Wrap physical quantities at API and schema boundaries (`Meters`, `Seconds`, `MetersPerSecond`, `Radians`). Serialized fields also carry unit suffixes such as `speed_mps`.
- Assign authored objects stable string IDs. Compile them once into dense integer IDs for hot loops and export the mapping in run provenance.
- Store hot agent state in dense, stable-order arrays. Do not use hash-map iteration or Bevy ECS scheduling in state-affecting kernel logic.
- Represent cars as oriented boxes and pedestrians as circles in Phase 1. Use `parry2d-f64` query primitives for exact distance/intersection and shape casting, but do not use a rigid-body dynamics engine.[^parry-cast]
- Contacts are detected and recorded; controllers stop or mark agents according to an explicit post-contact policy. Phase 1 does not pretend to reconstruct crash dynamics.

## Scenario and run contracts

### Authoring format

Use JSON5 as the source format: it remains structurally close to open JSON while allowing comments and trailing commas for hand-authored geometry. Parse into strongly typed Serde structures, generate a checked-in JSON Schema with Schemars, then perform semantic validation that JSON Schema cannot express.[^json5][^schemars]

Every scenario begins with:

```json5
{
  schema_version: 1,
  id: 'four_leg_signal_v1',
  coordinate_system: { x: 'east_m', y: 'north_m' },
  // boundaries, paths, movements, controls, portals, demand, population...
}
```

Validation is a separate command and is also mandatory before every run. Diagnostics include a stable code, source object ID, and actionable message. At minimum validate:

- unique and resolvable IDs;
- finite coordinates and positive dimensions/durations;
- path continuity, movement endpoints, and portal reachability;
- legal mode assignments and complete routes;
- signal states, transitions, and conflicting greens;
- demand distributions and population weights;
- spawn clearance and initial overlap;
- fidelity settings compatible with configured speeds and collision policy.

Scenario parsing produces a `ScenarioSource`; validation and compilation produce an immutable `CompiledScenario`. Derived data such as sampled guide paths, conflict regions, and spatial-index seeds are cached outputs identified by a content hash, never edited as source.

### Reproducibility contract

A run manifest records:

- scenario content hash and schema version;
- Tangle source revision and model version;
- dependency lockfile hash and build profile;
- target triple and determinism tier;
- fidelity settings, warm-up, duration, and termination condition;
- root seed and random-stream policy version;
- parameter overrides and output sampling policy.

Phase 1 guarantees identical canonical event streams for the same manifest on the same target triple, compiler version, build profile, and determinism tier. Cross-platform bitwise identity is a research question, not a Phase 1 promise. Cross-platform runs must still remain within declared numerical tolerances.

Use a portable ChaCha RNG and derive independent named streams from the root seed and stable agent/run IDs: `demand`, `profile`, `compliance`, and `perception`. Never share one mutable RNG across concerns. The ChaCha implementations explicitly support deterministic, portable generation.[^rand-chacha]

State-affecting operations use stable ordering, explicit tie-breakers, and a single-threaded tick in Phase 1. Parallelism occurs across complete replications. If a hot kernel later becomes parallel, its merge/reduction order must be defined and tested.

### Initial fidelity presets

These are provisional engineering defaults, not calibrated scientific claims:

| Setting | Fast | Standard | Fine |
|---|---:|---:|---:|
| Physics step | 100 ms | 50 ms | 20 ms |
| Vehicle decision cadence | 200 ms | 200 ms | 100 ms |
| Pedestrian decision cadence | 200 ms | 100 ms | 50 ms |
| Observation cadence | 500 ms | 100 ms | 50 ms |
| Broad-phase cell size | 12 m | 8 m | 4 m |
| Collision target distance | 5 cm | 2 cm | 1 cm |

Collision shape casting spans each physics step at all presets. Fidelity values live in the manifest, never only in code defaults. The final demonstration repeats selected runs at Standard and Fine and reports sensitivity.

## Graphics-first vertical slice

The first increment should end with a running viewer, even though its behavior is deliberately simple.

### Viewer content

- Orthographic top-down camera with pan and zoom.
- Rendered traversable polygons, boundaries, paths, connectors, portals, stop lines, crossings, conflict regions, and signal heads.
- Instanced/shared meshes for simple car rectangles and pedestrian discs; unique meshes or materials per agent are forbidden.
- Agent color by mode and state, plus heading and velocity vectors.
- Optional overlays for IDs, routes, envelopes, spatial cells, nearby-agent candidates, desired acceleration, and recent trajectories.
- Collision and near-miss markers that persist long enough to inspect.
- Status strip with scenario, simulation time, tick, seed, speed, agent counts, and event counts.
- Controls: pause/resume, single tick, 1x/4x/maximum speed, restart with same seed, restart with next seed, and overlay toggles.
- Click-to-inspect an agent's physical state, route, profile, current intent, and most recent decision reason.

Start with Bevy's ordinary batching and shared materials. It already batches compatible meshes; custom shaders, compute buffers, and custom instancing are optimization work triggered only by a measured render bottleneck.[^bevy-instancing]

### Walking-skeleton behavior

The initial demo scenario contains one general path, one portal at each end, and several box-shaped cars following sampled path distance at constant speed. It is acceptable for this first viewer increment to omit interaction. The key test is architectural: the same `Simulation::step()` is used by the viewer and a headless trace test.

## Delivery increments

The estimates below are sequencing guidance for one experienced developer, not calendar commitments. Review scope after each gate. A small team can overlap viewer polish, scenario examples, and analysis once the kernel contracts stabilize.

### Increment 0 — Workspace and visible walking skeleton (about 1 week)

Deliver:

- Rust workspace, pinned toolchain/dependencies, formatting, linting, unit-test, and release-build CI.
- Initial `tangle-model`, `tangle-sim`, CLI, and Bevy viewer crates.
- One parsed JSON5 scenario with a path and portals.
- Authoritative fixed-step clock, deterministic constant-speed agents, snapshots, and typed spawn/despawn events.
- The viewer content and controls needed to watch, pause, single-step, restart, and inspect the simple run.
- A headless command that produces a canonical trace from the same scenario.

Gate:

- Running the same seed twice produces the same trace hash.
- Rendering at different frame rates or using pause/speed controls does not change that hash.
- No Bevy type appears in the public API or dependency tree of `tangle-model` or `tangle-sim`.

### Increment 1 — General scenario foundation (about 1–2 weeks)

Deliver:

- Versioned source schema for boundaries, regions, guide paths, movements, portals, crossings, conflict regions, rules, and fixed-time signal phases.
- Schema generation, structural validation, semantic validation, and source-linked diagnostics.
- Compilation to dense IDs and derived geometry.
- Viewer rendering for every geometry/control primitive.
- Three benchmark scenarios: straight approach, perpendicular conflicting paths, and a small four-leg signalized intersection.

Gate:

- The benchmark layouts require no `IntersectionType` enum or scenario-specific branch in the simulator.
- Invalid references, unreachable routes, overlapping spawn areas, and conflicting signal states fail before time zero with stable diagnostics.
- Loading, compiling, and rendering the same scenario is stable under property/fuzz inputs that stay within declared limits.

### Increment 2 — Vehicle flow and controls (about 2 weeks)

Deliver:

- Portal demand generation, route assignment, physical/behavior profile sampling, and safe spawn admission.
- Path-distance tracking and a documented IDM-based longitudinal controller.
- Stop-line, signal, leader, following, queue, and exit behavior.
- Contextual red-light decision based on signal state, distance, speed, urgency, and compliance profile.
- Decision-reason records visible in the inspector.

Gate:

- Cars obey acceleration/braking/speed bounds and do not overlap in the controlled car-following benchmark.
- Saturated demand produces stable queues rather than unbounded spawn overlap.
- Profile distributions and red-light decisions are reproducible and use only their named random streams.
- Unit and scenario tests cover green/yellow/red boundaries and deterministic tie-breaking.

### Increment 3 — Pedestrians and mixed interaction (about 2 weeks)

Deliver:

- Pedestrian bodies, demand, routes through waiting areas/crossings, and a waypoint controller with local collision avoidance.
- Pedestrian signal compliance and a contextual decision to cross against the signal.
- Vehicle yielding/stopping response to occupied crossings.
- Explicit controller interfaces and model cards for the initial vehicle and pedestrian models.

Gate:

- Cars and pedestrians share the same continuous world, index, rule representation, and event system.
- Pedestrian noncompliance is a choice over a physically possible movement, not a special trajectory or teleport.
- An automated mixed benchmark completes without NaNs, unresolved overlaps, or route deadlock under nominal demand.

### Increment 4 — Geometry queries and safety events (about 1–2 weeks)

Deliver:

- Deterministic uniform-grid broad phase with stable candidate ordering.
- Exact box/box, circle/circle, and box/circle distance and intersection queries.
- Swept candidate bounds and time-of-impact shape casts for tunneling protection.
- Typed collision, near-miss, violation, entry/exit, queue, and control-transition events.
- Online TTC and minimum-separation tracking; conflict-region occupancy intervals for PET.
- Viewer overlays and inspector links from an event to its participants.

Gate:

- Property tests compare indexed candidates with an all-pairs reference on randomized small worlds.
- Swept fixtures detect crossing bodies that do not overlap at either tick endpoint.
- Event pairs have deterministic ordering and are emitted once according to a documented lifecycle.
- Fine-step differential tests agree with analytic/simple fixtures within declared tolerances.

### Increment 5 — Experiments, outputs, and convergence (about 1–2 weeks)

Deliver:

- CLI commands for `validate`, `run`, `batch`, and `replay`.
- Immutable run directory containing manifest, summary, event stream, and selectively sampled trajectories.
- JSON for manifests/summaries, compressed JSON Lines for sparse typed events, and Parquet for sampled trajectories.
- Aggregation across seeds with distributions and confidence intervals, disaggregated by mode and movement.
- Common-random-number seed banks for A/B comparisons.
- Fast/Standard/Fine convergence runner and a machine-readable sensitivity report.
- Release-mode end-to-end benchmarks and profiler captures before optimization.

Gate:

- A batch can be stopped and resumed without changing completed run artifacts.
- Parallel and serial batch execution produce identical per-run trace hashes.
- Every reported metric links back to manifest(s) and a versioned metric definition.
- Output size is bounded by the declared sampling policy; full trajectories are opt-in.

### Increment 6 — First useful release demonstration (about 1 week)

Deliver:

- Two freely described variants of the same small intersection, differing through scenario data rather than code.
- A checked-in experiment spec, seed bank, results, and concise comparison report.
- Golden traces, convergence evidence, known-limitations document, and one-command reproduction.
- A recorded replay and live-view instructions for visual review.

Gate:

- The comparison reports throughput, delay, queues, violations, collision/contact events, TTC, PET, and minimum separation by mode and movement.
- Selected findings remain directionally stable at Fine fidelity; material sensitivity is reported rather than hidden.
- The same manifest reproduces the same canonical event stream in the supported determinism environment.
- A developer can add a geometrically different benchmark scenario without changing simulator logic.

## Test strategy

Build test evidence with each feature, not after feature completion.

### Unit and property tests

- Unit wrappers, time/cadence arithmetic, path sampling, signal state machines, seed derivation, profile sampling, controller bounds, and every metric formula.
- Geometry symmetry, transform invariance, separation monotonicity, and broad-phase completeness.
- Parsers never panic on arbitrary input; valid generated scenarios round-trip through source structures.

### Reference and differential tests

- All-pairs collision/distance reference versus spatial index.
- Analytic constant-velocity cases versus shape casting, TTC, minimum separation, and PET.
- Coarse versus fine steps on a versioned fixture bank.
- Any future optimized or parallel implementation versus the Phase 1 reference implementation.

### Deterministic scenario tests

- Canonical events serialize with stable field order and hash to a golden value.
- Adding a draw to one named RNG stream cannot change outcomes owned by another stream.
- Viewer attached/detached, frame-rate changes, observation cadence changes, and batch order cannot alter state-affecting events.

### Statistical tests

- Sampled profiles recover configured marginal/correlated distributions within tolerance.
- Demand generators recover target rates over sufficiently long seeded ensembles.
- Tests use fixed seed banks and statistical tolerances; they never rely on a fresh random seed.

### Performance tests

Track rather than guess the long-term bottleneck. Record on named reference hardware:

- agent-steps per second by controller and interaction density;
- spatial candidates and exact queries per agent-step;
- simulated seconds per wall second for each benchmark scenario;
- viewer frame time by visible-agent count and overlay set;
- peak bytes per live agent and bytes per simulated hour of output.

Set absolute release budgets after Increment 2 supplies a representative baseline. Until then, CI flags regressions above an agreed threshold but does not encourage premature low-level optimization.

## Scale strategy

Phase 1 should optimize for the two real scale axes separately:

1. **One dense run:** dense arrays, stable IDs, one compiled scenario, uniform-grid broad phase, allocation-free hot ticks, selective observations, and release benchmarks.
2. **Many replications:** independent `Simulation` values run in a bounded worker pool, with no shared mutable model state and one output directory per manifest.

The second axis is likely to provide the largest early scientific throughput. It is embarrassingly parallel and preserves per-run determinism.

Do not move simulation state to GPU buffers in Phase 1. GPU compute introduces divergent agent logic, buffer synchronization/readback, more difficult inspection, hardware-dependent numerical behavior, and a second implementation to validate. Revisit it only when all of the following are true:

- profiling identifies a numerically regular kernel that dominates runtime;
- batch-level CPU parallelism is already saturated or insufficient;
- a CPU reference implementation and representative differential fixtures exist;
- the operation can tolerate or explicitly bound device-dependent numeric differences;
- end-to-end speedup includes transfer and observation costs.

Likely candidates are broad-phase candidate generation, batched distance/TTC queries, or large population sampling—not the whole perception/decision loop.

## Risks and controls

| Risk | Early warning | Control |
|---|---|---|
| Viewer logic leaks into the kernel | Headless and viewed traces differ | One-way crate dependencies and trace-hash gate in Increment 0 |
| Animation looks plausible before models are credible | Behavior changes have no model card or fixture | Decision reasons, model cards, analytic fixtures, and explicit limitations |
| General scenario schema becomes unusable | Examples need repeated verbose boilerplate | Keep primitives authoritative; add small source-level helpers/templates only after three examples expose repetition |
| Exact determinism blocks safe performance work | Hot loop cannot be parallelized without reorder | Stable reductions, per-run parallelism first, determinism tiers documented in manifest |
| Output dominates runtime/storage | Full trajectories emitted by default | Selective cadence, event-first records, compressed/columnar outputs, explicit size benchmarks |
| Collision library becomes simulation physics | Controller behavior depends on solver quirks | Use Parry queries only; own the motion and contact policy |
| Metrics are mathematically defined but misleading | TTC emitted for receding/non-intersecting paths | Analytic cases, applicability flags, quality/status fields, and versioned metric definitions |
| Bevy upgrades churn the project | Viewer migrations block kernel work | Pin versions and isolate Bevy in one app crate |

## Decisions to confirm before implementation

The recommendation above is actionable without further design work. Three product-level choices should be confirmed at kickoff because they affect ergonomics or release packaging, not the kernel architecture:

1. **Primary supported development OS:** recommend macOS initially because it matches the current environment, while keeping CI build/test coverage for Linux before the first release.
2. **Scenario source syntax:** recommend JSON5 plus generated JSON Schema. Plain JSON is more universally tooled; RON is more Rust-native but couples the format to the implementation ecosystem.
3. **Phase 1 comparison case:** recommend two fixed-time signal/control variants of one four-leg car/pedestrian intersection. A roundabout comparison would force yielding and lateral-path complexity too early.

If no alternative is selected, proceed with the recommendations.

## Definition of done

Phase 1 is complete only when all of the following are true:

- A live GPU-backed debug viewer is useful for inspecting geometry, state, decisions, collisions, and conflicts.
- The exact same kernel runs headless, faster than wall-clock, and in parallel replications.
- Cars and pedestrians move in continuous coordinates through scenarios composed from general primitives.
- The supported determinism contract is exercised by golden traces and viewer/headless equivalence tests.
- Collision/contact and surrogate metrics have analytic/reference fixtures and fidelity sensitivity results.
- Two scenario-only design variants can be reproduced and compared from checked-in manifests and seed banks.
- Known model limitations and unvalidated claims are explicit.

Passing this gate establishes the platform for Phase 2; it does not establish field calibration or absolute crash prediction.

## Upstream references for the technology decisions

[^bevy-fixed]: Bevy 0.19.1, [fixed update schedule](https://docs.rs/bevy/0.19.1/bevy/app/struct.FixedUpdate.html).
[^bevy-headless]: Bevy 0.19.1, [`MinimalPlugins` and its schedule runner](https://docs.rs/bevy/0.19.1/bevy/struct.MinimalPlugins.html).
[^bevy-instancing]: Bevy, [custom shader instancing example](https://bevy.org/examples/shaders/custom-shader-instancing/), which notes that compatible meshes are automatically batched and instanced.
[^glam]: glam, [`f64` vectors and matrices](https://docs.rs/glam/latest/glam/f64/index.html).
[^parry-cast]: Parry 2D `f64`, [shape casting](https://docs.rs/parry2d-f64/latest/parry2d_f64/query/fn.cast_shapes.html).
[^json5]: serde_json5, [Serde-compatible JSON5 parsing](https://docs.rs/serde_json5/latest/serde_json5/).
[^schemars]: Schemars, [JSON Schema generation](https://docs.rs/schemars/latest/schemars/).
[^rand-chacha]: rand_chacha, [deterministic portable generators](https://docs.rs/rand_chacha/latest/rand_chacha/).
