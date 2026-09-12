//! The deterministic, fixed-step simulation kernel.
//!
//! `Simulation` owns the authoritative clock. It advances exactly one
//! configured tick per [`Simulation::step`] call and never reads wall-clock
//! time, so frame rate, pause, and speed controls cannot change results.
//! Increment 2 slice A adds scenario-driven portal demand with route
//! assignment, per-agent profile sampling, and safe spawn admission. Slice B
//! adds path-distance tracking under a documented IDM longitudinal controller
//! ([`crate::control`]), leader/following/queue/exit behavior, authored stop
//! lines, the fixed-time signal phase state machine ([`crate::signal`]), and the
//! contextual signal-compliance decision ([`crate::compliance`]) that gates the
//! stop-line constraint and records a traceable reason. Increment 3 slice A
//! adds pedestrian agents generated from pedestrian demand: they share the
//! agent store, identifier space, events, and clock with vehicles and track
//! their assigned route at the sampled walking speed. Increment 3 slice B
//! replaces that constant-speed tracking with the documented pedestrian
//! waypoint controller ([`crate::pedestrian`]): waypoint and path-progress
//! tracking, bounded steering, and local collision avoidance against every
//! nearby body. Mixed interaction beyond avoidance is a later slice.

use glam::DVec2;
use tangle_model::{
    CompiledMovement, CompiledPedestrianRoute, CompiledScenario, DemandId, MovementId, PathEnd,
    PathId, PedestrianDemandId, PedestrianRouteId, PortalId, SignalColor, SignalId,
};

use crate::agent::{AgentId, AgentInit, AgentMode, AgentStore};
use crate::compliance::{self, ComplianceDecision, SignalAction};
use crate::config::RunConfig;
use crate::control::{self, Constraint, IDM_STANDSTILL_GAP_M};
use crate::demand::{DemandRuntime, MAX_PENDING_SPAWNS, sample_pedestrian_route, sample_route};
use crate::event::{DespawnReason, Event};
use crate::pedestrian::{self, Conflict, PedestrianState, PedestrianWaypoint};
use crate::profile::{
    PedestrianProfile, VehicleProfile, sample_pedestrian_profile, sample_profile,
};
use crate::rng::{
    STREAM_COMPLIANCE, STREAM_DEMAND, STREAM_PEDESTRIAN_DEMAND, STREAM_PROFILE, derive_stream,
    uniform01,
};
use crate::signal::{self, SignalRuntime};
use crate::snapshot::{AgentSample, MotionSample, Snapshot, SnapshotDetail};
use crate::time::SimTime;
use crate::units::Seconds;

/// Extra clearance in metres demanded beyond two bodies' half-lengths when a
/// portal admits a vehicle.
///
/// Admission is conservative: a vehicle enters only when its entry body clears
/// every live body on the same path. This is a portal rule, not a collision
/// check; exact box queries spanning crossing paths are Increment 4 work.
const MIN_SPAWN_CLEARANCE_M: f64 = 1.0;

/// Failure to build a [`Simulation`] from a compiled scenario and run config.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InitError {
    /// The fixed step is not finite and positive.
    #[error("fixed step must be finite and positive, got {step} s")]
    InvalidStep {
        /// The rejected step in seconds.
        step: f64,
    },
    /// The population requests vehicles but the scenario declares no path.
    #[error("cannot spawn {vehicles} vehicles because the scenario has no guide path")]
    NoGuidePath {
        /// Requested vehicle count.
        vehicles: u32,
    },
    /// The guide path cannot hold the requested initial population.
    #[error("population requests {requested} vehicles but the guide path fits only {capacity}")]
    PopulationOverflow {
        /// Requested vehicle count.
        requested: u32,
        /// Vehicles that fit at the configured spacing.
        capacity: u32,
    },
}

/// Typed events produced by a single step.
///
/// The output borrows the kernel's reused event buffer, so consuming it costs
/// no allocation and transfers no world state.
#[derive(Debug)]
pub struct StepOutput<'a> {
    time: SimTime,
    events: &'a [Event],
}

impl StepOutput<'_> {
    /// Simulation time after the step.
    pub fn time(&self) -> SimTime {
        self.time
    }

    /// Events emitted by the step, in stable agent order.
    pub fn events(&self) -> &[Event] {
        self.events
    }

    /// Whether the step emitted no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Immutable summary of a completed run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    scenario_id: String,
    seed: u64,
    ticks: u64,
    elapsed: Seconds,
    spawned: u64,
    despawned: u64,
    dropped: u64,
    remaining: usize,
}

impl RunSummary {
    /// Authored scenario identifier.
    pub fn scenario_id(&self) -> &str {
        &self.scenario_id
    }

    /// Root seed recorded for the run.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Completed fixed steps.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Simulated duration.
    pub fn elapsed(&self) -> Seconds {
        self.elapsed
    }

    /// Agents spawned over the whole run.
    pub fn spawned(&self) -> u64 {
        self.spawned
    }

    /// Agents despawned over the whole run.
    pub fn despawned(&self) -> u64 {
        self.despawned
    }

    /// Vehicle and pedestrian arrivals shed because a pending queue was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Agents still alive at the end.
    pub fn remaining(&self) -> usize {
        self.remaining
    }
}

/// The kernel: a compiled scenario plus authoritative clock and agent state.
#[derive(Debug)]
pub struct Simulation {
    scenario: CompiledScenario,
    config: RunConfig,
    tick: u64,
    agents: AgentStore,
    demand: Vec<DemandRuntime<MovementId>>,
    pedestrian_demand: Vec<DemandRuntime<PedestrianRouteId>>,
    signals: Vec<SignalRuntime>,
    signal_heads: Vec<Option<(SignalId, usize)>>,
    /// Waypoints of every compiled pedestrian route, in travel order and
    /// indexed by dense route id.
    waypoints: Vec<Vec<PedestrianWaypoint>>,
    /// Reused per-pedestrian neighbour buffer, so the avoidance scan does not
    /// allocate inside the tick loop.
    conflicts: Vec<Conflict>,
    events: Vec<Event>,
    population_announced: bool,
    spawned_total: u64,
    despawned_total: u64,
    dropped_total: u64,
    emergency_cap_steps: u64,
    pedestrian_cap_steps: u64,
}

