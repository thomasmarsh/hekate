# Hekate: Vision for an Intersection Simulation Laboratory

**Status:** Working vision, version 0.1  
**Date:** September 11, 2026

## Purpose

Hekate will be a high-fidelity, microscopic simulator for studying how people and vehicles move through intersections. It will represent individual road users—including drivers, pedestrians, cyclists, scooter riders, buses, and trucks—as physical, perceiving, decision-making agents. It will support ordinary behavior, mistakes, and deliberate rule violations without assuming that every participant behaves identically or perfectly.

The long-term purpose is not merely to animate traffic. Hekate should become an experimental laboratory in which we can:

- reproduce the operating and safety characteristics of a real intersection;
- construct intersection geometries and control schemes that do not fit a fixed catalog;
- compare conventional and novel designs under the same demand and behavior distributions;
- understand tradeoffs among safety, delay, throughput, accessibility, and fairness across travel modes; and
- eventually search a large design space for layouts and control policies suited to particular traffic patterns.

The near-term purpose is smaller: establish a trustworthy simulation kernel and a representation that can grow toward that vision without prematurely implementing every mode or behavior.

## Product thesis

An intersection is not a label such as “four-way stop,” “signalized junction,” or “roundabout.” It is a set of traversable spaces, legal and possible movements, control devices, visibility conditions, and interactions among agents. Hekate will model those elements directly. Familiar intersection types should emerge as configurations of general primitives, and unfamiliar intersection types should be equally representable.

Likewise, safety should not be a scripted property of a design. It should emerge from geometry, controls, demand, physical dynamics, perception, and a calibrated population of heterogeneous road users. A red-light runner, a pedestrian crossing against a signal, or a scooter rider salmoning should use the same world and interaction rules as everyone else; these should be behaviors an agent can choose, not special animation cases.

## What success looks like

Hekate succeeds when a user can describe an intersection and a population, run many reproducible trials, and make statements such as:

> Under this demand distribution, design B reduces severe crossing conflicts for pedestrians without moving unacceptable delay onto buses.

That statement is more important than photorealistic rendering. Visualization matters for debugging, explanation, and discovery, but the core product is a credible experiment with inspectable assumptions and statistically meaningful outputs.

Longer term, a calibrated Hekate model of a real intersection should reproduce multiple classes of observation within explicit uncertainty bounds:

- traffic counts, turning movements, speeds, travel times, queue lengths, and throughput;
- yielding, gap acceptance, signal compliance, route choice, and other observable behaviors;
- distributions of close interactions and traffic conflicts; and
- where sufficient local data and simulation volume exist, crash types, relative frequencies, and severity distributions.

“Plausible” will mean agreement with declared calibration targets, not that an animation looks convincing.

## Guiding principles

### 1. General primitives, not an intersection catalog

The scenario format will describe geometry, connectivity, permissions, priorities, and controls. A named template may make common designs convenient, but templates will compile to the same primitives available to every user. No simulator logic should switch on an enum of intersection types.

### 2. Continuous truth, configurable computational fidelity

Hekate should store geometry, position, orientation, velocity, and physical envelopes in continuous world coordinates using SI units. A grid may accelerate neighborhood queries and broad-phase collision detection, but it should not normally force agents to occupy discrete cells or snap their paths to a lattice.

This preserves a path to fine lateral behavior, irregular geometry, close passing, articulated vehicles, and mixed traffic. It also avoids making a convenient early grid into the ontology of the world.

### 3. Fidelity is a set of explicit budgets

A single “quantization level” would hide several different sources of error. Hekate should expose named fidelity presets, but each preset should resolve to independent controls:

- **physics step:** how often motion is integrated;
- **decision cadence:** how often an agent perceives and revises its action;
- **spatial-index resolution:** the cell or hierarchy size used to find possible interactions;
- **collision tolerance:** the precision of narrow-phase overlap or swept-volume tests; and
- **observation cadence:** how frequently trajectories and metrics are recorded.

