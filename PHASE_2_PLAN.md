# Hekate Phase 2 Implementation Plan

**Status:** Proposed  
**Date:** September 12, 2026  
**Scope:** Phase 2, “Mixed traffic,” from [VISION.md](VISION.md)  
**Prerequisite:** The [Phase 1 definition of done](PHASE_1_PLAN.md#definition-of-done) is satisfied.

## Outcome

Phase 2 will turn Hekate's car-and-pedestrian intersection model into a mixed-traffic laboratory. A scenario author will be able to combine passenger cars, pedestrians, bicycles, scooters, buses, rigid trucks, and articulated trucks in the same continuous world; assign each mode to ordinary, shared, or prohibited facilities; and observe overtaking, close passing, wrong-way travel, transit dwell, articulated off-tracking, and pedestrian group behavior.

The release will support comparative claims about how a design distributes delay, throughput, exposure, and conflicts across modes. Each new mode will have independent physical and operational validation evidence before it is admitted to the final mixed-mode comparison. Phase 2 will not claim field calibration or absolute crash prediction.

## Recommendation

Extend the existing `hekate-model`, `hekate-sim`, and `hekate-present` boundaries rather than introducing a crate or class hierarchy per mode. Compile authored mode templates into composable agent data: body geometry, kinematic limits, facility permissions, route-following behavior, tactical maneuver capabilities, occupancy, and optional transit or articulation state. A named mode remains useful for authoring, reporting, and selecting a controller family, but shared kernel behavior should operate on capabilities and physical state.

Use continuous path-relative coordinates for tactical road motion while keeping continuous world coordinates authoritative. A wheeled agent may track longitudinal progress and lateral offset from a reference path, but its pose and physical envelope are computed in world space on every physics step. This provides a tractable implementation of lane positioning, overtaking, and close passing without turning paths into discrete cells or making every road user follow a fixed centerline.

Keep simulation compute on the CPU and preserve Phase 1's deterministic single-run contract. Mixed traffic increases branching, geometric queries, and model uncertainty; it does not yet justify a second GPU implementation. Scale first through compact component arrays, broad-phase proxies for compound bodies, selective observations, and parallel independent replications.

## Entry gate and compatibility

Before Phase 2 changes behavior, capture a versioned Phase 1 baseline:

- run the Phase 1 release demonstration and retain its manifests, trace hashes, summaries, convergence report, and performance measurements;
- confirm that every Phase 1 scenario either remains valid or migrates mechanically to the Phase 2 schema;
- freeze the event and metric versions used by the Phase 1 comparison; and
- identify model changes that intentionally invalidate a trace instead of accepting unexplained golden-file churn.

Phase 2 may fix a Phase 1 defect discovered by a new mixed-mode fixture, but the fix must include a minimal Phase 1 regression case and an explicit event, metric, or model-version change when outputs change. Existing passenger-car and pedestrian behavior remains available as the comparison baseline throughout the phase.

## Phase 2 product slice

### Included

- Bicycles, standing electric scooters, buses, rigid trucks, and one tractor-semitrailer configuration, alongside Phase 1 cars and pedestrians.
- Authored mode templates that compile to bodies, dynamics, facility access, controller parameters, occupancy, and behavior-profile distributions.
- Traversable facilities with continuous width, nominal direction, allowed modes, applicable rules, and shared-space semantics.
- Continuous lateral position and steering for wheeled agents, including lane positioning, deliberate lateral clearance, lane changes where configured, overtaking, and abortable merge-back behavior.
- Contextual wrong-way travel for bicycles and scooters as a prohibited but physically possible route choice.
- Rigid and articulated swept envelopes, segment-level contacts, turning constraints, and off-tracking.
- Bus stops, waiting areas, capacity, aggregate boarding and alighting, dwell time, denied boarding, and person-throughput metrics.
- Pedestrian groups with shared destination and crossing intent, bounded cohesion, splitting, and rejoining.
- Mixed-mode interaction classification, close-pass events, mode-pair safety metrics, and viewer support for every new state.
- Independent, pairwise, and full mixed-mode validation fixtures, deterministic traces, fidelity checks, and release benchmarks.

### Deferred

- Individual passenger trip chains, transfers, wheelchair boarding processes, and detailed vehicle interiors.
- Motorcycle dynamics, rider balance, bicycle lean, falls, dooring, parking maneuvers, and detailed curb activity.
- Autonomous-vehicle stacks, vehicle-to-everything communication, and powertrain or suspension physics.
- Detailed visibility, occlusion, gaze, and perception-error models beyond the Phase 1 perception boundary.
- Field-data import, automatic calibration, parameter estimation, and claims of local behavioral validity.
- Injury biomechanics, absolute crash-rate prediction, rare-event acceleration, and statistical safety models.
- Multi-block routing, linked-signal networks, design optimization, GPU simulation, and distributed execution.

## Architecture

The Phase 1 dependency direction remains unchanged. Phase 2 adds composition inside the model and kernel rather than creating mode-specific applications:

```text
scenario v1/v2
      |
      v
+------------------+   compile    +---------------------------+
| authored modes,  |------------->| bodies, dynamics, access, |
| facilities,      |              | routes, profiles          |
| rules and demand |              +-------------+-------------+
+------------------+                            |
                                                v
                                  +---------------------------+
                                  | perceive -> choose tactic |
                                  | -> control -> move        |
                                  +-------------+-------------+
                                                |
                                      world-space envelopes
                                                |
                         +----------------------+-------------------+
                         v                      v                   v
                  safety/events          snapshots/replay     metrics/batch
```

`hekate-model` owns source versions, validation, stable authored IDs, and compilation of templates into immutable data. `hekate-sim` owns agent composition, tactical state, controllers, motion, collision queries, events, and online metrics. `hekate-present` projects all modes, body segments, maneuvers, facilities, and events into backend-independent scene data. The CLI, Bevy viewer, and terminal viewer consume those existing boundaries.

Do not create separate crates for bicycles, scooters, transit, or trucks during Phase 2. New controller or geometry modules may become crates later only after an independent consumer or a measured compile-time boundary appears.

### Agent composition

Every simulated road user has a stable agent ID and a compact core containing route, pose, velocity, intent, behavior profile, and lifecycle state. Optional or variant data supplies the capabilities that differ:

- **Body:** circle, oriented box, capsule-like two-dimensional envelope, or an ordered articulated chain of convex segments.
- **Motion:** holonomic walking, single-body wheeled steering, or articulated wheeled steering.
- **Tactical capabilities:** follow, stop, yield, choose lateral position, change lane, overtake, pass, reverse nominal direction, or serve a stop.
- **Access:** allowed facility kinds, nominal direction, speed policy, and rule set.
- **Occupancy:** operator-only, fixed occupants, or transit capacity with aggregate boarding state.
- **Social state:** optional pedestrian group membership and role.

Mode templates provide validated bundles of these components. The compiler rejects impossible combinations, such as transit dwell without capacity or articulation parameters on a holonomic body. Runtime code may dispatch to a small number of motion and controller families; it must not duplicate interaction, event, or metric logic for every named mode.

Store hot state in stable-order arrays grouped by motion/body family where profiling shows a benefit. Stable agent IDs and explicit merge order preserve deterministic event ordering when family-specific calculations are combined.

### Controller boundary

Split an agent update into four explicit stages:

1. **Relevant-world query:** collect nearby bodies, upcoming controls, facility boundaries, route options, and rules using the Phase 1 observer boundary.
2. **Tactical choice:** select a maneuver such as follow, yield, change lateral position, overtake, merge, travel against nominal direction, or dwell.
3. **Motion control:** convert the tactic into bounded longitudinal acceleration and, for wheeled agents, steering or lateral motion.
4. **Physical advance:** integrate the pose, update every body segment, perform swept queries, and emit contacts or invalid-state diagnostics.

Each tactic records a reason, target, start time, commitment state, and abort condition. Decisions occur only at the configured cadence; motion and collision checks still occur every physics step. Controller families consume immutable observations and return commands, which keeps them replaceable and prevents one mode from mutating another agent directly.

## Scenario and schema evolution

### Schema version 2

Introduce schema version 2 for mixed traffic. Version 2 adds:

- facility regions and reference paths with width, nominal direction, mode access, lateral-use policy, and speed policy;
- movement connectors that state which facilities and traversal directions they join;
- mode templates with body, dynamics, controller, occupancy, and profile distributions;
- demand by portal, mode template, destination or route distribution, and time interval;
- bus stops, waiting areas, service rules, capacity, and boarding/alighting distributions;
- pedestrian group distributions; and
- explicit permissions and obligations for nominal direction, lane use, overtaking, crossings, and stop service.

Keep authored objects identified by stable strings and compile them to dense IDs. Validate geometric containment, usable width, curvature against turning limits, connector continuity, directional reachability, mode-to-facility access, legal and physically possible routes, stop placement, transit capacity, articulation dimensions, and spawn clearance for the largest eligible body.

Provide a deterministic `migrate` command from schema version 1 to version 2. A migrated Phase 1 scenario explicitly receives a passenger-car template, its existing path becomes a directional facility, and its population becomes demand. The run manifest records the source schema version, source content hash, normalized version-2 hash, and migration version. The simulator runs normalized compiled data only; it never changes behavior through hidden parser defaults.

### Facilities, paths, and free space

A facility is a continuous traversable region plus optional reference paths. Reference paths support routing and local coordinates but do not define the only physical positions an agent can occupy. The compiled form provides:

- arc length `s`, signed lateral offset `d`, tangent, normal, and curvature along each reference path;
- usable lateral intervals after subtracting the current body's envelope and configured clearance;
- adjacency and connector relationships for route planning and tactical lane changes; and
- physically possible traversal directions separately from nominal or permitted directions.

Phase 2 does not require a general navigation mesh. Pedestrians continue to use Phase 1 walking geometry, while constrained wheeled agents use facility regions and references. Add a navigation-mesh prototype only if a release fixture cannot be represented without embedding scenario-specific waypoints.

## Motion and interaction models

### Continuous lateral motion

For ordinary road and bikeway travel, track a wheeled agent's route progress and lateral offset relative to a reference path. Integrate a kinematic steering model subject to speed, acceleration, braking, steering-angle, steering-rate, curvature, and lateral-acceleration limits. Reconstruct the world pose from the integrated motion, then project it back to route coordinates for tactical queries and drift checks. World pose remains the collision and output truth.

Lateral tactics specify a target clearance or offset and a feasible time horizon rather than teleporting between lane centers. A maneuver fails safely when its requested corridor becomes infeasible. At minimum, model these states:

```text
following -> preparing -> committed -> returning -> following
                  |           |
                  +-> aborted <-+
```

Gap acceptance checks predicted front, rear, side, and swept-envelope clearance over the maneuver horizon. Once committed, the controller still may brake or abort according to a documented safety policy. Deterministic tie-breakers decide simultaneous claims on the same gap.

### Overtaking and close passing

Overtaking is a tactical maneuver available only where geometry, permissions, profile, and visible gaps allow it. It covers same-lane bicycle/scooter passing, motor vehicles passing vulnerable users with lateral displacement, and configured lane changes around a slower leader.

The event model distinguishes an attempted overtake, commitment, abort, completion, and rule violation. A close-pass observation records the participants, facility, side, minimum body-to-body clearance, relative speed, duration below configured clearance bands, and whether the pass crossed a boundary or entered an opposing facility. Clearance bands are metric definitions, not universal declarations of safety, and remain configurable by jurisdiction or study.

### Wrong-way movement

Nominal direction and physical traversability are separate scenario properties. A bicycle or scooter may select an opposing traversal when a contextual compliance decision permits it and a physically connected route exists. The agent then uses the ordinary routing, steering, collision, yielding, and event systems. Wrong-way travel never spawns a scripted trajectory or disables collisions.

Context may include route savings, expected delay, facility type, observed density, urgency, and the stable compliance profile. Phase 2 uses the existing observer boundary and does not add detailed visibility error solely for this behavior. The simulator emits the perceived rule, decision reason, violation interval, and affected movement IDs.

### Bicycles and scooters

Bicycles and standing scooters share the wheeled steering and lateral-maneuver machinery but receive separate defaults and model cards. Their templates define distinct body dimensions, desired-speed distributions, acceleration and braking limits, steering response, lateral-clearance preferences, facility access, and compliance distributions.

The initial controllers support following, stopping, yielding, lane or bikeway positioning, passing slower narrow users, being passed, and contextual wrong-way travel. They do not model balance, lean, falls, pedaling biomechanics, dismounting, or sidewalk riding unless the scenario explicitly represents the latter as an allowed or violated facility choice.

### Buses and rigid trucks

Buses and rigid trucks use oriented rectangular bodies with mode-specific dimensions, wheelbase, turning limits, acceleration/braking envelopes, and desired-speed profiles. Their controllers reuse ordinary following, signal, priority, yielding, and lateral logic while accounting for longer bodies, slower response, wider turns, and body-specific spawn and gap requirements.

The compiler checks every assigned route against a conservative swept-turn feasibility test. Runtime controllers may reduce speed or stop when the planned envelope is infeasible; they may not silently clip a body through a curb or another agent.

### Articulated trucks

Support one tractor-semitrailer family as an ordered kinematic chain. Authored geometry specifies segment lengths and widths, wheelbase or axle locations, hitch offsets, maximum articulation angle, and steering limits. The tractor pose drives deterministic hitch and trailer integration; every segment contributes to the exact and swept body envelope.

The broad phase may index one conservative proxy for the whole chain or one proxy per segment, but the narrow phase reports the actual segment pair. Detect jackknife-limit violations, infeasible connector curvature, curb encroachment, and off-tracking as typed diagnostics or events. Contact recording remains geometric; Phase 2 does not add impact or vehicle-damage dynamics.

### Transit dwell and boarding

A bus stop combines a stopping position, a waiting area, a service policy, and passenger demand. Phase 2 represents waiting passengers as deterministic aggregate cohorts keyed by arrival time and destination class. It does not create a walking agent for every boarding passenger.

At a stop, a bus aligns within a configured berth tolerance, opens service, alights and boards subject to capacity, and dwells according to explicit parameters such as minimum dwell, door count, per-passenger service time, and an optional seeded disturbance. The run records arrivals, waiting time, boarding, alighting, denied boarding, occupancy, dwell, and departure. Boarding randomness uses its own named stream and cannot perturb vehicle behavior or demand streams.

Buses re-enter traffic through the same gap-selection and priority rules used by other lateral maneuvers. Whether traffic must yield to a departing bus is scenario data, not a built-in jurisdictional assumption.

### Pedestrian groups

A group is a relationship among ordinary pedestrian agents, not one large collision body. Members retain individual physical envelopes and contacts while sharing a destination, desired crossing decision, and bounded cohesion preference. A group may stretch, split when constrained, and rejoin after a crossing; it must not deadlock solely because a configured group cannot fit abreast.

Group fixtures cover paired adults, an adult with a slower companion, and a larger group. The model records group delay, split duration, crossing dispersion, and whether members are stranded across a control transition. Social influence on compliance is explicit and uses a named group-decision stream.

## Safety, operations, and outputs

Extend Phase 1 events and metrics without changing their meaning. Every metric carries a definition version and applicability status. New outputs include:

- throughput, delay, stops, queueing, route completion, and exposure by mode, movement, and facility;
- person-throughput and person-delay, with vehicle occupancy and bus passengers reported separately from vehicle counts;
- bus waiting time, dwell, occupancy, denied boarding, schedule or headway deviation when configured, and merge delay;
- overtake attempts, aborts, completions, lateral-displacement violations, and close-pass clearance distributions;
- wrong-way distance, duration, exposure, encountered agents, and conflicts;
- pedestrian group split time, dispersion, and crossing completion;
- articulated off-tracking, curb or boundary encroachment, articulation-limit events, and segment-level contacts; and
- collisions, TTC, PET, minimum separation, and conflict severity grouped by participant-mode pair and interaction geometry.

Classify interactions as following, crossing, merging, overtaking, head-on/opposing, shared-space, or stop-service related when the available trajectories support the label. Preserve an `unknown` classification rather than forcing ambiguous interactions into a category.

TTC is reported only when its motion assumptions apply. Curved, articulated, and lateral interactions also use finite-horizon predicted minimum separation and actual sampled/swept minimum separation. Phase 2 does not present a single surrogate as a calibrated probability of collision.

Trajectory records add mode template, body-segment poses when requested, lateral route coordinates, maneuver state, occupancy, group ID, and rule state. High-volume fields remain subject to the manifest's sampling policy; sparse state transitions and safety events are always event records.

## Reproducibility and fidelity

Retain Phase 1's supported-platform determinism contract. Derive random draws from the root seed, run ID, stable agent or group ID, and a versioned stream name. Add streams for `maneuver`, `transit_service`, and `group_decision`; do not share them with demand, profile, compliance, or perception. Adding a bus passenger or pedestrian group must not change unrelated car behavior through draw-order coupling.

Stable ordering covers facility queries, candidate bodies, simultaneous gap claims, contact pairs, group decisions, segment pairs, and emitted events. Parallel batch order remains irrelevant to per-run hashes.

Extend fidelity settings with lateral-decision cadence, steering integration tolerance, articulated-segment sweep tolerance, and maneuver-prediction horizon. The Fast, Standard, and Fine presets resolve every value explicitly in the manifest. Convergence reports compare mode-specific distributions and rare-event counts, not only aggregate throughput.

## Delivery increments

The estimates are sequencing guidance for one experienced developer. Each increment ends in a runnable scenario and evidence gate. Review model scope after each gate; do not begin the final mixed-mode experiment while an individual-mode gate is failing.

### Increment 0 — Baseline and extension contract (about 1 week)

Deliver:

- Captured Phase 1 trace, metric, convergence, and performance baseline.
- Schema-version-2 design, deterministic version-1 migration, and manifest provenance for source and normalized hashes.
- Compiled agent components, mode-template validation, controller-stage interfaces, and a model-card template.
- Benchmark matrix and quantitative tolerances for independent, pairwise, and mixed-mode validation.
- Viewer and output representation for arbitrary body kinds and optional body segments, initially populated by Phase 1 modes.

Gate:

- All Phase 1 acceptance scenarios pass through either their original reader or the explicit migration path.
- Any intentional baseline change has a minimal regression fixture and a versioned explanation.
- A synthetic template can alter dimensions, limits, and access without adding a named-mode branch to shared interaction code.
- The dependency-direction check still prevents UI, filesystem, and wall-clock types from entering the kernel.

### Increment 1 — Facilities and narrow modes (about 2 weeks)

Deliver:

- Continuous-width facilities, reference-path coordinates, directional traversal, access rules, and connectors.
- Bicycle and scooter templates, demand, profiles, bodies, isolated controllers, and model cards.
- Longitudinal following, stops, signals, priorities, facility selection, and route completion before free lateral maneuvers are enabled.
- Isolated straight, curve, braking, following, signal, and crossing fixtures for both modes.

Gate:

- Path/world coordinate round trips remain within declared tolerance on straight and curved fixtures.
- Bicycle and scooter motion respects dimensions, speed, acceleration, braking, steering, and facility boundaries.
- Repeated seeded runs reproduce demand, profiles, decisions, events, and trace hashes.
- Each mode passes independent fixtures without relying on car-specific dimensions or controller defaults.

### Increment 2 — Lateral motion, passing, and wrong-way travel (about 2–3 weeks)

Deliver:

- Continuous lateral targeting, lane/facility transitions, gap prediction, maneuver commitment, abort, and return states.
- Bicycle/scooter passing, motor-vehicle overtaking of narrow users, and configured lane changes around slower leaders.
- Close-pass observations and violations with exact clearance and relative-speed evidence.
- Contextual wrong-way route selection and ordinary opposing-flow interaction.
- Viewer overlays for usable corridor, target offset, predicted gap, maneuver state, and wrong-way rule state.

Gate:

- No maneuver teleports, snaps laterally, crosses a forbidden boundary without an event, or exceeds motion limits.
- Analytic and fine-step fixtures bound the error in predicted and observed minimum clearance.
- Simultaneous gap claims resolve deterministically and unsafe commits follow the documented braking/abort policy.
- Wrong-way agents use normal routing, collision, and metric paths and cannot bypass an occupied opposing corridor.

### Increment 3 — Heavy and articulated vehicles (about 2 weeks)

Deliver:

- Bus and rigid-truck templates with heavy-vehicle dynamics and turning envelopes.
- Tractor-semitrailer geometry, hitch integration, segment broad/narrow phase, swept queries, and articulation limits.
- Route-feasibility diagnostics and off-tracking/boundary events.
- Independent straight, constant-radius, S-turn, stop, following, and conflict fixtures.

Gate:

- Segment poses match analytic or high-resolution reference trajectories within declared tolerances.
- Swept tests detect contacts by any segment even when no endpoint overlaps.
- Infeasible authored turns fail validation or produce an explicit runtime inability to proceed; bodies never clip through boundaries.
- Phase 1 box/circle collision fixtures remain unchanged except for declared versioned corrections.

### Increment 4 — Transit service (about 1–2 weeks)

Deliver:

- Stops, waiting areas, aggregate passenger arrival/alighting demand, capacity, boarding, denied boarding, dwell, and departure.
- Bus approach, berth alignment, stop service, and deterministic merge-back behavior.
- Transit events, occupancy and person metrics, inspector state, and replay support.
- Isolated low-demand, capacity-constrained, variable-dwell, blocked-berth, and merge fixtures.

Gate:

- Passenger conservation accounts for every waiting, boarded, alighted, denied, and remaining passenger.
- The configured dwell formula is reproduced by hand-computable fixtures and its random stream is isolated.
- Bus occupancy never exceeds capacity or becomes negative.
- Vehicle throughput and person-throughput remain distinct throughout summaries and comparisons.

### Increment 5 — Pedestrian groups and pairwise interaction (about 2 weeks)

Deliver:

- Group generation, shared crossing intent, cohesion, splitting, and rejoining.
- Pairwise fixtures for every material mode pair and interaction family: following, crossing, merging, passing, opposing, and shared facility.
- Decision-reason and interaction-classification records sufficient to explain yields, stops, rejected gaps, and deadlocks.
- A scripted density ramp that identifies the operating range before persistent gridlock or model breakdown.

Gate:

- Group members retain individual contacts and can split rather than forcing an infeasible group envelope.
- Nominal pairwise fixtures complete without NaNs, unresolved overlaps, or indefinite mutual yielding.
- Every mode pair has an explicit supported, impossible, or deferred disposition for each interaction family.
- Results outside the validated density range are labeled rather than silently treated as credible.

### Increment 6 — Mixed-mode safety and experiment outputs (about 1–2 weeks)

Deliver:

- Versioned interaction taxonomy, mode-pair conflict metrics, articulated segment evidence, close-pass reports, and wrong-way exposure.
- Person- and vehicle-based operational summaries, bus service summaries, and pedestrian-group summaries.
- Batch comparison and confidence intervals disaggregated by mode, movement, facility, and participant pair.
- Common-random-number handling that preserves comparable demand and profiles when a design changes feasible routes.
- Viewer inspection and replay for maneuver histories, groups, occupancy, body segments, and mixed-mode safety events.

Gate:

- Every aggregate value can be traced to versioned event/trajectory definitions and source manifests.
- Metrics state when TTC or another surrogate is inapplicable instead of emitting a misleading number.
- Serial and parallel batches produce identical per-run hashes.
- Output size and runtime remain within declared sampling and benchmark budgets.

### Increment 7 — Mixed-traffic release demonstration (about 2 weeks)

Deliver:

- Two scenario-only variants of one compact intersection containing all supported modes and one bus stop.
- A checked-in experiment specification, demand bank, seed bank, manifests, results, and comparison report.
- Independent mode evidence, pairwise matrix, full mixed-mode evidence, and Standard/Fine sensitivity results.
- Model cards, validated operating ranges, known limitations, replay recordings, and one-command reproduction.

Gate:

- Both variants complete agent, passenger, route, and occupancy conservation checks.
- The report compares throughput, person-delay, queues, bus service, group outcomes, passing clearance, wrong-way exposure, collisions, TTC, PET, and minimum separation without hiding a harmed mode in an aggregate.
- Material conclusions remain directionally stable at Fine fidelity or are explicitly reported as sensitive.
- A scenario author can add a new template assembled from existing components without changing shared physics, safety, metric, or presentation code.

## Validation strategy

### Validation ladder

Validate in this order:

1. **Physical:** dimensions, bounds, kinematics, steering, articulation, collision geometry, and conservation.
2. **Isolated operational:** free speed, acceleration, braking, following, stopping, turning, dwell, and group cohesion.
3. **Pairwise interaction:** every relevant mode pair under crossing, following, merge, pass, opposing, and shared-space conditions.
4. **Mixed scenario:** all modes under low, nominal, saturated, and intentionally adversarial demand.
5. **Fidelity:** selected fixtures and experiment findings at Standard and Fine settings.

Passing a later rung does not excuse a failure in an earlier one. Calibration against field observations belongs to Phase 3; Phase 2 evidence establishes implementation correctness, numerical stability, qualitative response, and declared operating ranges.

### Unit and property tests

- Template compilation, invalid component combinations, facility access, route direction, usable-width calculation, and schema migration.
- Path-coordinate transforms, curvature, steering integration, lateral bounds, and maneuver state transitions.
- Convex and articulated distance, intersection, swept-envelope, and segment-order invariants.
- Transit queues, dwell arithmetic, capacity, passenger conservation, and group split/rejoin state.
- Transform invariance, candidate completeness, deterministic ordering, and finite-state progress under bounded inputs.

### Reference and differential tests

- Kinematic motion against analytic straight and constant-radius trajectories.
- Articulated off-tracking against a high-resolution reference integrator.
- Broad-phase candidates against all-pairs segment queries on randomized small worlds.
- Maneuver prediction against fine-step executed clearance.
- Fast and Standard presets against Fine on the release fixture bank.

### Deterministic and statistical tests

- Golden traces for each new mode, tactic, body family, transit lifecycle, and group lifecycle.
- Stream-isolation tests that add an unrelated mode, passenger cohort, or group without changing other agents' owned draws.
- Fixed-seed distribution tests for mode profiles, maneuver choices, wrong-way decisions, passenger arrivals, dwell disturbances, and group sizes.
- Ensemble checks use declared intervals and sufficient samples; they never pass or fail from a fresh random seed.

### Performance tests

Record on named reference hardware:

- agent-steps per second by mode mix and interaction density;
- tactical candidates, broad-phase candidates, segment queries, and prediction work per agent-step;
- memory per rigid agent, articulated agent, waiting cohort, and pedestrian group;
- simulated seconds per wall second for isolated, pairwise, and release scenarios;
- batch throughput and output bytes per simulated hour; and
- viewer frame time by visible bodies, articulated segments, overlays, and backend.

Set Phase 2 budgets from the Increment 0 Phase 1 baseline and Increment 2 representative mixed-mode profile. Optimize only measured dominant work. Any optimized geometry or controller path keeps a reference implementation or differential fixture.

## Risks and controls

| Risk | Early warning | Control |
|---|---|---|
| Composition becomes an indirect class hierarchy | Every mode needs matching enums across crates | Dispatch on a few body/motion/controller families; keep mode names in templates and reporting |
| Path-relative motion becomes discrete lane snapping | Lateral changes complete in one decision tick | Integrate bounded steering/lateral motion and collide only in world space |
| Mixed interaction rules deadlock | Agents repeatedly yield with no progress | Explicit commitment, priority, timeout, and deterministic tie-break policies with adversarial fixtures |
| Overtaking appears safe because prediction is coarse | Executed clearance is much smaller than accepted clearance | Swept checks, conservative body corridors, abort policy, and prediction-versus-execution tests |
| Articulated bodies miss collisions or explode query cost | Trailer contacts lack segment evidence or candidates grow quadratically | Conservative proxies, exact segment narrow phase, query counters, and all-pairs reference tests |
| Aggregate passengers conceal impossible service | Passenger totals drift or dwell cannot be reconstructed | Cohort ledger, conservation invariant, explicit dwell formula, and per-stop events |
| Group cohesion overrules individual safety | Groups pull members through occupied space | Individual bodies and collision avoidance remain authoritative; cohesion is bounded and breakable |
| New RNG draws invalidate old comparisons | Adding a mode changes unrelated car traces | Stable IDs, keyed named streams, and stream-isolation tests |
| More modes create precise-looking but unvalidated claims | Defaults lack provenance or operating range | One model card and independent evidence gate per mode; comparative claims only |
| Schema migration silently changes Phase 1 behavior | Migrated trace changes with no versioned reason | Source and normalized hashes, explicit migration version, and retained Phase 1 baseline |

## Decisions for implementation

The following defaults make the plan executable without a separate architecture phase:

1. Use schema version 2 with an explicit version-1 migration command; do not grow version 1 through optional mixed-traffic fields.
2. Use continuous facility regions plus reference-path coordinates for constrained wheeled motion; defer a general navigation mesh until a fixture proves it necessary.
3. Use one wheeled steering family for cars, bicycles, scooters, buses, and rigid trucks, parameterized by body and limits; use a separate deterministic articulated integrator for tractor-semitrailers.
4. Represent bus passengers as aggregate cohorts with exact conservation; defer individual passenger agents and transfers.
5. Treat groups as relationships among pedestrian agents, never as one physical body.
6. Keep nominal direction, permissions, and physical possibility separate so wrong-way behavior uses ordinary motion and interaction logic.
7. Preserve CPU reference paths and per-run determinism; revisit GPU or within-run parallel simulation only after release profiling identifies a dominant kernel and differential fixtures exist.

## Definition of done

Phase 2 is complete only when all of the following are true:

- Passenger cars, pedestrians, bicycles, scooters, buses, rigid trucks, and a tractor-semitrailer can share one scenario through composable bodies, dynamics, access, and controllers.
- Wheeled agents occupy continuous lateral positions and can perform bounded, inspectable passing and lane/facility transitions without teleporting or bypassing collision checks.
- Wrong-way travel is a contextual violation over physically connected space and produces ordinary interactions plus explicit rule evidence.
- Articulated envelopes, off-tracking, and contacts are tested against reference trajectories and swept fixtures.
- Bus stop service conserves passengers and produces separate vehicle and person metrics.
- Pedestrian groups coordinate while retaining individual motion, collision, and safety state.
- Every supported mode passes physical and isolated operational gates, and every material mode pair has a documented interaction disposition.
- The release comparison reports operational and conflict distributions by mode and participant pair with reproducible manifests, seed banks, uncertainty intervals, and fidelity sensitivity.
- Phase 1 scenarios remain reproducible through their supported source or explicit migration path, apart from documented and versioned corrections.
- Model cards state assumptions, parameter sources, validated ranges, known failure modes, and incompatible fidelity settings.

Passing this gate establishes a controlled mixed-traffic simulator. Real-intersection import, field calibration, held-out validation, and parameter-identifiability work begin in Phase 3.