impl Simulation {
    /// Build a simulation from a compiled scenario and run configuration.
    ///
    /// When the scenario declares demand sources of either mode, agents arrive
    /// over time at their portals and the static walking-skeleton population is
    /// not used. Otherwise the initial population is placed immediately along
    /// the first guide path at the configured spacing, so a snapshot before the
    /// first step already shows the starting state. The first [`Self::step`]
    /// announces an initial population with [`Event::Spawned`] before
    /// advancing the clock.
    pub fn new(scenario: CompiledScenario, config: RunConfig) -> Result<Self, InitError> {
        let step = config.step();
        if !step.is_finite_positive() {
            return Err(InitError::InvalidStep {
                step: step.as_secs(),
            });
        }

        // One `demand` substream per vehicle source and one
        // `pedestrian_demand` substream per pedestrian source: arrivals at
        // different portals are independent, and the mode-specific stream
        // names keep the two modes from sharing a generator.
        let demand: Vec<DemandRuntime<MovementId>> = scenario
            .demand()
            .iter()
            .enumerate()
            .map(|(index, _)| {
                DemandRuntime::new(
                    index,
                    derive_stream(config.seed(), STREAM_DEMAND, index as u32),
                )
            })
            .collect();
        let pedestrian_demand: Vec<DemandRuntime<PedestrianRouteId>> = scenario
            .pedestrian_demand()
            .iter()
            .enumerate()
            .map(|(index, _)| {
                DemandRuntime::new(
                    index,
                    derive_stream(config.seed(), STREAM_PEDESTRIAN_DEMAND, index as u32),
                )
            })
            .collect();

        let population = *scenario.population();
        let signals: Vec<SignalRuntime> = scenario
            .signals()
            .iter()
            .map(|signal| SignalRuntime::new(signal.cycle_s()))
            .collect();
        let signal_heads = signal::movement_signal_map(&scenario);
        // Waypoint plans are derived once per run from the compiled routes, so
        // the tick loop only reads them.
        let waypoints: Vec<Vec<PedestrianWaypoint>> = scenario
            .pedestrian_routes()
            .iter()
            .map(|route| {
                let (_, direction) = route_entry(&scenario, route);
                pedestrian::plan_route(&scenario, route, direction)
            })
            .collect();
        let mut agents = AgentStore::default();
        let mut spawned_total = 0;
        if demand.is_empty() && pedestrian_demand.is_empty() && population.vehicle_count > 0 {
            let path = scenario.paths().first().ok_or(InitError::NoGuidePath {
                vehicles: population.vehicle_count,
            })?;
            let capacity = spawn_capacity(
                path.length(),
                population.vehicle_spacing_m,
                population.vehicle_length_m,
            );
            if population.vehicle_count > capacity {
                return Err(InitError::PopulationOverflow {
                    requested: population.vehicle_count,
                    capacity,
                });
            }

            for index in 0..population.vehicle_count {
                let distance_m = f64::from(index) * population.vehicle_spacing_m;
                agents.push(AgentInit {
                    mode: AgentMode::Vehicle,
                    path: path.id(),
                    distance_m,
                    speed_mps: population.vehicle_speed_mps,
                    position: path.position_at(distance_m),
                    heading_rad: path.heading_at(distance_m),
                    body_length_m: population.vehicle_length_m,
                    body_width_m: population.vehicle_width_m,
                    direction: 1.0,
                    movement: None,
                    profile: None,
                    pedestrian_route: None,
                    pedestrian_profile: None,
                });
            }
            spawned_total = u64::from(population.vehicle_count);
        }

        Ok(Self {
            scenario,
            config,
            tick: 0,
            agents,
            demand,
            pedestrian_demand,
            signals,
            signal_heads,
            waypoints,
            conflicts: Vec::new(),
            events: Vec::new(),
            population_announced: false,
            spawned_total,
            despawned_total: 0,
            dropped_total: 0,
            emergency_cap_steps: 0,
            pedestrian_cap_steps: 0,
        })
    }