The spatial index is an optimization and may be coarse without making positions coarse. Collision candidates found in the broad phase should be checked using actual agent envelopes; fast agents should use swept tests or a sufficiently small physics step so that they cannot pass through each other between samples.

Lower fidelity should make a run cheaper in known ways, not silently change its meaning. Every run will record its fidelity settings. Key results should be subjected to a convergence check—for example, repeating a sample at half the time step and finer collision settings—to measure sensitivity. Existing microscopic simulators demonstrate both the value and cost of separating simulation step, action step, and lateral decision resolution.[^sumo-step][^sumo-sublane]

Adaptive fidelity near conflict zones may eventually improve performance, but it must preserve deterministic replay and be validated against uniformly fine runs.

### 4. Heterogeneity is fundamental

Agent diversity is not noise added after the “real” model. It is part of the model. Every agent will combine:

- a **mode and physical body**, such as pedestrian, bicycle, scooter, passenger car, bus, or articulated truck;
- **dynamic limits**, including size, acceleration, braking, turning radius, and lateral agility;
- a **route and current intent**;
- a **behavior profile**, sampled for the agent and stable over an appropriate period;
- a **transient state**, such as attention, urgency, impairment, or obstruction; and
- a **perception state**, which may lag or differ from ground truth.

Profiles should be multidimensional and, where evidence permits, correlated. Useful dimensions include desired speed, following headway, reaction time, gap acceptance, risk tolerance, patience, yielding propensity, rule compliance, lateral clearance, and attentiveness. “Aggressive” or “law-abiding” may be convenient preset names, but must not collapse behavior into a single scalar or permanent moral category.

Rare behavior should be contextual rather than an independent coin flip each tick. A driver’s probability of entering late on red might depend on signal phase, speed, distance to the stop line, following traffic, enforcement expectations, urgency, and their stable profile. A pedestrian’s crossing decision might depend on delay, visible gaps, group behavior, walking speed, and signal state. Salmoning should be expressible as an agent selecting a physically possible movement that is prohibited or contrary to nominal flow.

### 5. Rules are data; violations are choices

Traffic rules will be declarative constraints and obligations attached to spaces, movements, controls, and agent classes. They should describe what is permitted, not impose an invisible physical barrier. An agent can perceive a rule, comply with it, misunderstand it, fail to perceive it, or knowingly violate it.

Separating rules from physics enables both ordinary control and realistic noncompliance. It also lets an experiment distinguish:

- physically impossible movement;
- legally prohibited movement;
- a movement unavailable to a particular mode;
- a perceived but rejected rule; and
- an unperceived or misunderstood rule.

### 6. Reproducibility before optimization

A run will be identified by an immutable scenario version, model version, parameter set, fidelity configuration, random seed, and random-stream policy. Given those inputs, the kernel should replay deterministically on a supported platform. Results must include enough provenance to reproduce the run.

Randomness should use separate named streams for demand, profile sampling, perception error, and rare events. Adding an unrelated random draw must not reshuffle every downstream result. Optimization is meaningful only when candidate designs can be compared under controlled, repeatable populations and demand realizations.

### 7. Evidence is layered and uncertainty is visible

Hekate will not infer a universal crash rate from a national average and declare itself realistic. Observed crash data are sparse, location-dependent, and affected by reporting and exposure. National sources such as NHTSA’s CRSS and FARS are useful for population-level crash types and outcomes, while local trajectories, traffic counts, signal timing, and crash records are needed to reproduce a particular site.[^crss][^fars]

The calibration hierarchy will be:

1. **Physical validity:** motion respects dimensions and dynamic bounds; collision detection is numerically stable.
2. **Operational validity:** volumes, speeds, routes, delays, queues, and capacities match held-out field observations.
3. **Behavioral validity:** distributions such as yielding, compliance, headway, and gap acceptance match observations.
4. **Interaction validity:** the frequency, location, type, and severity of conflicts match trajectory-based evidence where available.
5. **Crash validity:** aggregate crash frequency and severity are checked against appropriate local models and records, with uncertainty intervals.

Calibration and validation must use different observations or time periods, and results should be tested across multiple random seeds. This follows established microsimulation guidance that calibration targets be chosen for the analysis purpose and checked against field performance.[^fhwa-calibration]

## Conceptual model

### The world

The physical world is a local two- or two-and-a-half-dimensional coordinate system containing:

- traversable and non-traversable polygons;
- curbs, islands, medians, barriers, and boundaries;
- directed guide paths or navigation meshes;
- entry and exit portals that generate and absorb demand;
- movement connectors and conflict regions;
- stop lines, crossings, waiting areas, and loading zones;
- visibility obstacles and, later, surface condition and grade; and
- sensors and control devices.

The navigation graph answers “where can this agent plan to go?” It does not define all physical positions the agent may occupy. This distinction allows lane-following cars and freely moving pedestrians to share one world, and allows a rule-breaking agent to leave its nominal path while remaining physically simulated.

### Movements and permissions

A movement connects regions or paths and carries metadata such as allowed modes, nominal direction, right-of-way relationship, speed policy, and applicable controls. Conflict regions may be derived geometrically from crossing movement envelopes, then reviewed or overridden by a scenario author.

Control logic will be composed from state machines, detectors, timers, phases, and priority rules. Fixed-time traffic signals, actuated signals, stop control, yield control, flashing operation, and uncontrolled movements should all be configurations of these pieces.

### Agents

An agent operates through a perception–decision–motion loop:

1. It receives an imperfect, delayed view of relevant nearby agents, signals, signs, boundaries, and route choices.
2. It selects an intent or maneuver subject to its route, profile, transient state, and perceived rules.
3. A mode-specific controller converts that intent into acceleration, steering, or walking motion within physical limits.
4. The world advances motion and resolves contacts.
5. The simulator emits observations and safety events without feeding omniscient information back into the agent.

Vehicle-following can begin with a documented microscopic model such as IDM, which represents continuous longitudinal motion through interpretable parameters.[^idm] Pedestrian motion can begin simply with waypoint following and collision avoidance, while retaining a boundary that permits later use of empirically grounded approaches such as social-force or velocity-obstacle models.[^social-force] No single behavioral model should be hardwired into the kernel.

### Components rather than a deep type hierarchy

Modes share capabilities without sharing every behavior. A bus is a large road vehicle with passenger-stop behavior; a bicycle is a narrow vehicle that may use a lane, bike facility, or shared space; a person may walk, wait, ride, transfer, and walk again. The design should favor composable capabilities—body, dynamics, perception, controller, occupancy, passenger carrier, route follower—over a rigid class tree.

This also gives Hekate a path to model a trip as one traveler occupying different vehicles or modes without pretending that a bus and its passengers are one behavioral entity.

## Safety model and outputs

### Collisions are necessary but insufficient

At fine fidelity, Hekate will detect contact between oriented physical envelopes and record the involved agents, contact geometry, relative velocity, and estimated change in velocity. The first implementation need not be a crash reconstruction or injury biomechanics model. It must be honest about that limitation.

Because serious crashes are rare, ordinary Monte Carlo runs may contain too few collisions to compare designs reliably. Hekate will also compute surrogate safety measures from trajectories, including:

- time to collision (TTC);
- post-encroachment time (PET);
- minimum separation and required deceleration;
- conflict angle and interaction type; and
- relative speed and estimated delta-v when contact occurs.

FHWA’s Surrogate Safety Assessment Model uses trajectory-derived conflict counts and indicators including TTC, PET, and delta-v, while emphasizing comparative analysis across alternatives.[^ssam] Hekate should initially make the same restrained claim: surrogate measures can rank and diagnose scenarios, but do not automatically equal predicted crash counts.

### From conflicts to expected crashes

Absolute safety prediction is a later, separately validated capability. It may combine:

- simulated exposure and conflict distributions;
- locally calibrated mappings from conflicts to crashes;
- Safety Performance Functions and Crash Modification Factors; and
- empirical-Bayes estimates using site history where available.