    /// Advance exactly one configured tick and return its typed events.
    ///
    /// Never reads wall-clock time. The first call also announces the initial
    /// population with [`Event::Spawned`] events.
    pub fn step(&mut self) -> StepOutput<'_> {
        self.events.clear();
        if !self.population_announced {
            self.population_announced = true;
            self.announce_initial_population();
        }
        self.advance_one_tick();
        StepOutput {
            time: self.time(),
            events: &self.events,
        }
    }

    /// Authoritative simulation time.
    pub fn time(&self) -> SimTime {
        SimTime::from_tick(self.tick, self.config.step())
    }

    /// Take an observer view of every live agent.
    pub fn snapshot(&self, detail: SnapshotDetail) -> Snapshot {
        let agents = (0..self.agents.len())
            .filter(|&index| self.agents.alive[index])
            .map(|index| AgentSample {
                id: AgentId::from_index(index),
                position: self.agents.position[index],
                heading_rad: self.agents.heading_rad[index],
                motion: match detail {
                    SnapshotDetail::Position => None,
                    SnapshotDetail::Full => Some(MotionSample {
                        mode: self.agents.mode[index],
                        speed_mps: self.agents.speed_mps[index],
                        path: self.agents.path[index],
                        path_distance_m: self.agents.distance_m[index],
                        body_length_m: self.agents.body_length_m[index],
                        body_width_m: self.agents.body_width_m[index],
                        route: self.agents.movement[index],
                        profile: self.agents.profile[index],
                        pedestrian_route: self.agents.pedestrian_route[index],
                        pedestrian_profile: self.agents.pedestrian_profile[index],
                        decision: self.agents.decision[index],
                    }),
                },
            })
            .collect();

        Snapshot::new(self.scenario.id().to_owned(), self.time(), detail, agents)
    }

    /// Consume the simulation and report what happened.
    pub fn finish(self) -> RunSummary {
        RunSummary {
            scenario_id: self.scenario.id().to_owned(),
            seed: self.config.seed(),
            ticks: self.tick,
            elapsed: Seconds::from_secs(self.time().seconds()),
            spawned: self.spawned_total,
            despawned: self.despawned_total,
            dropped: self.dropped_total,
            remaining: self.agents.alive_count(),
        }
    }

    /// The compiled scenario this simulation runs.
    pub fn scenario(&self) -> &CompiledScenario {
        &self.scenario
    }

    /// The run configuration.
    pub fn config(&self) -> RunConfig {
        self.config
    }

    /// Number of live agents.
    pub fn agent_count(&self) -> usize {
        self.agents.alive_count()
    }

    /// Vehicles waiting for safe admission at one demand source.
    pub fn pending_arrivals(&self, demand: DemandId) -> usize {
        self.demand
            .get(demand.index())
            .map_or(0, DemandRuntime::pending_len)
    }

    /// Arrivals shed because a demand source's pending queue was full.
    pub fn dropped_arrivals(&self, demand: DemandId) -> u64 {
        self.demand
            .get(demand.index())
            .map_or(0, |runtime| runtime.dropped)
    }

    /// Pedestrians waiting for safe admission at one demand source.
    pub fn pending_pedestrian_arrivals(&self, demand: PedestrianDemandId) -> usize {
        self.pedestrian_demand
            .get(demand.index())
            .map_or(0, DemandRuntime::pending_len)
    }

    /// Pedestrian arrivals shed because a demand source's pending queue was
    /// full.
    pub fn dropped_pedestrian_arrivals(&self, demand: PedestrianDemandId) -> u64 {
        self.pedestrian_demand
            .get(demand.index())
            .map_or(0, |runtime| runtime.dropped)
    }

    /// Mode of an agent slot, or `None` when the identifier has no slot.
    pub fn agent_mode(&self, agent: AgentId) -> Option<AgentMode> {
        self.agents.mode.get(agent.index()).copied()
    }

    /// Route assigned to an agent, present for demand-generated vehicles.
    pub fn agent_route(&self, agent: AgentId) -> Option<MovementId> {
        self.agents.movement.get(agent.index()).copied().flatten()
    }

    /// Sampled profile of an agent, present for demand-generated vehicles.
    pub fn agent_profile(&self, agent: AgentId) -> Option<VehicleProfile> {
        self.agents.profile.get(agent.index()).copied().flatten()
    }

    /// Route assigned to an agent, present for demand-generated pedestrians.
    pub fn agent_pedestrian_route(&self, agent: AgentId) -> Option<PedestrianRouteId> {
        self.agents
            .pedestrian_route
            .get(agent.index())
            .copied()
            .flatten()
    }

    /// Sampled pedestrian body and gait, present for demand-generated
    /// pedestrians.
    pub fn agent_pedestrian_profile(&self, agent: AgentId) -> Option<PedestrianProfile> {
        self.agents
            .pedestrian_profile
            .get(agent.index())
            .copied()
            .flatten()
    }

    /// Steps so far in which a safety position cap forced more braking than
    /// the sampled profile's comfortable deceleration.
    ///
    /// IDM's commanded acceleration is clamped to `[-b, +a_max]`, so the
    /// profile bound holds for the model command. The two position caps (the
    /// nearest leader's rear and a required stop line) are emergency backstops
    /// outside that clamp and can demand a single larger deceleration. This
    /// counter is the assertion seam for that documented exception: the
    /// controlled car-following benchmark requires it to stay `0`.
    pub fn emergency_cap_steps(&self) -> u64 {
        self.emergency_cap_steps
    }

    /// Derived waypoints of a compiled pedestrian route, in travel order with
    /// the route exit last.
    ///
    /// Empty for an unknown route. See [`crate::pedestrian`] for how the
    /// waypoints are derived and ordered.
    pub fn route_waypoints(&self, route: PedestrianRouteId) -> &[PedestrianWaypoint] {
        self.waypoints.get(route.index()).map_or(&[], Vec::as_slice)
    }

    /// The waypoint a pedestrian is currently steering toward.
    ///
    /// Present for a live demand pedestrian, whose cursor advances along its
    /// route's derived waypoints and never moves backward.
    pub fn pedestrian_waypoint(&self, agent: AgentId) -> Option<PedestrianWaypoint> {
        let route = self
            .agents
            .pedestrian_route
            .get(agent.index())
            .copied()
            .flatten()?;
        let cursor = *self.agents.pedestrian_waypoint_index.get(agent.index())?;
        self.route_waypoints(route).get(cursor).copied()
    }

    /// Steps so far in which the pedestrian spacing cap forced more braking
    /// than the steering model's bounded deceleration.
    ///
    /// The pedestrian controller's heading and speed bounds hold for its
    /// steering command; the hard spacing cap that keeps a pedestrian from
    /// tunnelling into a nearby body is an emergency backstop outside that
    /// bound. This counter is the assertion seam for that documented exception,
    /// mirroring [`Self::emergency_cap_steps`] for vehicles.
    pub fn pedestrian_cap_steps(&self) -> u64 {
        self.pedestrian_cap_steps
    }

    /// Most recent signal-compliance decision of an agent.
    ///
    /// Present for vehicles whose movement is signal-controlled, including a
    /// green proceed, so every state-affecting signal decision is traceable.
    pub fn agent_decision(&self, agent: AgentId) -> Option<ComplianceDecision> {
        self.agents.decision.get(agent.index()).copied().flatten()
    }

    /// Current display color of the signal head controlling a movement.
    ///
    /// `None` when the movement has no signal rule or no matching head. This is
    /// the observable seam for the fixed-time phase state machine; it reports
    /// the authored color only and makes no compliance decision.
    pub fn movement_signal(&self, movement: MovementId) -> Option<SignalColor> {
        let (signal_id, head_index) = self.signal_heads.get(movement.index()).copied().flatten()?;
        let signal = self.scenario.signal(signal_id)?;
        let elapsed = self
            .signals
            .get(signal_id.index())
            .map_or(0.0, |runtime| runtime.elapsed_s());
        signal::head_color(signal, head_index, elapsed)
    }

    /// The root seed recorded for the run.
    pub fn seed(&self) -> u64 {
        self.config.seed()
    }

    fn announce_initial_population(&mut self) {
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            self.events.push(Event::Spawned {
                agent: AgentId::from_index(index),
                path: self.agents.path[index],
                distance_m: self.agents.distance_m[index],
            });
        }
    }

    fn advance_one_tick(&mut self) {
        self.tick += 1;
        let dt = self.config.step().as_secs();

        // Fixed-time phases advance with the authoritative clock, so the color
        // governing this interval is a function of simulation time only.
        for runtime in &mut self.signals {
            runtime.advance(dt);
        }

        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            match self.agents.mode[index] {
                AgentMode::Pedestrian => self.step_pedestrian(index, dt),
                AgentMode::Vehicle => self.step_vehicle(index, dt),
            }
        }

        self.advance_demand(dt);
    }

    /// Advance one vehicle under IDM or, without a sampled profile, at the
    /// static walking skeleton's constant speed.
    fn step_vehicle(&mut self, index: usize, dt: f64) {
        let path_id = self.agents.path[index];
        // Recompute the agent's signal decision before integrating its
        // speed, so the stop-line constraint the controller sees is exactly
        // the recorded decision.
        self.update_signal_decision(index);
        let direction = self.agents.direction[index];
        // Profile vehicles drive under IDM; the walking skeleton's static
        // population keeps its constant speed and is byte-identical.
        let profile = self.agents.profile[index];
        let speed_mps = match profile {
            Some(profile) => self.controlled_speed(index, &profile, dt),
            None => self.agents.speed_mps[index],
        };
        let Some(path) = self.scenario.path(path_id) else {
            return;
        };
        let travelled = self.agents.distance_m[index] + speed_mps * direction * dt;
        let length = path.length();
        let distance_m = if direction < 0.0 {
            travelled.max(0.0)
        } else {
            travelled.min(length)
        };

        self.agents.speed_mps[index] = speed_mps;
        self.agents.distance_m[index] = distance_m;
        self.agents.position[index] = path.position_at(distance_m);
        self.agents.heading_rad[index] = path.heading_at(distance_m);

        let exited = if direction < 0.0 {
            travelled <= 0.0
        } else {
            travelled >= length
        };
        if exited {
            self.agents.alive[index] = false;
            self.despawned_total += 1;
            self.events.push(Event::Despawned {
                agent: AgentId::from_index(index),
                path: path_id,
                reason: DespawnReason::ExitedPath,
            });
        }
    }

    /// Advance one pedestrian under the documented waypoint controller.
    ///
    /// A pedestrian is steered in world space rather than integrated along its
    /// path: it seeks the next derived waypoint, deflects around the nearby
    /// bodies ahead of it, and reports route progress as the projection of its
    /// world position onto the route path. See [`crate::pedestrian`] for the
    /// model card, its bounds, and its emergency spacing cap.
    fn step_pedestrian(&mut self, index: usize, dt: f64) {
        let (Some(route_id), Some(profile)) = (
            self.agents.pedestrian_route[index],
            self.agents.pedestrian_profile[index],
        ) else {
            // A pedestrian slot without a demand route has no waypoint plan; it
            // keeps the walking skeleton's constant-speed path.
            self.step_vehicle(index, dt);
            return;
        };
        let path_id = self.agents.path[index];
        let direction = self.agents.direction[index];
        let path_length_m = self
            .scenario
            .path(path_id)
            .map_or(0.0, |path| path.length());

        // Advance the cursor past every waypoint already reached, so the target
        // is always ahead and the cursor is monotone.
        let progress_m =
            pedestrian::route_progress_m(self.agents.distance_m[index], direction, path_length_m);
        self.advance_waypoint(index, progress_m);
        let cursor = self.agents.pedestrian_waypoint_index[index];
        let Some(target) = self.route_waypoints(route_id).get(cursor).copied() else {
            return;
        };

        self.collect_conflicts(index);
        let state = PedestrianState {
            agent: AgentId::from_index(index),
            position: self.agents.position[index],
            heading_rad: self.agents.heading_rad[index],
            speed_mps: self.agents.speed_mps[index],
            target: target.position(),
        };
        let steering = pedestrian::steer(&profile, &state, &self.conflicts, dt);
        let (speed_mps, capped) = pedestrian::advance_speed(
            state.speed_mps,
            steering.speed_target_mps,
            steering.spacing_cap_mps,
            dt,
        );
        if capped {
            self.pedestrian_cap_steps += 1;
        }
        let heading_rad = steering.heading_rad;
        let position = state.position + DVec2::from_angle(heading_rad) * (speed_mps * dt);

        let Some(path) = self.scenario.path(path_id) else {
            return;
        };
        let arc_m = pedestrian::closest_arc(path, position);
        let progress_m = pedestrian::route_progress_m(arc_m, direction, path_length_m);

        self.agents.speed_mps[index] = speed_mps;
        self.agents.heading_rad[index] = heading_rad;
        self.agents.position[index] = position;
        self.agents.distance_m[index] = arc_m;

        if progress_m >= path_length_m {
            self.agents.alive[index] = false;
            self.despawned_total += 1;
            self.events.push(Event::Despawned {
                agent: AgentId::from_index(index),
                path: path_id,
                reason: DespawnReason::ExitedPath,
            });
        }
    }

    /// Move a pedestrian's waypoint cursor past every waypoint it has reached.
    ///
    /// The cursor is monotone: it advances past a waypoint only once the
    /// pedestrian's route progress has reached it, and never off the last
    /// waypoint, so the current target is always the next waypoint ahead.
    fn advance_waypoint(&mut self, index: usize, progress_m: f64) {
        let Some(route_id) = self.agents.pedestrian_route[index] else {
            return;
        };
        let path_length_m = self
            .scenario
            .path(self.agents.path[index])
            .map_or(0.0, |path| path.length());
        let direction = self.agents.direction[index];
        let Some(plan) = self.waypoints.get(route_id.index()) else {
            return;
        };
        let mut cursor = self.agents.pedestrian_waypoint_index[index];
        while cursor + 1 < plan.len()
            && progress_m >= pedestrian::waypoint_progress_m(plan[cursor], direction, path_length_m)
        {
            cursor += 1;
        }
        self.agents.pedestrian_waypoint_index[index] = cursor;
    }

    /// Gather the bodies a pedestrian must avoid, in ascending agent id order.
    ///
    /// The order is the documented scan order and tie-break, so the interaction
    /// terms are summed deterministically and no hash map is iterated for
    /// state-affecting logic. Bodies beyond [`pedestrian::SENSE_RADIUS_M`] are
    /// not considered.
    fn collect_conflicts(&mut self, index: usize) {
        self.conflicts.clear();
        let position = self.agents.position[index];
        let radius_m = self.agents.body_length_m[index] * 0.5;
        for other in 0..self.agents.len() {
            if other == index || !self.agents.alive[other] {
                continue;
            }
            let other_position = self.agents.position[other];
            if (other_position - position).length() > pedestrian::SENSE_RADIUS_M {
                continue;
            }
            let nearest = match self.agents.mode[other] {
                AgentMode::Pedestrian => pedestrian::nearest_circle_point(
                    other_position,
                    self.agents.body_length_m[other] * 0.5,
                    position,
                ),
                AgentMode::Vehicle => pedestrian::nearest_box_point(
                    other_position,
                    self.agents.heading_rad[other],
                    self.agents.body_length_m[other],
                    self.agents.body_width_m[other],
                    position,
                ),
            };
            // The conflict reports the vector from the pedestrian's centre to
            // the neighbour's nearest surface point, which is what the steering
            // model's direction and clearance both need.
            let to_surface = nearest - position;
            self.conflicts.push(Conflict {
                agent: AgentId::from_index(other),
                to_surface,
                clearance_m: to_surface.length() - radius_m,
                speed_mps: self.agents.speed_mps[other],
            });
        }
    }

    /// Speed for one profile vehicle after IDM, its bounds, and safety caps.
    ///
    /// The controller is the documented IDM in [`crate::control`]. Two hard
    /// caps keep the bounded model collision-free without an extra collision
    /// resolver: the next speed may not pass the nearest leader's rear, and it
    /// may not pass a stop line whose control requires a stop. Each cap can
    /// force a deceleration beyond the profile's comfortable value, so a step
    /// where it does is counted in [`Self::emergency_cap_steps`].
    fn controlled_speed(&mut self, index: usize, profile: &VehicleProfile, dt: f64) -> f64 {
        let leader = self.nearest_leader(index);
        let stop_line = self.stop_line_constraint(index);

        let mut constraints = [Constraint {
            gap_m: f64::INFINITY,
            speed_mps: 0.0,
            standstill_m: 0.0,
        }; 2];
        let mut count = 0;
        for constraint in [leader, stop_line].into_iter().flatten() {
            constraints[count] = constraint;
            count += 1;
        }

        let speed_mps = self.agents.speed_mps[index];
        let accel = control::desired_acceleration(profile, speed_mps, &constraints[..count]);
        let mut new_speed = (speed_mps + accel * dt).clamp(0.0, profile.desired_speed_mps);

        if let Some(leader) = leader {
            // The follower's front bumper must not pass the leader's rear this
            // step. The gap already reflects the leader's post-step position
            // (lower-index leaders integrate first), so the reachable speed is
            // `gap / dt`. This binds only when IDM's bounded braking is
            // insufficient.
            new_speed = new_speed.min(leader.gap_m / dt);
        }
        if let Some(stop_line) = stop_line {
            // A required stop holds the front bumper at the authored line.
            new_speed = new_speed.min(stop_line.gap_m / dt);
        }
        let new_speed = new_speed.max(0.0);

        // Comfortable braking alone would leave the vehicle at this speed, so
        // anything below it was forced by a cap rather than by the controller.
        let comfort_floor = (speed_mps - profile.comfortable_brake_mps2 * dt).max(0.0);
        if new_speed < comfort_floor - 1e-9 {
            self.emergency_cap_steps += 1;
        }
        new_speed
    }

    /// Nearest live leader ahead on the same path travelling the same way.
    ///
    /// The gap is bumper to bumper along the path. Iterating in ascending agent
    /// order means the lowest agent id wins a tie, which keeps tie-breaking
    /// stable across runs. Opposite-direction and crossing-path interaction is
    /// later work.
    fn nearest_leader(&self, index: usize) -> Option<Constraint> {
        let direction = self.agents.direction[index];
        let own_progress = direction * self.agents.distance_m[index];
        let own_front = own_progress + self.agents.body_length_m[index] * 0.5;
        let mut best: Option<(usize, f64)> = None;
        for other in 0..self.agents.len() {
            if other == index || !self.agents.alive[other] {
                continue;
            }
            if self.agents.path[other] != self.agents.path[index]
                || self.agents.direction[other] != direction
            {
                continue;
            }
            let other_progress = direction * self.agents.distance_m[other];
            if other_progress <= own_progress {
                continue;
            }
            let gap = other_progress - self.agents.body_length_m[other] * 0.5 - own_front;
            if best.is_none_or(|(_, best_gap)| gap < best_gap) {
                best = Some((other, gap));
            }
        }
        best.map(|(other, gap)| Constraint {
            gap_m: gap.max(0.0),
            speed_mps: self.agents.speed_mps[other],
            standstill_m: IDM_STANDSTILL_GAP_M,
        })
    }

    /// Compute and record the agent's current signal-compliance decision.
    fn update_signal_decision(&mut self, index: usize) {
        let decision = self.signal_decision(index);
        self.agents.decision[index] = decision;
    }

    /// The contextual signal-compliance decision for one agent, if its
    /// movement is signal-controlled.
    ///
    /// Pure in the agent's current state and the governing head; see
    /// [`crate::compliance`] for the model. The recorded decision is what the
    /// inspector shows and what [`Self::stop_line_constraint`] obeys.
    fn signal_decision(&self, index: usize) -> Option<ComplianceDecision> {
        let movement_id = self.agents.movement[index]?;
        let color = self.movement_signal(movement_id)?;
        let profile = self.agents.profile[index]?;
        let movement = self.scenario.movement(movement_id)?;
        let path = self.scenario.path(self.agents.path[index])?;
        let direction = self.agents.direction[index];
        // `stop_line_m` is measured from the movement entry along travel, so a
        // backward movement reaches it at `path_length - stop_line_m`.
        let stop_line_distance = if direction < 0.0 {
            path.length() - movement.stop_line_m()
        } else {
            movement.stop_line_m()
        };
        let own_progress = direction * self.agents.distance_m[index];
        let own_front = own_progress + self.agents.body_length_m[index] * 0.5;
        let gap = direction * stop_line_distance - own_front;
        Some(compliance::decide(
            color,
            gap,
            self.agents.speed_mps[index],
            profile.compliance,
            profile.comfortable_brake_mps2,
        ))
    }

    /// Stop-line constraint for an agent whose recorded decision is to stop.
    ///
    /// A vehicle whose decision proceeded (green, already past the line, or a
    /// noncompliant run) returns `None` and continues under ordinary IDM
    /// control; it is never pulled backward or given a special trajectory.
    fn stop_line_constraint(&self, index: usize) -> Option<Constraint> {
        let decision = self.agents.decision[index]?;
        if decision.action != SignalAction::Stop {
            return None;
        }
        Some(Constraint {
            gap_m: decision.stop_line_gap_m.max(0.0),
            speed_mps: 0.0,
            standstill_m: 0.0,
        })
    }

    /// Generate demand arrivals, then admit them subject to portal clearance.
    ///
    /// Random draws are confined to the named streams: the expected number of
    /// arrivals and the route of each arrival come from the source's `demand`
    /// substream, while the profile is sampled from the `profile` stream when
    /// an arrival is admitted. Every state-affecting choice is therefore a
    /// deterministic function of the root seed.
    fn advance_demand(&mut self, dt: f64) {
        self.advance_vehicle_demand(dt);
        self.advance_pedestrian_demand(dt);
    }

    /// Generate vehicle arrivals, then admit them subject to portal clearance.
    fn advance_vehicle_demand(&mut self, dt: f64) {
        for runtime in 0..self.demand.len() {
            let source_index = self.demand[runtime].source;
            let rate_vph = self.scenario.demand()[source_index].rate_vph();
            let expected = rate_vph / 3600.0 * dt;
            let mut arrivals = 0u64;
            if expected.is_finite() && expected > 0.0 {
                arrivals = expected.floor() as u64;
                let fraction = expected - expected.floor();
                if uniform01(&mut self.demand[runtime].rng) < fraction {
                    arrivals += 1;
                }
            }
            for _ in 0..arrivals {
                let movement = sample_route(
                    &self.scenario.demand()[source_index],
                    &mut self.demand[runtime].rng,
                );
                if self.demand[runtime].pending.len() < MAX_PENDING_SPAWNS {
                    self.demand[runtime].pending.push_back(movement);
                } else {
                    self.demand[runtime].dropped += 1;
                    self.dropped_total += 1;
                }
            }
        }

        // Admit in FIFO order per source, stopping at the first blocked arrival
        // so a queue stays ordered and does not jump later arrivals ahead.
        for runtime in 0..self.demand.len() {
            while let Some(&movement) = self.demand[runtime].pending.front() {
                if self.try_admit(movement) {
                    self.demand[runtime].pending.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Generate pedestrian arrivals, then admit them subject to portal
    /// clearance.
    ///
    /// This mirrors the vehicle path with the `pedestrian_demand` stream and
    /// the pedestrian route identifier, so the two modes share the demand
    /// machinery but never a mutable generator.
    fn advance_pedestrian_demand(&mut self, dt: f64) {
        for runtime in 0..self.pedestrian_demand.len() {
            let source_index = self.pedestrian_demand[runtime].source;
            let rate_pph = self.scenario.pedestrian_demand()[source_index].rate_pph();
            let expected = rate_pph / 3600.0 * dt;
            let mut arrivals = 0u64;
            if expected.is_finite() && expected > 0.0 {
                arrivals = expected.floor() as u64;
                let fraction = expected - expected.floor();
                if uniform01(&mut self.pedestrian_demand[runtime].rng) < fraction {
                    arrivals += 1;
                }
            }
            for _ in 0..arrivals {
                let route = sample_pedestrian_route(
                    &self.scenario.pedestrian_demand()[source_index],
                    &mut self.pedestrian_demand[runtime].rng,
                );
                if self.pedestrian_demand[runtime].pending.len() < MAX_PENDING_SPAWNS {
                    self.pedestrian_demand[runtime].pending.push_back(route);
                } else {
                    self.pedestrian_demand[runtime].dropped += 1;
                    self.dropped_total += 1;
                }
            }
        }

        for runtime in 0..self.pedestrian_demand.len() {
            while let Some(&route) = self.pedestrian_demand[runtime].pending.front() {
                if self.try_admit_pedestrian(route) {
                    self.pedestrian_demand[runtime].pending.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Admit one demand vehicle on `movement` when the portal entry is clear.
    fn try_admit(&mut self, movement_id: MovementId) -> bool {
        let Some((path_id, entry_distance, direction)) =
            self.scenario.movement(movement_id).map(|movement| {
                let (entry_distance, direction) = movement_entry(&self.scenario, movement);
                (movement.path(), entry_distance, direction)
            })
        else {
            return false;
        };

        // The profile is derived from the stable agent id, so a blocked arrival
        // re-derives the same profile on the next attempt. Physical/longitudinal
        // parameters use the `profile` stream and the compliance propensity the
        // `compliance` stream; the two never share a generator.
        let agent_id = AgentId::from_index(self.agents.len());
        let profile = sample_profile(
            self.scenario.profiles(),
            &mut derive_stream(self.config.seed(), STREAM_PROFILE, agent_id.get()),
            &mut derive_stream(self.config.seed(), STREAM_COMPLIANCE, agent_id.get()),
        );
        if !self.entry_clear(path_id, entry_distance, profile.length_m) {
            return false;
        }

        let Some(path) = self.scenario.path(path_id) else {
            return false;
        };
        let position = path.position_at(entry_distance);
        let heading_rad = path.heading_at(entry_distance);
        // Enter at a speed from which the profile's comfortable braking can
        // still follow the nearest vehicle ahead; the desired speed remains the
        // target once the entry is safe.
        let speed_mps = self.safe_entry_speed(path_id, entry_distance, direction, &profile);

        self.agents.push(AgentInit {
            mode: AgentMode::Vehicle,
            path: path_id,
            distance_m: entry_distance,
            speed_mps,
            position,
            heading_rad,
            body_length_m: profile.length_m,
            body_width_m: profile.width_m,
            direction,
            movement: Some(movement_id),
            profile: Some(profile),
            pedestrian_route: None,
            pedestrian_profile: None,
        });
        self.spawned_total += 1;
        self.events.push(Event::Spawned {
            agent: agent_id,
            path: path_id,
            distance_m: entry_distance,
        });
        // Record the entry decision immediately so a snapshot taken right after
        // admission already carries a traceable reason.
        self.update_signal_decision(agent_id.index());
        true
    }

    /// Admit one pedestrian on `route_id` when the portal entry is clear.
    ///
    /// A pedestrian enters at its sampled walking speed, heading at the first
    /// waypoint of its route, and is then steered by the documented waypoint
    /// controller. Body shape, demand, route assignment, and admission are
    /// unchanged from the previous slice; a route that names no zone heads at
    /// the route exit.
    fn try_admit_pedestrian(&mut self, route_id: PedestrianRouteId) -> bool {
        let Some((path_id, entry_distance, direction)) =
            self.scenario.pedestrian_route(route_id).map(|route| {
                let (entry_distance, direction) = route_entry(&self.scenario, route);
                (route.path(), entry_distance, direction)
            })
        else {
            return false;
        };

        // The body and gait are derived from the stable agent id, so a blocked
        // arrival re-derives the same profile on the next attempt. Pedestrians
        // share the mode-neutral `profile` stream with vehicles, and agent ids
        // are unique across modes, so no two agents share a generator.
        let agent_id = AgentId::from_index(self.agents.len());
        let profile = sample_pedestrian_profile(
            self.scenario.pedestrian_profiles(),
            &mut derive_stream(self.config.seed(), STREAM_PROFILE, agent_id.get()),
        );
        // A pedestrian body is a circle, so its bounding box is the diameter on
        // both axes.
        let diameter_m = profile.radius_m * 2.0;
        if !self.entry_clear(path_id, entry_distance, diameter_m) {
            return false;
        }

        let Some(path) = self.scenario.path(path_id) else {
            return false;
        };
        let position = path.position_at(entry_distance);
        // Enter heading at the first waypoint, so the controller starts with a
        // zero heading error and a route that enters at the path end heads
        // inward rather than along the path tangent.
        let mut heading_rad = path.heading_at(entry_distance);
        if let Some(first) = self
            .waypoints
            .get(route_id.index())
            .and_then(|plan| plan.first())
        {
            let toward = first.position() - position;
            if toward.length() > 0.0 {
                heading_rad = toward.y.atan2(toward.x);
            }
        }

        self.agents.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path: path_id,
            distance_m: entry_distance,
            speed_mps: profile.desired_speed_mps,
            position,
            heading_rad,
            body_length_m: diameter_m,
            body_width_m: diameter_m,
            direction,
            movement: None,
            profile: None,
            pedestrian_route: Some(route_id),
            pedestrian_profile: Some(profile),
        });
        self.spawned_total += 1;
        self.events.push(Event::Spawned {
            agent: agent_id,
            path: path_id,
            distance_m: entry_distance,
        });
        true
    }

    /// Whether the entry at `entry_distance` on `path` clears every live body.
    ///
    /// The rule is an along-path separation on the same path, which is
    /// conservative for a portal and independent of travel direction. Exact box
    /// queries that also cover crossing paths are Increment 4 work.
    fn entry_clear(&self, path: PathId, entry_distance: f64, candidate_length: f64) -> bool {
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] || self.agents.path[index] != path {
                continue;
            }
            let clearance =
                (self.agents.body_length_m[index] + candidate_length) * 0.5 + MIN_SPAWN_CLEARANCE_M;
            if (self.agents.distance_m[index] - entry_distance).abs() < clearance {
                return false;
            }
        }
        true
    }

    /// Greatest speed at which an entering vehicle can safely follow ahead.
    ///
    /// The entering vehicle is never faster than the speed from which the
    /// profile's comfortable braking brings it to the nearest leader's speed
    /// within the available gap: `v = sqrt(v_leader² + 2·b·gap)`. Without a
    /// leader the desired speed is unchanged. This is what keeps IDM's bounded
    /// comfortable braking sufficient during admission, so the emergency
    /// anti-overlap cap is a backstop rather than the normal regime.
    fn safe_entry_speed(
        &self,
        path: PathId,
        entry_distance: f64,
        direction: f64,
        profile: &VehicleProfile,
    ) -> f64 {
        let entry_progress = direction * entry_distance;
        let entry_front = entry_progress + profile.length_m * 0.5;
        let brake = profile.comfortable_brake_mps2;
        let mut speed_mps = profile.desired_speed_mps;
        for index in 0..self.agents.len() {
            if !self.agents.alive[index]
                || self.agents.path[index] != path
                || self.agents.direction[index] != direction
            {
                continue;
            }
            let progress = direction * self.agents.distance_m[index];
            if progress <= entry_progress {
                continue;
            }
            let gap = progress - self.agents.body_length_m[index] * 0.5 - entry_front;
            let usable = (gap - IDM_STANDSTILL_GAP_M).max(0.0);
            let leader_speed = self.agents.speed_mps[index];
            let limit = (leader_speed * leader_speed + 2.0 * brake * usable).sqrt();
            speed_mps = speed_mps.min(limit);
        }
        speed_mps.max(0.0)
    }
}

/// Vehicles that fit along a path at the configured centre-to-centre spacing.
///
/// The last vehicle's front bumper must remain on the path, so the usable
/// placement span is `path_length - body_length`.
fn spawn_capacity(path_length_m: f64, spacing_m: f64, body_length_m: f64) -> u32 {
    let usable = path_length_m - body_length_m;
    if !usable.is_finite() || usable < 0.0 || !spacing_m.is_finite() || spacing_m <= 0.0 {
        return 0;
    }
    let slots = (usable / spacing_m).floor();
    if slots >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        slots as u32 + 1
    }
}

/// Entry arc length and travel direction for a movement.
///
/// A movement enters at its `from` portal. On the path start it enters at
/// distance zero travelling forward; on the path end it enters at the full
/// length travelling backward.
fn movement_entry(scenario: &CompiledScenario, movement: &CompiledMovement) -> (f64, f64) {
    let path_length = scenario
        .path(movement.path())
        .map_or(0.0, |path| path.length());
    match portal_end(scenario, movement.from()) {
        PathEnd::End => (path_length, -1.0),
        PathEnd::Start => (0.0, 1.0),
    }
}

/// Entry arc length and travel direction for a pedestrian route.
///
/// A route enters at its `from` portal, exactly as a movement does: on the path
/// start it enters at distance zero travelling forward; on the path end it
/// enters at the full length travelling backward.
fn route_entry(scenario: &CompiledScenario, route: &CompiledPedestrianRoute) -> (f64, f64) {
    let path_length = scenario
        .path(route.path())
        .map_or(0.0, |path| path.length());
    match portal_end(scenario, route.from()) {
        PathEnd::End => (path_length, -1.0),
        PathEnd::Start => (0.0, 1.0),
    }
}

/// The authored end of a compiled portal, falling back to the start.
fn portal_end(scenario: &CompiledScenario, portal: PortalId) -> PathEnd {
    scenario
        .portal(portal)
        .map_or(PathEnd::Start, |portal| portal.end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_STEP;
    use crate::units::Seconds;
    use tangle_model::{CompiledScenario, parse_scenario_source};

    const WALKING: &str = r#"
    {
      schema_version: 1,
      id: 'walking_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 60.0, y: 0.0 }, { x: 120.0, y: 0.0 } ] },
      ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      population: {
        vehicle_count: 6,
        vehicle_speed_mps: 12.0,
        vehicle_spacing_m: 20.0,
        vehicle_length_m: 4.5,
        vehicle_width_m: 1.8,
      },
    }
    "#;

    fn walking_scenario() -> CompiledScenario {
        let source = parse_scenario_source(WALKING).expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    fn walking_sim(seed: u64) -> Simulation {
        Simulation::new(walking_scenario(), RunConfig::new(seed)).expect("simulation builds")
    }

    #[test]
    fn places_initial_population_at_configured_spacing() {
        let sim = walking_sim(1);
        assert_eq!(sim.agent_count(), 6);
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        assert_eq!(snapshot.agents().len(), 6);
        assert_eq!(snapshot.time().tick(), 0);
        let distances: Vec<f64> = snapshot
            .agents()
            .iter()
            .map(|agent| agent.motion.expect("full detail").path_distance_m)
            .collect();
        assert_eq!(distances, vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    }

    #[test]
    fn first_step_announces_initial_population() {
        let mut sim = walking_sim(7);
        let output = sim.step();
        assert_eq!(output.events().len(), 6);
        assert!(
            output
                .events()
                .iter()
                .all(|event| matches!(event, Event::Spawned { .. }))
        );
        assert_eq!(output.time(), SimTime::from_tick(1, DEFAULT_STEP));
    }

    #[test]
    fn step_advances_exactly_one_tick_at_constant_speed() {
        let mut sim = walking_sim(3);
        sim.step();
        assert_eq!(sim.time().tick(), 1);
        let after_one = sim.snapshot(SnapshotDetail::Full);
        let lead = after_one.agents()[0].motion.expect("full detail");
        // 12 m/s * 0.05 s = 0.6 m.
        assert!((lead.path_distance_m - 0.6).abs() < 1e-12);
        assert!((after_one.agents()[0].position - glam::DVec2::new(0.6, 0.0)).length() < 1e-12);

        for _ in 0..9 {
            sim.step();
        }
        assert_eq!(sim.time().tick(), 10);
        assert!((sim.time().seconds() - 0.5).abs() < 1e-12);
        let after_ten = sim.snapshot(SnapshotDetail::Full);
        assert!((after_ten.agents()[0].motion.unwrap().path_distance_m - 6.0).abs() < 1e-12);
    }

    #[test]
    fn despawns_agents_that_reach_the_path_end() {
        let mut sim = walking_sim(11);
        let mut despawned = 0;
        for _ in 0..250 {
            let output = sim.step();
            for event in output.events() {
                if let Event::Despawned {
                    reason: DespawnReason::ExitedPath,
                    ..
                } = event
                {
                    despawned += 1;
                }
            }
        }
        assert_eq!(despawned, 6);
        assert_eq!(sim.agent_count(), 0);
        let summary = sim.finish();
        assert_eq!(summary.spawned(), 6);
        assert_eq!(summary.despawned(), 6);
        assert_eq!(summary.remaining(), 0);
        assert_eq!(summary.seed(), 11);
        assert_eq!(summary.ticks(), 250);
        assert_eq!(summary.scenario_id(), "walking_guide_v1");
    }

    #[test]
    fn despawn_events_arrive_in_stable_agent_order() {
        let mut sim = walking_sim(5);
        let mut order = Vec::new();
        for _ in 0..220 {
            for event in sim.step().events() {
                if let Event::Despawned { agent, .. } = event {
                    order.push(agent.get());
                }
            }
        }
        // The lead vehicle (highest spawn index) exits first, and agents are
        // always emitted in ascending index order within a step.
        assert_eq!(order, vec![5, 4, 3, 2, 1, 0]);
    }

    #[test]
    fn snapshot_detail_controls_motion_payload() {
        let sim = walking_sim(2);
        assert!(
            sim.snapshot(SnapshotDetail::Position)
                .agents()
                .iter()
                .all(|agent| agent.motion.is_none())
        );
        assert!(
            sim.snapshot(SnapshotDetail::Full)
                .agents()
                .iter()
                .all(|agent| agent.motion.is_some())
        );
    }

    #[test]
    fn same_seed_produces_identical_observations() {
        fn trace(seed: u64) -> Vec<(u64, String, Vec<Event>)> {
            let mut sim = walking_sim(seed);
            let mut frames = Vec::new();
            for _ in 0..40 {
                let (tick, events) = {
                    let output = sim.step();
                    (output.time().tick(), output.events().to_vec())
                };
                let snapshot = sim.snapshot(SnapshotDetail::Full);
                let fingerprint = format!("{:?}", snapshot.agents());
                frames.push((tick, fingerprint, events));
            }
            frames
        }

        assert_eq!(trace(99), trace(99));
    }

    #[test]
    fn wall_clock_delay_does_not_change_results() {
        fn run(pause: bool) -> Vec<String> {
            let mut sim = walking_sim(42);
            let mut frames = Vec::new();
            for _ in 0..30 {
                if pause {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                sim.step();
                frames.push(format!("{:?}", sim.snapshot(SnapshotDetail::Full)));
            }
            frames
        }

        assert_eq!(run(false), run(true));
    }

    #[test]
    fn rejects_a_non_positive_step() {
        let error = Simulation::new(
            walking_scenario(),
            RunConfig::new(0).with_step(Seconds::from_secs(0.0)),
        )
        .expect_err("zero step must be rejected");
        assert_eq!(error, InitError::InvalidStep { step: 0.0 });
    }

    #[test]
    fn rejects_a_population_without_a_guide_path() {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'empty', coordinate_system: { x: 'a', y: 'b' }, \
             paths: [], portals: [], population: { vehicle_count: 2, vehicle_speed_mps: 10.0, \
             vehicle_spacing_m: 5.0, vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let error = Simulation::new(scenario, RunConfig::new(0)).expect_err("no path");
        assert_eq!(error, InitError::NoGuidePath { vehicles: 2 });
    }

    #[test]
    fn rejects_a_population_that_does_not_fit() {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'short', coordinate_system: { x: 'a', y: 'b' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ], \
             population: { vehicle_count: 6, vehicle_speed_mps: 10.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let error = Simulation::new(scenario, RunConfig::new(0)).expect_err("does not fit");
        // Usable span 6 m fits a single vehicle at 20 m spacing.
        assert_eq!(
            error,
            InitError::PopulationOverflow {
                requested: 6,
                capacity: 1,
            }
        );
    }

    #[test]
    fn spawn_capacity_counts_the_starting_vehicle() {
        assert_eq!(spawn_capacity(120.0, 20.0, 4.5), 6);
        assert_eq!(spawn_capacity(10.0, 20.0, 4.0), 1);
        assert_eq!(spawn_capacity(3.0, 20.0, 4.0), 0);
        assert_eq!(spawn_capacity(120.0, 0.0, 4.0), 0);
    }

    /// The model card's leader tie-break: two candidates at the same gap are
    /// broken by agent id, not by any other attribute.
    #[test]
    fn leader_ties_resolve_to_the_lowest_agent_id() {
        let mut sim = walking_sim(1);
        // Overlap is normally impossible, so this degenerate tie is constructed
        // directly: agents 1 and 2 occupy the same arc length, which is exactly
        // the state the anti-overlap cap leaves behind when it pins a queue.
        sim.agents.distance_m[2] = sim.agents.distance_m[1];
        sim.agents.speed_mps[1] = 5.0;
        sim.agents.speed_mps[2] = 9.0;
        // The precondition: both candidates sit at exactly the same gap.
        let half_lengths = (sim.agents.body_length_m[1] + sim.agents.body_length_m[0]) * 0.5;
        let gap_to_first = sim.agents.distance_m[1] - sim.agents.distance_m[0] - half_lengths;
        let gap_to_second = sim.agents.distance_m[2] - sim.agents.distance_m[0] - half_lengths;
        assert!(
            (gap_to_first - gap_to_second).abs() < 1e-12,
            "the constructed state must be a genuine tie"
        );
        let leader = sim.nearest_leader(0).expect("a tied leader");
        assert_eq!(
            leader.speed_mps, 5.0,
            "the lowest tied id must win, even when it is the slower candidate"
        );

        // Swapping which tied candidate moves faster must not move the winner.
        sim.agents.speed_mps[1] = 9.0;
        sim.agents.speed_mps[2] = 5.0;
        let leader = sim.nearest_leader(0).expect("a tied leader");
        assert_eq!(
            leader.speed_mps, 9.0,
            "the tie-break follows agent id, not the candidate's speed"
        );
    }

    /// The documented exception to the profile braking bound: when the
    /// anti-overlap cap binds, the step's implied deceleration exceeds `b` and
    /// the emergency counter records it.
    #[test]
    fn the_anti_overlap_cap_is_counted_when_it_brakes_beyond_the_profile_bound() {
        let mut sim = walking_sim(1);
        let profile = VehicleProfile {
            desired_speed_mps: 10.0,
            length_m: 4.0,
            width_m: 2.0,
            time_gap_s: 1.5,
            max_accel_mps2: 2.0,
            comfortable_brake_mps2: 3.0,
            compliance: 1.0,
        };
        // Park the last walker and enter a profile follower 0.1 m behind its
        // rear at 4 m/s. Comfortable braking removes 0.15 m/s in one 0.05 s
        // step, far more than the cap allows, so the cap must bind.
        let leader_index = 5;
        sim.agents.speed_mps[leader_index] = 0.0;
        let path = sim.agents.path[leader_index];
        let distance_m = sim.agents.distance_m[leader_index] - 2.25 - 0.1 - 2.0;
        let path_geometry = sim.scenario.path(path).expect("guide path").clone();
        sim.agents.push(AgentInit {
            mode: AgentMode::Vehicle,
            path,
            distance_m,
            speed_mps: 4.0,
            position: path_geometry.position_at(distance_m),
            heading_rad: path_geometry.heading_at(distance_m),
            body_length_m: profile.length_m,
            body_width_m: profile.width_m,
            direction: 1.0,
            movement: None,
            profile: Some(profile),
            pedestrian_route: None,
            pedestrian_profile: None,
        });

        sim.step();
        let follower = sim.agents.len() - 1;
        // The cap allows at most `gap / dt`; the step therefore decelerates far
        // beyond the profile's comfortable 3.0 m/s².
        assert!(
            sim.agents.speed_mps[follower] < 4.0 - profile.comfortable_brake_mps2 * 0.05,
            "the anti-overlap cap should have braked harder than the profile bound"
        );
        assert_eq!(sim.emergency_cap_steps(), 1);

        // The static population's constant-speed path never engages the cap.
        let mut free = walking_sim(1);
        for _ in 0..20 {
            free.step();
        }
        assert_eq!(free.emergency_cap_steps(), 0);
    }

    #[test]
    fn zero_vehicle_population_produces_no_events() {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'empty', coordinate_system: { x: 'a', y: 'b' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ], \
             population: { vehicle_count: 0, vehicle_speed_mps: 10.0, vehicle_spacing_m: 5.0, \
             vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("builds");
        assert!(sim.step().is_empty());
        assert_eq!(sim.agent_count(), 0);
    }
}