The Highway Safety Manual framework relates traffic volume and roadway characteristics to expected crash frequency and supports explicit comparison of design alternatives.[^hsm] Hekate should interoperate with that statistical layer rather than claiming that agent simulation alone solves rare-event crash prediction.

Rare-event acceleration or importance sampling may later make dangerous interactions computationally observable. Such runs must retain likelihood weights and must never be mixed with ordinary Monte Carlo results as raw counts.

### Core metrics

Every experiment should be able to report distributions—not only averages—of:

- completed trips and throughput by movement and mode;
- travel time, delay, stops, and queue length;
- reliability and spillback;
- rule violations and failed or aborted maneuvers;
- collisions by type and severity proxy;
- conflicts and near misses by type, participant mode, TTC, and PET;
- pedestrian and cyclist exposure;
- bus delay and person-throughput; and
- later, emissions, energy, accessibility, and curb use.

Metrics should remain disaggregated by mode and movement so that an apparently efficient design cannot hide harm transferred to a smaller or more vulnerable group.

## Scenario and experiment model

A Hekate study will distinguish four artifacts:

1. **Scenario:** geometry, navigation, controls, rules, demand distributions, environment, and population distributions.
2. **Run manifest:** exact scenario version, model versions, fidelity settings, seed, duration, and warm-up.
3. **Event/trajectory record:** a selectively sampled, machine-readable account of what happened.
4. **Analysis:** metric definitions, aggregation, uncertainty estimates, and comparisons across replications or designs.

The scenario format should be declarative, versioned, schema-validated, diffable, and independent of the visualization layer. Hand-authored examples should be possible even after map and GIS importers exist. Derived data—such as spatial indexes and inferred conflict zones—should be cached build products, not the source of truth.

Experiments should support factorial sweeps over geometry, controls, demand, behavior distributions, weather, and fidelity. Candidate designs in a search should be evaluated against the same bank of random seeds (“common random numbers”) to reduce comparison noise.

## Scope boundaries

Hekate is initially:

- microscopic and agent-based;
- centered on one intersection and its approaches;
- headless at its core, with visualization as a client;
- designed for repeated batch experiments; and
- intended for research and comparative decision support.

Hekate is not initially:

- a citywide route-assignment platform;
- a photorealistic driving game;
- a certified crash reconstruction or injury model;
- a replacement for field data or professional engineering judgment;
- a catalog of hard-coded intersection templates;
- a full vehicle powertrain, tire, suspension, or weather-physics model; or
- a claim that finer time steps alone create more accurate human behavior.

The architecture should allow multiple blocks and richer streetscape behavior later, but the first implementation should not pay every cost of city-scale simulation in advance.

## First useful release

The first release should answer one question well: **Can Hekate compare two small, freely described intersection configurations using reproducible microscopic motion and interpretable operational and conflict metrics?**

### Included

- A headless, deterministic fixed-step kernel using continuous 2D coordinates.
- A declarative scenario format with arbitrary boundaries, paths, connectors, portals, conflict regions, and signal or priority controls.
- Two participant modes: passenger cars and pedestrians.
- Simple, replaceable car-following and pedestrian waypoint/collision-avoidance controllers.
- Agent dimensions, speed and acceleration limits, routes, and a small set of behavior-profile parameters.
- Contextual red-light running and pedestrian crossing against the signal as the first noncompliant behaviors.
- Broad-phase spatial indexing plus exact envelope checks; collision and near-miss event records.
- Throughput, delay, queue, TTC, PET, minimum-separation, violation, and collision metrics.
- Seeded batch replication, run manifests, and trajectory export.
- A minimal visual replay/debug view, if needed to validate geometry and behavior.

### Deliberately deferred

- Buses, trucks, bicycles, scooters, wheelchairs, and articulated bodies.
- Full lateral lane choice, overtaking, salmoning, and shared-space negotiation.
- Imperfect visibility and detailed perception error.
- Injury estimation and absolute crash-rate prediction.
- Real-map import, automatic calibration, multi-block routing, and design optimization.
- Photorealistic graphics.

The schema and component boundaries should reserve clean extension points for deferred modes, but unused abstractions should not be implemented solely to look future-proof.

### Acceptance criteria

The first release is credible when:

- common layouts can be expressed without adding a new intersection type to code;
- the same manifest reproduces the same event stream;
- no agent can tunnel through another in the supported speed and fidelity envelope;
- conservation checks account for every generated and completed or stranded agent;
- simple hand-computable scenarios produce expected right-of-way, queue, and collision outcomes;
- halving the physics step does not materially change selected benchmark distributions beyond declared tolerances;
- a signal or geometry change produces explainable differences in operational and surrogate-safety metrics; and
- every published result carries its scenario, seed set, model versions, fidelity settings, and uncertainty interval.

## Evolution path

### Phase 1: Trustworthy kernel

Build the first useful release. Prioritize invariants, determinism, observability, and scenario ergonomics over mode count. Establish benchmark scenarios and numerical convergence tests at the same time as features.

### Phase 2: Mixed traffic

Add bicycles, scooters, buses, and trucks through composable bodies and controllers. Introduce continuous lateral motion, overtaking and close-passing behavior, wrong-way movement, dwell and boarding, articulated envelopes, and richer pedestrian group behavior. Validate each mode independently before trusting cross-mode results.

### Phase 3: Real-intersection modeling

Add import and authoring tools for maps, signal timing, counts, and trajectories. Create a calibration pipeline with held-out validation periods, parameter identifiability checks, and uncertainty reporting. Support scenario packages that preserve source provenance and local assumptions.

### Phase 4: Streetscape and network context

Expand beyond the intersection box to approaches, mid-block crossings, driveways, curb activity, parking, bus stops, and linked intersections. Add route choice and boundary conditions capable of reproducing upstream arrivals and downstream spillback without immediately becoming a whole-city simulator.

### Phase 5: Design exploration

Define constrained, constructible design parameters and multi-objective fitness functions. Begin with transparent sweeps and Bayesian or surrogate-assisted optimization before genetic algorithms. Search results must be re-evaluated at higher fidelity, across unseen demand and seed sets, and under stress scenarios so the optimizer cannot exploit numerical artifacts or a narrow calibration regime.

## Technical posture

The vision does not yet choose a programming language, rendering engine, or storage technology. Early architectural choices should preserve these properties:

- the simulation kernel is independent of UI and wall-clock time;
- behavior and motion models are replaceable behind narrow interfaces;
- world state and metrics can be inspected without rendering;
- geometry and topology have stable identifiers;
- scenario input is schema-versioned and validated before a run;
- units are explicit at boundaries and canonical internally;
- events are typed rather than encoded as log strings;
- parallel replications do not change the result of an individual run; and
- performance optimizations have reference implementations or differential tests.

Correctness should be defended with unit tests, geometric property tests, scenario invariants, golden deterministic traces, differential tests between coarse and fine fidelity, and statistical tests over ensembles. A visually plausible trace is useful evidence, but never sufficient evidence.

## Risks and open questions

### Scientific validity

The largest risk is producing precise-looking results from weak behavioral assumptions. The response is to attach provenance and uncertainty to parameters, distinguish calibration from validation, publish failed validation targets, and limit claims to the evidence available.

### Rare events

Directly matching annual crash counts may require impractically many simulated hours and still suffer from limited local observations. Conflict metrics, statistical safety models, and carefully weighted rare-event methods will be necessary; their agreement cannot be assumed.

### Parameter identifiability

Different combinations of reaction time, desired gap, perception error, and risk tolerance may reproduce the same queue length while predicting different conflicts. Calibration must use several observables and report parameter uncertainty rather than selecting a single unexplained “best” profile.

### Fidelity and performance

Fine steps and complex envelopes can consume most computation without improving the decision being made. Hekate needs benchmarked fidelity presets and convergence evidence. The user should be able to spend precision specifically on lateral interaction or collision detection rather than globally increasing all work.

### General geometry versus usable authoring

General primitives can become tedious and error-prone. Templates, validators, visual editing, and derived conflict detection should provide convenience while keeping the primitive model authoritative.

### Optimization validity

An optimizer will find loopholes in the simulator and objective. Designs must satisfy explicit feasibility constraints, protect every mode through separate metrics, and survive higher-fidelity and out-of-sample evaluation before being considered promising.

### Questions to resolve through prototypes

- Is a lane/path graph plus free-space polygons sufficient, or is a navigation mesh needed from the start for pedestrians?
- Which collision representation gives the best first balance: circles/capsules, oriented boxes, or convex polygons?
- What minimum trajectory data supports TTC and PET without making output the dominant cost?
- Should the initial control model use general finite-state machines or a smaller phase/rule vocabulary that later compiles to them?
- What determinism guarantees are practical across platforms and parallel execution?
- Which field dataset can serve as the first end-to-end calibration and validation case?

## Research foundation

This vision uses existing methods as starting points, not as unquestioned truth. The initial research spine is:

- microscopic, continuous agent motion and explicit numerical step controls;
- established vehicle-following and pedestrian model families behind replaceable interfaces;
- FHWA-style calibration and validation against field operations;
- trajectory-based surrogate safety analysis for frequent, comparable signals; and
- HSM-style statistical methods and observed crash datasets for cautious absolute safety claims.

As Hekate grows, every behavioral model and default parameter distribution should have a short model card: intended population and context, data source, calibration procedure, validation evidence, uncertainty, known failure modes, and incompatible fidelity settings.

## Enduring vision

Hekate should let us pull apart the knot of geometry, control, demand, human variation, and chance that makes an intersection safe or dangerous. Its value will come from combining freedom of design with discipline of evidence: any layout can be represented, any assumption can be inspected, and no result is more certain than its calibration allows.

If that foundation is sound, richer modes, streetscapes, networks, and automated design search become extensions of the same system rather than rewrites of it.

## References

[^sumo-step]: Eclipse SUMO, [“Basic Definition: Defining the Time Step Length”](https://sumo.dlr.de/docs/Simulation/Basic_Definition.html). Documents separate simulation and action step lengths and their accuracy/performance implications.

[^sumo-sublane]: Eclipse SUMO, [“Sublane Model”](https://sumo.dlr.de/docs/Simulation/SublaneModel.html). Describes continuous lateral position with configurable granularity for decision-making and collision detection.

[^crss]: National Highway Traffic Safety Administration, [“Crash Report Sampling System”](https://www.nhtsa.gov/crash-data-systems/crash-report-sampling-system). CRSS is a nationally representative sample of police-reported crashes involving motor vehicles, pedestrians, and cyclists.

[^fars]: National Highway Traffic Safety Administration, [“Fatality Analysis Reporting System”](https://www.nhtsa.gov/research-data/fatality-analysis-reporting-system-fars). FARS is a nationwide census of fatal motor-vehicle traffic crashes.

[^fhwa-calibration]: Federal Highway Administration, [*Traffic Analysis Toolbox Volume III: Guidelines for Applying Traffic Microsimulation Modeling Software* (2019 update)](https://ops.fhwa.dot.gov/publications/fhwahop18036/).

[^idm]: Martin Treiber, Ansgar Hennecke, and Dirk Helbing, [“Congested Traffic States in Empirical Observations and Microscopic Simulations”](https://arxiv.org/abs/cond-mat/0002177) (2000). Introduces the Intelligent Driver Model in an empirical simulation study.

[^social-force]: Dirk Helbing and Péter Molnár, [“Social Force Model for Pedestrian Dynamics”](https://doi.org/10.1103/PhysRevE.51.4282), *Physical Review E* 51 (1995), 4282–4286.

[^ssam]: Federal Highway Administration, [*Surrogate Safety Assessment Model and Validation: Final Report*](https://www.fhwa.dot.gov/publications/research/safety/08051/08051.pdf), FHWA-HRT-08-051 (2008).

[^hsm]: Federal Highway Administration, [“Highway Safety Manual”](https://highways.dot.gov/safety/data-analysis-tools/highway-safety-manual). Overview of predictive safety methods, Safety Performance Functions, and supporting tools.
