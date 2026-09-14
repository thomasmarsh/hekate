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
//! nearby body. Increment 3 slice C adds the fixed-time pedestrian signal state
//! of a crossing ([`crate::signal`]) and the contextual pedestrian
//! signal-compliance decision ([`crate::PedestrianComplianceDecision`]) that gates a
//! compliant pedestrian's stopping profile and records a traceable reason.
//! Increment 3 slice D adds vehicle yielding to an occupied crossing: a
//! vehicle obliged by an authored `yield` rule brakes for the crossing it
//! crosses while a pedestrian body overlaps the crossing region, using the
//! shared spatial index ([`crate::index`]) and the shared typed event system
//! ([`Event::Yielded`]). Increment 3 slice E routes both modes through the
//! explicit, replaceable controller interfaces ([`crate::controller`]): the
//! kernel keeps every interaction decision and calls the vehicle longitudinal
//! model and the pedestrian model through their traits, so the IDM and
//! waypoint models are replaceable without editing the interaction logic.
//! Increment 4 slice D adds the online interaction metrics ([`crate::metrics`]):
//! time to collision and minimum surface separation over the swept candidate
//! pairs, and conflict-region post-encroachment time. The pass only reads the
//! integrated tick, so it reports how close the run came to a conflict without
//! changing a trajectory.

use glam::DVec2;
use tangle_model::{
    AgentFamily, CommitPolicySource, CompiledMovement, CompiledPath, CompiledPedestrianRoute,
    CompiledReferencePath, CompiledScenario, CrossingId, DemandId, ModeTemplateId, MovementId,
    PathEnd, PathId, PedestrianDemandId, PedestrianRouteId, PortalId, RuleKind, SignalColor,
    SignalId,
};

use crate::agent::{AgentId, AgentInit, AgentMode, AgentStore, RouteState};
use crate::compliance::{self, ComplianceDecision, ComplianceReason, SignalAction};
use crate::config::RunConfig;
use crate::control::{Constraint, IDM_STANDSTILL_GAP_M};
use crate::controller::{ControllerModelNames, ControllerModels};
use crate::demand::{DemandRuntime, MAX_PENDING_SPAWNS, sample_pedestrian_route, sample_route};
use crate::event::{DespawnReason, Event, ViolationKind};
use crate::index::{self, SpatialIndex};
use crate::metrics::InteractionMetrics;
use crate::narrow::{self, NarrowProfile, sample_narrow_profile};
use crate::pedestrian::{self, Conflict, PedestrianState, PedestrianWaypoint, PedestrianZone};
use crate::pedestrian_compliance::{self, PedestrianComplianceDecision, PedestrianSignalAction};
use crate::prediction::{
    DEFAULT_SUBDIVISIONS, ManeuverInputs, ManeuverPrediction, PredictedBody,
    predict_maneuver_corridor,
};
use crate::profile::{
    PedestrianProfile, VehicleProfile, sample_pedestrian_profile, sample_profile,
};
use crate::query;
use crate::rng::{
    STREAM_COMPLIANCE, STREAM_DEMAND, STREAM_PEDESTRIAN_DEMAND, STREAM_PROFILE, derive_stream,
    uniform01,
};
use crate::safety::SafetyMonitor;
use crate::signal::{self, PedestrianSignalColor, SignalRuntime};
use crate::snapshot::{AgentSample, MotionSample, RouteStateSample, Snapshot, SnapshotDetail};
use crate::stage::{
    AbortCondition, CorridorClaim, LateralManeuverRequest, ManeuverAbortReason, ManeuverCorridor,
    ManeuverEdge, ManeuverState, ManeuverTransition, MotionCommand, MotionControl, Observation,
    PedestrianObservation, PhysicalAdvance, RelevantWorldQuery, SETTLE_TOLERANCE_M, Tactic,
    TacticReason, TacticTarget, TacticalChoice, VehicleObservation, arbitrate_claims,
};
use crate::steering::{
    BoundedSteering, LateralCorridor, SteeringLimits, SteeringRequest, bounded_steering_step,
};
use crate::time::SimTime;
use crate::units::Seconds;

/// Extra clearance in metres demanded beyond two bodies' half-lengths when a
/// portal admits a vehicle.
///
/// Admission is conservative: a vehicle enters only when its entry body clears
/// every live body on the same path. This is a portal rule, not a collision
/// check; exact box queries spanning crossing paths are Increment 4 work.
const MIN_SPAWN_CLEARANCE_M: f64 = 1.0;

/// Minimum metres added around a crossing region's bounding box when querying
/// the shared spatial index for candidate bodies.
///
/// [`Simulation::crossing_query_margin_m`] widens by the scenario's largest
/// authored pedestrian radius and never narrows below this floor. It only
/// widens the candidate query: the exact circle-versus-ring test still decides
/// whether a body actually occupies the region, so a margin keeps the candidate
/// set independent of any one sampled body's size.
const MIN_CROSSING_QUERY_MARGIN_M: f64 = 0.5;

/// Metres short of a crossing entry a yielding vehicle comes to rest.
///
/// The stop point sits behind the crossing region, so the vehicle's whole body
/// stays clear of it and a waiting pedestrian's waypoint is never blocked. The
/// positive margin also keeps the front-bumper gap strictly positive at rest,
/// so a stopped vehicle keeps yielding instead of creeping across the entry.
const YIELD_STOP_MARGIN_M: f64 = 0.5;

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
/// The output borrows the kernel's reused event and transition buffers, so
/// consuming it costs no allocation and transfers no world state.
#[derive(Debug)]
pub struct StepOutput<'a> {
    time: SimTime,
    events: &'a [Event],
    transitions: &'a [ManeuverTransition],
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

    /// The maneuver state transitions the step produced, in a deterministic
    /// order: the attempts of every agent in ascending id order, then the
    /// committed clauses and the settle edges, then the decided claim batch.
    ///
    /// One record is produced per state change, so a later increment can emit
    /// one public event per transition; no event is emitted here.
    pub fn transitions(&self) -> &[ManeuverTransition] {
        self.transitions
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
    /// The replaceable motion models the kernel drives both modes through; see
    /// [`crate::controller`].
    controllers: ControllerModels,
    demand: Vec<DemandRuntime<MovementId>>,
    pedestrian_demand: Vec<DemandRuntime<PedestrianRouteId>>,
    signals: Vec<SignalRuntime>,
    signal_heads: Vec<Option<(SignalId, usize)>>,
    /// Pedestrian signal phase clock of every compiled crossing, indexed by
    /// dense crossing id; `None` for an uncontrolled crossing.
    pedestrian_signals: Vec<Option<SignalRuntime>>,
    /// Waypoints of every compiled pedestrian route, in travel order and
    /// indexed by dense route id.
    waypoints: Vec<Vec<PedestrianWaypoint>>,
    /// Reused per-pedestrian neighbour buffer, so the avoidance scan does not
    /// allocate inside the tick loop.
    conflicts: Vec<Conflict>,
    /// Shared uniform-grid spatial index over live agents of both modes,
    /// rebuilt once at the start of each tick.
    spatial: SpatialIndex,
    /// Per-tick observation of the safety records slice C adds: body contacts,
    /// near misses, region transits, standstill queues, and control
    /// transitions. See [`crate::safety`] for the predicates and the
    /// once-per-transition lifecycle.
    safety: SafetyMonitor,
    /// Online interaction metrics observed from the same integrated tick: time
    /// to collision, minimum surface separation, and conflict-region
    /// post-encroachment time. See [`crate::metrics`] for the definitions. The
    /// pass only borrows state, so it cannot change the simulated trajectory.
    metrics: InteractionMetrics,
    /// Reused candidate buffer for a crossing-occupancy query, so the query
    /// does not allocate inside the tick loop.
    candidates: Vec<AgentId>,
    events: Vec<Event>,
    /// The maneuver state transitions this step produced, in a deterministic
    /// order. Cleared at the start of every tick, like [`Self::events`], and
    /// exposed on [`StepOutput`] for the later event surface to emit; no public
    /// event is produced here.
    transitions: Vec<ManeuverTransition>,
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
        let pedestrian_signals: Vec<Option<SignalRuntime>> = scenario
            .crossings()
            .iter()
            .map(|crossing| {
                crossing
                    .pedestrian_signal()
                    .map(|signal| SignalRuntime::new(signal.cycle_s()))
            })
            .collect();
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
                    narrow_profile: None,
                    pedestrian_route: None,
                    pedestrian_profile: None,
                    // The walking skeleton is a legacy version-1 path population,
                    // so it carries no route coordinates.
                    route_state: None,
                });
            }
            spawned_total = u64::from(population.vehicle_count);
        }

        Ok(Self {
            scenario,
            config,
            tick: 0,
            agents,
            controllers: ControllerModels::initial(),
            demand,
            pedestrian_demand,
            signals,
            signal_heads,
            pedestrian_signals,
            waypoints,
            conflicts: Vec::new(),
            spatial: SpatialIndex::default(),
            safety: SafetyMonitor::default(),
            metrics: InteractionMetrics::default(),
            candidates: Vec::new(),
            events: Vec::new(),
            transitions: Vec::new(),
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
            transitions: &self.transitions,
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
                        body_kind: self.agents.body_kind[index],
                        // A Phase 1 body is a single box or circle envelope, so
                        // it carries no ordered segments yet.
                        segments: Vec::new(),
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
                        pedestrian_decision: self.agents.pedestrian_decision[index],
                        yield_crossing: self.agents.yield_crossing[index],
                        route_state: self.agents.route_state[index].map(|state| RouteStateSample {
                            s_m: state.s_m,
                            d_m: state.d_m,
                            maneuver_state: state.maneuver,
                            target_offset_m: state.target_offset_m,
                            target_facility: state.target_facility,
                            predicted_min_clearance_m: state.predicted_min_clearance_m,
                            target_clearance_m: state.target_clearance_m,
                            horizon_s: state.horizon_s,
                        }),
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

    /// Sampled narrow wheeled profile of a narrow mode agent.
    ///
    /// Present exactly for a narrow wheeled agent (a capsule that steers),
    /// carrying its narrow-specific steering and lateral-clearance parameters;
    /// `None` for a passenger car, the scripted population, and a pedestrian.
    /// The shared longitudinal parameters are the same agent's
    /// [`Simulation::agent_profile`].
    pub fn agent_narrow_profile(&self, agent: AgentId) -> Option<narrow::NarrowProfile> {
        self.agents
            .narrow_profile
            .get(agent.index())
            .copied()
            .flatten()
    }

    /// Sampled longitudinal profile of an agent, present for demand-generated
    /// vehicles and narrow modes.
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

    /// Most recent pedestrian signal-compliance decision of an agent.
    ///
    /// Present for a pedestrian on a route that reaches a signal-controlled
    /// crossing, including a walk proceed, so every state-affecting pedestrian
    /// signal decision is traceable. `None` for a pedestrian with no upcoming
    /// signal-controlled crossing.
    pub fn agent_pedestrian_decision(
        &self,
        agent: AgentId,
    ) -> Option<PedestrianComplianceDecision> {
        self.agents
            .pedestrian_decision
            .get(agent.index())
            .copied()
            .flatten()
    }

    /// Crossing a live vehicle is currently yielding to.
    ///
    /// Present only while the vehicle is yielding: its movement is obliged by a
    /// `yield` rule, the crossing it crosses is occupied by a pedestrian, and
    /// its front bumper is still upstream of the crossing entry. `None`
    /// otherwise, including once the crossing clears.
    pub fn agent_yield_crossing(&self, agent: AgentId) -> Option<CrossingId> {
        self.agents
            .yield_crossing
            .get(agent.index())
            .copied()
            .flatten()
    }

    /// Current pedestrian signal state of a crossing.
    ///
    /// `None` when the crossing is uncontrolled (carries no pedestrian signal).
    /// This is the observable seam for the crossing's fixed-time pedestrian
    /// signal; it reports the walk state only and makes no compliance decision.
    pub fn crossing_signal(&self, crossing: CrossingId) -> Option<PedestrianSignalColor> {
        let signal = self.scenario.crossing(crossing)?.pedestrian_signal()?;
        let elapsed = self
            .pedestrian_signals
            .get(crossing.index())
            .and_then(Option::as_ref)
            .map_or(0.0, |runtime| runtime.elapsed_s());
        signal::pedestrian_walk_at(signal, elapsed)
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

    /// Names of the motion models this run drives.
    ///
    /// The kernel reaches the vehicle longitudinal model, the narrow wheeled
    /// longitudinal model, and the pedestrian model through the interfaces in
    /// [`crate::controller`], so this reports the model identities a trace was
    /// produced with.
    pub fn controller_models(&self) -> ControllerModelNames {
        self.controllers.names()
    }

    /// Online interaction metrics for the run so far.
    ///
    /// Time to collision, minimum surface separation, and conflict-region
    /// post-encroachment time, observed once per tick from the state the tick
    /// integrated. See [`crate::metrics`] for the definitions, the candidate
    /// set, and the declared tolerances.
    pub fn interaction_metrics(&self) -> &InteractionMetrics {
        &self.metrics
    }

    /// Install replacement motion models.
    ///
    /// The kernel calls both modes through [`crate::controller::ControllerModels`],
    /// so a unit test can swap a model and show that the interaction logic is
    /// untouched. Production code chooses its models in
    /// [`crate::controller::ControllerModels::initial`].
    #[cfg(test)]
    pub(crate) fn set_controller_models(&mut self, models: ControllerModels) {
        self.controllers = models;
    }

    fn announce_initial_population(&mut self) {
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            self.events.push(Event::Spawned {
                agent: AgentId::from_index(index),
                mode: self.agents.mode[index],
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
        for runtime in self.pedestrian_signals.iter_mut().flatten() {
            runtime.advance(dt);
        }

        // Rebuild the shared spatial index once, before any body moves, so
        // every crossing-occupancy query this tick sees one consistent
        // candidate view that does not depend on agent iteration order. The
        // safety monitor records the tick-start bodies here too, so its swept
        // bodies span the whole tick.
        self.spatial.rebuild(&self.agents);
        self.safety.begin_tick(&self.agents);
        self.metrics.begin_tick(&self.agents);

        // The maneuver pass runs before any agent command, so every claim this
        // step is collected from one immutable observation of the tick-start
        // state and arbitrated as a batch: no claim can depend on the order the
        // agents are commanded in, or on a body a later step has already moved.
        // It is inert unless a maneuver is in flight or an intent is recorded.
        self.resolve_maneuvers(dt);

        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            self.step_agent(index, dt);
        }

        // Observe the integrated tick once, before new demand is admitted, so
        // every safety record describes exactly the state this tick produced.
        self.safety
            .observe(&self.agents, &self.scenario, &mut self.events);

        // The online interaction metrics observe the same state, for the same
        // reason: the same live bodies the tick integrated, and the region edges
        // the safety records just added. The pass borrows the state and the
        // events and writes only its own records, so it cannot change the
        // trajectory, a trace, or a golden.
        self.metrics
            .observe(&self.agents, self.tick, self.config, &self.events);

        self.advance_demand(dt);

        // The documented within-tick order: ascending agent, then kind, then the
        // variant's stable key. Sorting here, after every emission point, makes
        // the order a property of the records and not of where they were
        // produced. The sort is stable, so only identical records can tie; see
        // [`Event::order_key`].
        self.events.sort_by_key(|event| event.order_key());
    }

    /// Advance one agent through the four explicit controller stages.
    ///
    /// The kernel drives every live agent through the four stages in order —
    /// relevant-world query, tactical choice, motion control, and physical
    /// advance — so the Phase 1 vehicle update and the Phase 1 pedestrian
    /// update share the same boundaries and the motion model is reached only
    /// through [`crate::controller`]. See [`crate::stage`] for the interfaces
    /// and the interaction decisions the kernel keeps.
    fn step_agent(&mut self, index: usize, dt: f64) {
        let observation = self.query_world(index);
        let tactic = self.choose_tactic(index, &observation);
        let command = self.command_motion(index, &observation, &tactic, dt);
        self.advance_physics(index, observation, &command, dt);
    }

    /// Record one agent's lateral intent for the following decisions.
    ///
    /// This is the seam the tactical leaves supply: a leaf that decides an agent
    /// should displace laterally — a pass, a lane change, a positioning tactic —
    /// records the target and the passed obstacle here, and the kernel's
    /// maneuver pass performs the whole lifecycle from the guard checks on. The
    /// kernel decides whether the attempt is admissible, so a requester supplies
    /// only the target and the obstacle it displaces around.
    ///
    /// Returns `false`, and records nothing, when the agent can never perform a
    /// lateral maneuver: it is not alive, the passed obstacle is not another
    /// live body, it carries no route state or no bounded-steering envelope, its
    /// mode declares no lateral policy, or the scenario authors no
    /// `maneuver_policy.commit` to commit under.
    pub fn request_lateral_maneuver(
        &mut self,
        agent: AgentId,
        request: LateralManeuverRequest,
    ) -> bool {
        if self.scenario.commit_policy().is_none() {
            return false;
        }
        let index = agent.index();
        let passed = request.passed_body.index();
        if index >= self.agents.len()
            || !self.agents.alive[index]
            || request.passed_body == agent
            || passed >= self.agents.len()
            || !self.agents.alive[passed]
            || !request.target_offset_m.is_finite()
        {
            return false;
        }
        let Some(state) = self.agents.route_state[index] else {
            return false;
        };
        if state.bounded_steering.is_none()
            || state.target_clearance_m.is_none()
            || state.horizon_s.is_none()
        {
            return false;
        }
        self.agents.route_state[index] = Some(RouteState {
            intent: Some(request),
            ..state
        });
        true
    }

    /// The agent's own maneuver lifecycle state, or [`ManeuverState::Following`]
    /// for an agent that carries no route state.
    fn agent_maneuver_state(&self, index: usize) -> ManeuverState {
        self.agents
            .route_state
            .get(index)
            .copied()
            .flatten()
            .map_or(ManeuverState::Following, |state| state.maneuver)
    }

    /// The kernel's maneuver lifecycle pass: fix targets, arbitrate this step's
    /// corridor claims as a batch, and record every state transition.
    ///
    /// It runs once at the start of a tick, before any agent command is
    /// produced. Every decision reads one immutable observation — a copy of each
    /// agent's route state and the tick-start bodies — and the decided state is
    /// written back only after the whole batch, so no decision reads a state
    /// another decision wrote and no claim depends on agent iteration order.
    ///
    /// The pass is inert unless an agent holds a maneuver in flight or has
    /// recorded an intent, and a scenario that authors no
    /// `maneuver_policy.commit` has no maneuver machinery at all, so a run with
    /// no Increment 2 maneuver behaves exactly as Increment 1.
    ///
    /// Transitions are recorded in phase order — the attempts of every agent in
    /// ascending id order, then the committed and settle clauses, then the
    /// decided claim batch — so the record order is a property of the decisions
    /// and not of where they were produced.
    fn resolve_maneuvers(&mut self, dt: f64) {
        self.transitions.clear();
        let Some(commit) = self.scenario.commit_policy() else {
            return;
        };
        if !self.maneuver_in_flight() {
            return;
        }

        let now = self.time();
        let bodies = self.predicted_bodies();
        let batch = ManeuverBatch {
            bodies: &bodies,
            commit: &commit,
            now,
            dt,
        };
        let mut plans: Vec<ManeuverPlan> = Vec::new();
        let mut claims: Vec<CorridorClaim> = Vec::new();

        // Phase 1, `following -> preparing`: an intent whose guards hold fixes
        // the target and the candidate corridor. An admitted intent is consumed;
        // a corridor that is merely not feasible yet keeps the intent, so the
        // agent retries at the next decision; an intent whose passed obstacle
        // has disappeared is discarded with no transition, because the agent
        // never prepared anything.
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            let Some(state) = self.agents.route_state[index] else {
                continue;
            };
            if state.maneuver != ManeuverState::Following {
                continue;
            }
            let Some(request) = state.intent else {
                continue;
            };
            match self.attempt_maneuver(index, &state, &request, &batch) {
                Attempt::Admissible {
                    corridor,
                    predicted_clearance_m,
                } => plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        maneuver: ManeuverState::Preparing,
                        target_offset_m: Some(request.target_offset_m),
                        passed_body: Some(request.passed_body),
                        corridor: Some(corridor),
                        predicted_min_clearance_m: Some(predicted_clearance_m),
                        pre_maneuver_offset_m: state.d_m,
                        state_since: Some(now),
                        intent: None,
                        braking: false,
                        hold_since: None,
                        settled_since: None,
                        ..state
                    },
                    transition: Some(ManeuverTransition {
                        agent: AgentId::from_index(index),
                        from: ManeuverState::Following,
                        to: ManeuverState::Preparing,
                        edge: ManeuverEdge::Attempted,
                        reason: None,
                        time: now,
                    }),
                }),
                Attempt::TargetLost => plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        intent: None,
                        ..state
                    },
                    transition: None,
                }),
                Attempt::Infeasible => {}
            }
        }

        // Phase 2: `preparing` seeks its claim at the decision after the
        // attempt and aborts on a lost target, a timeout, or an infeasible
        // corridor; `committed` keeps its corridor as an unbeatable claim and
        // applies the commitment-loss policy. Both read the tick-start state.
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            let Some(state) = self.agents.route_state[index] else {
                continue;
            };
            let agent = AgentId::from_index(index);
            // Only the attempt puts an agent into a maneuver state, and it
            // requires the mode's resolved target clearance, so a live maneuver
            // always carries one.
            let target_clearance_m = state
                .target_clearance_m
                .expect("a maneuver state carries its mode's target clearance");
            match state.maneuver {
                ManeuverState::Preparing => {
                    // A maneuver prepares for one decision and claims at the next,
                    // so a claim never depends on a sibling attempt made in the
                    // same batch.
                    let due = state
                        .state_since
                        .is_some_and(|since| since.tick() < now.tick());
                    if !due {
                        continue;
                    }
                    let Some(corridor) = state.corridor else {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::CorridorInfeasible,
                            now,
                        ));
                        continue;
                    };
                    let Some(passed) = state.passed_body else {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::TargetLost,
                            now,
                        ));
                        continue;
                    };
                    if !self.agents.alive[passed.index()] {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::TargetLost,
                            now,
                        ));
                        continue;
                    }
                    if hold_elapsed(now, state.state_since, commit.hold_timeout_s, dt) {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::HoldTimeout,
                            now,
                        ));
                        continue;
                    }
                    let Some(target_offset_m) = state.target_offset_m else {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::CorridorInfeasible,
                            now,
                        ));
                        continue;
                    };
                    if !self
                        .predict_maneuver(index, target_offset_m, &batch)
                        .is_some_and(|prediction| prediction.is_feasible())
                    {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::CorridorInfeasible,
                            now,
                        ));
                        continue;
                    }
                    claims.push(CorridorClaim {
                        agent,
                        corridor,
                        entry_distance_m: CorridorClaim::entry_distance_m(
                            &corridor,
                            state.s_m,
                            self.agents.direction[index],
                        ),
                        committed: false,
                        target_clearance_m,
                    });
                }
                ManeuverState::Committed => {
                    let Some(corridor) = state.corridor else {
                        plans.push(self.abort_plan(
                            index,
                            state,
                            ManeuverAbortReason::CorridorInfeasible,
                            now,
                        ));
                        continue;
                    };
                    // A committed claimant stays in the batch whatever its own
                    // decision, so a maneuver that loses clearance this step
                    // never hands its corridor to another claimant mid-step:
                    // that claim is re-arbitrated at the next decision. Its own
                    // outcome never changes its state, because an existing
                    // commitment is never revoked.
                    claims.push(CorridorClaim {
                        agent,
                        corridor,
                        entry_distance_m: CorridorClaim::entry_distance_m(
                            &corridor,
                            state.s_m,
                            self.agents.direction[index],
                        ),
                        committed: true,
                        target_clearance_m,
                    });
                    plans.push(self.committed_plan(index, state, target_clearance_m, &batch));
                }
                ManeuverState::Following | ManeuverState::Returning | ManeuverState::Aborted => {}
            }
        }

        // Phase 3: the settle edges. `returning` and `aborted` steer back to the
        // offset the agent held before the attempt — a returning target that is
        // always inside the usable corridor, because the agent occupied it — and
        // only return to `following` once that offset has held for a decision.
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            let Some(state) = self.agents.route_state[index] else {
                continue;
            };
            let edge = match state.maneuver {
                ManeuverState::Returning => ManeuverEdge::Completed,
                ManeuverState::Aborted => ManeuverEdge::Aborted,
                ManeuverState::Following | ManeuverState::Preparing | ManeuverState::Committed => {
                    continue;
                }
            };
            let at_target = (state.d_m - state.pre_maneuver_offset_m).abs() <= SETTLE_TOLERANCE_M;
            if !at_target {
                plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        settled_since: None,
                        ..state
                    },
                    transition: None,
                });
                continue;
            }
            let since = state.settled_since.unwrap_or(now);
            if since.tick() >= now.tick() {
                plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        settled_since: Some(since),
                        ..state
                    },
                    transition: None,
                });
                continue;
            }
            plans.push(ManeuverPlan {
                index,
                state: RouteState {
                    maneuver: ManeuverState::Following,
                    target_offset_m: None,
                    target_facility: None,
                    predicted_min_clearance_m: None,
                    corridor: None,
                    passed_body: None,
                    state_since: None,
                    braking: false,
                    hold_since: None,
                    settled_since: None,
                    ..state
                },
                transition: Some(ManeuverTransition {
                    agent: AgentId::from_index(index),
                    from: state.maneuver,
                    to: ManeuverState::Following,
                    edge,
                    reason: None,
                    time: now,
                }),
            });
        }

        // Phase 4: the batch. Claims are arbitrated once, and only the decided
        // outcomes move a `preparing` maneuver; a claim that loses aborts with
        // the documented reason.
        for outcome in arbitrate_claims(&claims) {
            let index = outcome.agent.index();
            let Some(state) = self.agents.route_state[index] else {
                continue;
            };
            if state.maneuver != ManeuverState::Preparing {
                continue;
            }
            if outcome.granted {
                plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        maneuver: ManeuverState::Committed,
                        state_since: Some(now),
                        braking: false,
                        hold_since: None,
                        ..state
                    },
                    transition: Some(ManeuverTransition {
                        agent: outcome.agent,
                        from: ManeuverState::Preparing,
                        to: ManeuverState::Committed,
                        edge: ManeuverEdge::Committed,
                        reason: None,
                        time: now,
                    }),
                });
            } else {
                plans.push(self.abort_plan(index, state, ManeuverAbortReason::ClaimRejected, now));
            }
        }

        // Apply the batch. Nothing above read a state another decision wrote, so
        // the result is a pure function of the tick-start observation and the
        // batch, and every decided transition is recorded exactly once.
        for plan in plans {
            self.agents.route_state[plan.index] = Some(plan.state);
            if let Some(transition) = plan.transition {
                self.transitions.push(transition);
            }
        }
    }

    /// Whether any live agent holds a maneuver or a recorded intent, so the
    /// maneuver pass has a decision to make.
    fn maneuver_in_flight(&self) -> bool {
        (0..self.agents.len()).any(|index| {
            self.agents.alive[index]
                && self.agents.route_state[index].is_some_and(|state| {
                    state.maneuver != ManeuverState::Following || state.intent.is_some()
                })
        })
    }

    /// The tick-start bodies every prediction of this step reads, in ascending
    /// [`AgentId`] order.
    ///
    /// A wheeled body faces its reference tangent, so its travel sign turns that
    /// facing into the direction of travel; a walking body's heading is already
    /// its travel heading. Each velocity is held constant over the horizon, as
    /// [`PredictedBody`] documents.
    fn predicted_bodies(&self) -> Vec<PredictedBody> {
        (0..self.agents.len())
            .filter(|&index| self.agents.alive[index])
            .map(|index| {
                let travel_sign = match self.agents.mode[index] {
                    AgentMode::Vehicle => self.agents.direction[index],
                    AgentMode::Pedestrian => 1.0,
                };
                PredictedBody {
                    id: AgentId::from_index(index),
                    shape: query::agent_body(&self.agents, index),
                    velocity_mps: DVec2::from_angle(self.agents.heading_rad[index])
                        * (self.agents.speed_mps[index] * travel_sign),
                }
            })
            .collect()
    }

    /// Predict one agent's candidate maneuver corridor and clearances from the
    /// tick-start state, or `None` when the agent carries no bounded-steering
    /// envelope or no compiled facility reference.
    fn predict_maneuver(
        &self,
        index: usize,
        target_offset_m: f64,
        batch: &ManeuverBatch<'_>,
    ) -> Option<ManeuverPrediction> {
        let state = self.agents.route_state[index]?;
        let steering = state.bounded_steering?;
        let geometry = self.route_geometry(index)?;
        let facility = self.scenario.facility(state.facility)?;
        // The predicted bodies are the *other* bodies: the agent's own envelope
        // is the candidate motion's start, never one of its obstacles.
        let agent = AgentId::from_index(index);
        let others: Vec<PredictedBody> = batch
            .bodies
            .iter()
            .copied()
            .filter(|body| body.id != agent)
            .collect();
        Some(predict_maneuver_corridor(
            ManeuverInputs {
                geometry,
                facility_width_m: facility.width_m(),
                direction: self.agents.direction[index],
                body: query::agent_body(&self.agents, index),
                speed_mps: self.agents.speed_mps[index],
                target_offset_m,
                steering,
                target_clearance_m: state.target_clearance_m?,
                horizon_s: state.horizon_s?,
                // This leaf's decision cadence is the fixed step: the prediction
                // subdivides one decision of motion. The manifest's own
                // lateral-decision cadence arrives with the fidelity settings.
                cadence_s: batch.dt,
                subdivisions: DEFAULT_SUBDIVISIONS,
            },
            &others,
        ))
    }

    /// Fix one agent's lateral target and candidate corridor, or report why the
    /// attempt is not admissible this step.
    fn attempt_maneuver(
        &self,
        index: usize,
        state: &RouteState,
        request: &LateralManeuverRequest,
        batch: &ManeuverBatch<'_>,
    ) -> Attempt {
        let passed = request.passed_body.index();
        if passed >= self.agents.len() || !self.agents.alive[passed] {
            return Attempt::TargetLost;
        }
        let Some(prediction) = self.predict_maneuver(index, request.target_offset_m, batch) else {
            return Attempt::Infeasible;
        };
        // The candidate corridor exists only when the predicted motion is
        // feasible: the target is inside the usable corridor and the bounded
        // motion never leaves it, which is the prediction's own verdict.
        if !prediction.is_feasible() {
            return Attempt::Infeasible;
        }
        let Some(geometry) = self.route_geometry(index) else {
            return Attempt::Infeasible;
        };
        let direction = self.agents.direction[index];
        // `predict_maneuver` above already resolved the mode's policy, so both
        // values exist here.
        let Some(target_clearance_m) = state.target_clearance_m else {
            return Attempt::Infeasible;
        };
        // The claimed corridor is the space the maneuver needs: the passed
        // obstacle's footprint plus both bodies' half lengths and the target
        // clearance along the reference, and the predicted corridor's lateral
        // band in the facility's own frame, widened by the same clearance.
        let mirror = if direction < 0.0 { -1.0 } else { 1.0 };
        let mut d_min_m = f64::INFINITY;
        let mut d_max_m = f64::NEG_INFINITY;
        for sample in &prediction.corridor {
            let d_m = sample.d_m * mirror;
            d_min_m = d_min_m.min(d_m);
            d_max_m = d_max_m.max(d_m);
        }
        let passed_s_m = geometry.project(self.agents.position[passed]).s();
        let half_self_m = self.agents.body_length_m[index] * 0.5;
        let half_passed_m = self.agents.body_length_m[passed] * 0.5;
        let corridor = ManeuverCorridor {
            facility: state.facility,
            s_min_m: passed_s_m - half_passed_m - half_self_m - target_clearance_m,
            s_max_m: passed_s_m + half_passed_m + half_self_m + target_clearance_m,
            d_min_m: d_min_m - target_clearance_m,
            d_max_m: d_max_m + target_clearance_m,
        };
        Attempt::Admissible {
            corridor,
            predicted_clearance_m: prediction.clears.swept.clearance_m,
        }
    }

    /// The planned update for one committed agent: the completion edge when the
    /// passed obstacle is cleared, the documented brake/hold/abort response when
    /// predicted clearance is lost, or no change at all.
    fn committed_plan(
        &self,
        index: usize,
        state: RouteState,
        target_clearance_m: f64,
        batch: &ManeuverBatch<'_>,
    ) -> ManeuverPlan {
        let Some(passed) = state.passed_body else {
            return self.abort_plan(index, state, ManeuverAbortReason::TargetLost, batch.now);
        };
        if passed.index() >= self.agents.len() || !self.agents.alive[passed.index()] {
            return self.abort_plan(index, state, ManeuverAbortReason::TargetLost, batch.now);
        }
        let Some(target_offset_m) = state.target_offset_m else {
            return self.abort_plan(
                index,
                state,
                ManeuverAbortReason::CorridorInfeasible,
                batch.now,
            );
        };
        let Some(prediction) = self.predict_maneuver(index, target_offset_m, batch) else {
            return self.abort_plan(
                index,
                state,
                ManeuverAbortReason::CorridorInfeasible,
                batch.now,
            );
        };
        let swept_m = prediction.clears.swept.clearance_m;

        if self.passed_body_cleared(index, passed, target_clearance_m) {
            return ManeuverPlan {
                index,
                state: RouteState {
                    maneuver: ManeuverState::Returning,
                    predicted_min_clearance_m: Some(swept_m),
                    state_since: Some(batch.now),
                    braking: false,
                    hold_since: None,
                    settled_since: None,
                    ..state
                },
                transition: Some(ManeuverTransition {
                    agent: AgentId::from_index(index),
                    from: ManeuverState::Committed,
                    to: ManeuverState::Returning,
                    edge: ManeuverEdge::Completed,
                    reason: None,
                    time: batch.now,
                }),
            };
        }

        // The ordered unsafe-commit response: a corridor below the policy's
        // minimum aborts; anything below the target clearance brakes within the
        // comfortable braking and holds, at most for the hold timeout; anything
        // at or above the target continues and clears the hold.
        if swept_m < batch.commit.min_predicted_clearance_m {
            return self.abort_plan(index, state, ManeuverAbortReason::ClearanceLost, batch.now);
        }
        if !prediction.is_feasible() || swept_m <= target_clearance_m {
            let since = state.hold_since.unwrap_or(batch.now);
            if hold_elapsed(
                batch.now,
                Some(since),
                batch.commit.hold_timeout_s,
                batch.dt,
            ) {
                return self.abort_plan(index, state, ManeuverAbortReason::HoldTimeout, batch.now);
            }
            return ManeuverPlan {
                index,
                state: RouteState {
                    predicted_min_clearance_m: Some(swept_m),
                    braking: true,
                    hold_since: Some(since),
                    ..state
                },
                transition: None,
            };
        }
        ManeuverPlan {
            index,
            state: RouteState {
                predicted_min_clearance_m: Some(swept_m),
                braking: false,
                hold_since: None,
                ..state
            },
            transition: None,
        }
    }

    /// Whether the agent's rear envelope point is at least `target_clearance_m`
    /// ahead of the passed body's front envelope along its travel direction, so
    /// the passed obstacle is cleared.
    fn passed_body_cleared(&self, index: usize, passed: AgentId, target_clearance_m: f64) -> bool {
        let Some(geometry) = self.route_geometry(index) else {
            return false;
        };
        let direction = self.agents.direction[index];
        let agent_s_m = geometry.project(self.agents.position[index]).s();
        let passed_s_m = geometry.project(self.agents.position[passed.index()]).s();
        let rear_m = agent_s_m - direction * self.agents.body_length_m[index] * 0.5;
        let front_m = passed_s_m + direction * self.agents.body_length_m[passed.index()] * 0.5;
        direction * (rear_m - front_m) >= target_clearance_m
    }

    /// The planned `-> aborted` edge for one agent: the maneuver ends without
    /// reaching its target, and the agent steers back to the offset it held
    /// before the attempt.
    fn abort_plan(
        &self,
        index: usize,
        state: RouteState,
        reason: ManeuverAbortReason,
        now: SimTime,
    ) -> ManeuverPlan {
        ManeuverPlan {
            index,
            state: RouteState {
                maneuver: ManeuverState::Aborted,
                state_since: Some(now),
                braking: false,
                hold_since: None,
                settled_since: None,
                ..state
            },
            transition: Some(ManeuverTransition {
                agent: AgentId::from_index(index),
                from: state.maneuver,
                to: ManeuverState::Aborted,
                edge: ManeuverEdge::Aborted,
                reason: Some(reason),
                time: now,
            }),
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

    /// The pedestrian signal-compliance decision for the upcoming crossing.
    ///
    /// Returns `None` when the route reaches no signal-controlled crossing at or
    /// after the cursor. The gap is the route distance from the pedestrian's
    /// current progress to the crossing's stop point, positive while upstream.
    fn pedestrian_signal_decision(
        &self,
        index: usize,
        route_id: PedestrianRouteId,
        cursor: usize,
        progress_m: f64,
    ) -> Option<PedestrianComplianceDecision> {
        let direction = self.agents.direction[index];
        let (crossing, stop_progress_m) =
            self.next_crossing_waypoint(route_id, cursor, direction)?;
        let signal = self.crossing_signal(crossing)?;
        let gap_m = stop_progress_m - progress_m;
        Some(pedestrian_compliance::decide(
            signal,
            gap_m,
            self.agents.speed_mps[index],
            self.agents.pedestrian_profile[index]?.compliance,
            pedestrian::MAX_DECEL_MPS2,
        ))
    }

    /// The first crossing waypoint at or after `cursor`, as its dense id and
    /// route progress.
    ///
    /// The cursor only advances past a crossing once the pedestrian has reached
    /// it, so the first crossing at or after the cursor is always the upcoming
    /// one, whatever the target waypoint (a waiting area or the crossing).
    fn next_crossing_waypoint(
        &self,
        route_id: PedestrianRouteId,
        cursor: usize,
        direction: f64,
    ) -> Option<(CrossingId, f64)> {
        let path_length_m = self
            .scenario
            .pedestrian_route(route_id)
            .and_then(|route| self.scenario.path(route.path()))
            .map_or(0.0, |path| path.length());
        self.route_waypoints(route_id)
            .iter()
            .skip(cursor)
            .find_map(|waypoint| match waypoint.zone() {
                Some(PedestrianZone::Crossing(crossing)) => Some((
                    crossing,
                    pedestrian::waypoint_progress_m(*waypoint, direction, path_length_m),
                )),
                _ => None,
            })
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

    /// Whether an authored `yield` rule obliges a movement to yield to the
    /// crossings it crosses.
    ///
    /// This is the shared rule representation: a crossing names the movements
    /// it crosses ([`tangle_model::CompiledCrossing::movements`]) and a
    /// movement's [`RuleKind::Yield`] rule is the obligation to yield to them.
    /// A movement with no yield rule is unaffected, so yielding is authored
    /// scenario data rather than a simulator branch, and adding this rule never
    /// changes a scenario that omits it.
    fn movement_yields(&self, movement: MovementId) -> bool {
        self.scenario
            .rules()
            .iter()
            .any(|rule| rule.movement() == movement && rule.kind() == RuleKind::Yield)
    }

    /// Recompute the crossing a vehicle yields to and emit any transition.
    ///
    /// A vehicle yields to the next crossing its movement crosses, only while
    /// that crossing is occupied by a pedestrian and its front bumper is still
    /// upstream of the crossing entry. The recorded state is the crossing
    /// currently yielded to; a change emits [`Event::Yielded`] for the crossing
    /// the yield ended on and then the one it began on, so every consumer sees
    /// the transition through the shared event stream.
    fn update_yield_state(&mut self, index: usize) {
        let previous = self.agents.yield_crossing[index];
        let mut desired = None;
        if let Some(movement) = self.agents.movement[index]
            && self.movement_yields(movement)
            && let Some(crossing) = self.next_crossing(index, movement)
            && self.crossing_occupied(crossing)
        {
            desired = Some(crossing);
        }
        if desired == previous {
            return;
        }
        let agent = AgentId::from_index(index);
        if let Some(previous) = previous {
            self.events.push(Event::Yielded {
                agent,
                crossing: previous,
                yielding: false,
            });
        }
        if let Some(desired) = desired {
            self.events.push(Event::Yielded {
                agent,
                crossing: desired,
                yielding: true,
            });
        }
        self.agents.yield_crossing[index] = desired;
    }

    /// The next crossing across a movement's path whose entry the vehicle's
    /// front bumper has not reached, preferring the one it reaches first.
    ///
    /// A crossing whose entry the front bumper has reached is behind the
    /// vehicle's yield point, so the vehicle is committed to it and continues
    /// under ordinary IDM control rather than stopping in the crossing. The
    /// stop point sits [`YIELD_STOP_MARGIN_M`] short of the entry, so a stopped
    /// vehicle holds clear of the region and keeps its yield while the crossing
    /// stays occupied.
    fn next_crossing(&self, index: usize, movement: MovementId) -> Option<CrossingId> {
        let path = self.scenario.path(self.agents.path[index])?;
        let direction = self.agents.direction[index];
        let own_front =
            direction * self.agents.distance_m[index] + self.agents.body_length_m[index] * 0.5;
        let mut best: Option<(CrossingId, f64)> = None;
        for crossing in self.scenario.crossings() {
            if !crossing.movements().contains(&movement) {
                continue;
            }
            let Some(entry) = self.crossing_entry_progress(crossing.id(), path, direction) else {
                continue;
            };
            if own_front >= entry {
                continue;
            }
            match best {
                Some((_, best_entry)) if best_entry <= entry => {}
                _ => best = Some((crossing.id(), entry)),
            }
        }
        best.map(|(crossing, _)| crossing)
    }

    /// Directional path progress (`direction * arc`) of a crossing region's
    /// entry along a vehicle path: the least progress among the region's ring
    /// vertices projected onto the path.
    ///
    /// That is the first point of the region the vehicle reaches. The progress
    /// uses the same `direction`-signed convention as [`Self::signal_decision`]
    /// and [`Self::nearest_leader`], so a vehicle front
    /// (`direction * distance_m + body_length/2`) and this entry are directly
    /// comparable in either travel direction; mixing in the pedestrian helper's
    /// travel-progress convention would offset a backward movement by the whole
    /// path length. Using the crossing's shared authored region keeps its
    /// geometry and its vehicle rule in one representation.
    fn crossing_entry_progress(
        &self,
        crossing: CrossingId,
        path: &CompiledPath,
        direction: f64,
    ) -> Option<f64> {
        let region = self.scenario.crossing(crossing)?.region();
        let ring = self.scenario.region(region)?.polygon().ring();
        let mut entry: Option<f64> = None;
        for &vertex in ring {
            let progress = direction * pedestrian::closest_arc(path, vertex);
            entry = Some(entry.map_or(progress, |current| current.min(progress)));
        }
        entry
    }

    /// Candidate-query margin in metres around a crossing region's bounding box.
    ///
    /// The widest body the occupancy test must consider is a pedestrian circle
    /// of the scenario's largest authored radius, so the margin is that radius:
    /// a larger body can reach into the region from farther outside the box. The
    /// floor covers a non-finite or degenerate authored range. This keeps the
    /// candidate set permissive for any sampled body rather than trusting a
    /// fixed constant against an unbounded authored radius.
    fn crossing_query_margin_m(&self) -> f64 {
        let radius_m = self.scenario.pedestrian_profiles().radius_m().max();
        if radius_m.is_finite() && radius_m > MIN_CROSSING_QUERY_MARGIN_M {
            radius_m
        } else {
            MIN_CROSSING_QUERY_MARGIN_M
        }
    }

    /// Stop constraint for a vehicle yielding to an occupied crossing.
    ///
    /// The constraint holds the front bumper at [`YIELD_STOP_MARGIN_M`] short
    /// of the crossing entry, exactly as a stop line holds the bumper at the
    /// authored line: IDM sees a stationary constraint with a zero standstill
    /// gap, and the kernel adds a position cap. `None` when the vehicle is not
    /// yielding.
    fn crossing_yield_constraint(&self, index: usize) -> Option<(CrossingId, Constraint)> {
        let crossing = self.agents.yield_crossing[index]?;
        let path = self.scenario.path(self.agents.path[index])?;
        let direction = self.agents.direction[index];
        let own_front =
            direction * self.agents.distance_m[index] + self.agents.body_length_m[index] * 0.5;
        let entry = self.crossing_entry_progress(crossing, path, direction)?;
        Some((
            crossing,
            Constraint {
                gap_m: (entry - YIELD_STOP_MARGIN_M - own_front).max(0.0),
                speed_mps: 0.0,
                standstill_m: 0.0,
            },
        ))
    }

    /// Whether any live pedestrian body overlaps a crossing region.
    ///
    /// Candidates come from the shared spatial index, so both modes share one
    /// candidate query; the exact circle-versus-ring test then decides overlap.
    /// Candidates are visited in ascending agent id order, the documented
    /// tie-break, and the scan stops at the first overlap.
    fn crossing_occupied(&mut self, crossing: CrossingId) -> bool {
        let Some(region) = self
            .scenario
            .crossing(crossing)
            .map(|crossing| crossing.region())
        else {
            return false;
        };
        let Some(ring) = self
            .scenario
            .region(region)
            .map(|region| region.polygon().ring())
        else {
            return false;
        };
        let bounds = bounding_box(ring);
        let margin = DVec2::splat(self.crossing_query_margin_m());
        self.spatial
            .candidates_in_aabb(bounds.0 - margin, bounds.1 + margin, &mut self.candidates);
        for candidate in &self.candidates {
            let index = candidate.index();
            if self.agents.mode[index] != AgentMode::Pedestrian {
                continue;
            }
            let radius_m = self.agents.body_length_m[index] * 0.5;
            if index::circle_overlaps_ring(ring, self.agents.position[index], radius_m) {
                return true;
            }
        }
        false
    }

    /// Nearest live leader ahead on the same path travelling the same way.
    ///
    /// The gap is bumper to bumper along the path. Iterating in ascending agent
    /// order means the lowest agent id wins a tie, which keeps tie-breaking
    /// stable across runs. Opposite-direction and crossing-path interaction is
    /// later work.
    fn nearest_leader(&self, index: usize) -> Option<(AgentId, Constraint)> {
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
        best.map(|(other, gap)| {
            (
                AgentId::from_index(other),
                Constraint {
                    gap_m: gap.max(0.0),
                    speed_mps: self.agents.speed_mps[other],
                    standstill_m: IDM_STANDSTILL_GAP_M,
                },
            )
        })
    }

    /// Compute and record the agent's current signal-compliance decision.
    ///
    /// This is also where a red-light violation is recorded, because the two
    /// decision records make the crossing unambiguous without recomputing any
    /// geometry: the recorded previous decision was to proceed with the front
    /// bumper still upstream of a line whose head forbade it (its gap is
    /// positive, its action is `Proceed`, and its color is not green), and the
    /// decision this tick reports `PastStopLine` for the first time. A vehicle
    /// whose recorded decision was to stop is not a noncompliant run even when
    /// it comes to rest exactly on the line, and the `PastStopLine` record keeps
    /// every later tick from repeating the violation, so the record is emitted
    /// exactly once per crossing action.
    fn update_signal_decision(&mut self, index: usize) {
        let previous = self.agents.decision[index];
        let decision = self.signal_decision(index);
        if let Some(previous) = previous
            && let Some(decision) = decision
            && decision.reason == ComplianceReason::PastStopLine
            && previous.action == SignalAction::Proceed
            && previous.color != SignalColor::Green
            && previous.stop_line_gap_m > 0.0
        {
            self.events.push(Event::Violation {
                agent: AgentId::from_index(index),
                kind: ViolationKind::RanRedLight,
            });
        }
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
            // The mode template this source produces; `None` for a version-1
            // source with no mode tag, which keeps the car path unchanged.
            let source = self.demand[runtime].source;
            let mode = self.scenario.demand_mode(DemandId::from_index(source));
            while let Some(&movement) = self.demand[runtime].pending.front() {
                if self.try_admit(movement, mode) {
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
    ///
    /// `mode` is the mode template the source produces, or `None` for a
    /// version-1 source. A capsule template spawns the narrow wheeled family
    /// (a narrow profile and a capsule body); every other mode, and a version-1
    /// source, keeps the passenger-car path. The branch is on the compiled
    /// family, never on a template id.
    fn try_admit(&mut self, movement_id: MovementId, mode: Option<ModeTemplateId>) -> bool {
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
        // `compliance` stream; the two never share a generator. A narrow wheeled
        // mode draws from the same per-agent streams and projects onto the
        // shared longitudinal profile, so the shared stages stay one code path.
        let agent_id = AgentId::from_index(self.agents.len());
        let narrow_profile = match mode.and_then(|id| self.scenario.mode_template(id)) {
            Some(template) if template.family() == Some(AgentFamily::WheeledCapsule) => {
                Some(sample_narrow_profile(
                    template,
                    &mut derive_stream(self.config.seed(), STREAM_PROFILE, agent_id.get()),
                    &mut derive_stream(self.config.seed(), STREAM_COMPLIANCE, agent_id.get()),
                ))
            }
            _ => None,
        };
        let profile = match narrow_profile {
            Some(narrow) => narrow.vehicle_profile(),
            None => sample_profile(
                self.scenario.profiles(),
                &mut derive_stream(self.config.seed(), STREAM_PROFILE, agent_id.get()),
                &mut derive_stream(self.config.seed(), STREAM_COMPLIANCE, agent_id.get()),
            ),
        };
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
        // A steering mode on a compiled facility initializes its route
        // coordinates by projecting the entry pose onto the facility reference.
        // The family check inside keeps passenger cars and narrow agents on one
        // code path; a version-1 demand has no mode and carries no state.
        let route_state = mode.and_then(|mode| {
            self.route_state_for(mode, path_id, position, direction, narrow_profile)
        });

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
            narrow_profile,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state,
        });
        self.spawned_total += 1;
        self.events.push(Event::Spawned {
            agent: agent_id,
            mode: AgentMode::Vehicle,
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
        // share the mode-neutral `profile` stream with vehicles for their body
        // and gait and the `compliance` stream for their crossing propensity;
        // agent ids are unique across modes, so no two agents share a generator.
        let agent_id = AgentId::from_index(self.agents.len());
        let profile = sample_pedestrian_profile(
            self.scenario.pedestrian_profiles(),
            &mut derive_stream(self.config.seed(), STREAM_PROFILE, agent_id.get()),
            &mut derive_stream(self.config.seed(), STREAM_COMPLIANCE, agent_id.get()),
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
            narrow_profile: None,
            pedestrian_route: Some(route_id),
            pedestrian_profile: Some(profile),
            // A pedestrian walks in world space and carries no route
            // coordinates.
            route_state: None,
        });
        self.spawned_total += 1;
        self.events.push(Event::Spawned {
            agent: agent_id,
            mode: AgentMode::Pedestrian,
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

    /// Route-relative state for a newly spawned agent, when its mode steers and
    /// its route lies on a compiled facility that mode may use.
    ///
    /// The decision is taken from the compiled mode family and the compiled
    /// facility geometry, never from a mode or scenario id: a wheeled box, a
    /// wheeled capsule, and a future articulated chain share the one path, and
    /// a holonomic walking body carries no route coordinates. The mode's
    /// resolved lateral policy seeds the target clearance and horizon when it
    /// declares one, and its sampled narrow profile seeds the bounded-steering
    /// limits and usable corridor; `None` leaves each of them absent exactly as
    /// Increment 1.
    fn route_state_for(
        &self,
        mode: ModeTemplateId,
        path: PathId,
        position: DVec2,
        direction: f64,
        narrow: Option<NarrowProfile>,
    ) -> Option<RouteState> {
        let template = self.scenario.mode_template(mode)?;
        if template.family() == Some(AgentFamily::HolonomicCircle) {
            return None;
        }
        let facility = self.scenario.facilities().iter().find(|facility| {
            facility.reference_path() == Some(path) && facility.permits_mode(mode)
        })?;
        let geometry = facility.reference()?.geometry();
        let lateral = template.lateral();
        let state = RouteState::project(
            facility.id(),
            geometry,
            position,
            direction,
            lateral.map(|policy| policy.target_clearance_m()),
            lateral.map(|policy| policy.horizon_s()),
        );
        // Only a mode whose sampled profile carries a lateral-acceleration limit
        // can steer laterally; the corridor is the facility's usable interval
        // for its envelope width and preferred clearance. A template with no
        // compiled limit stays longitudinal-only.
        let bounded = narrow.and_then(|narrow| {
            let lateral_accel_max_mps2 = narrow.lateral_accel_max_mps2?;
            let interval =
                facility.usable_lateral_interval(narrow.body_width_m(), narrow.lateral_clearance_m);
            Some(BoundedSteering {
                limits: SteeringLimits {
                    heading_rate_max_rad_s: narrow.steering_rate_max_rad_s,
                    lateral_accel_max_mps2,
                },
                corridor: LateralCorridor {
                    d_min: interval.d_min(),
                    d_max: interval.d_max(),
                },
            })
        });
        Some(match bounded {
            Some(bounded) => state.with_bounded_steering(bounded),
            None => state,
        })
    }

    /// Reproject one agent's integrated world pose into its facility route
    /// frame.
    ///
    /// The world pose is the integrated truth, so the tactical coordinates are
    /// projected back from it after integration; the pose is never moved to a
    /// target offset. An agent with no route state is left untouched, so the
    /// legacy path population costs nothing.
    fn reproject_route_state(&mut self, index: usize, direction: f64) {
        let Some(mut state) = self.agents.route_state[index] else {
            return;
        };
        let position = self.agents.position[index];
        let Some(geometry) = self
            .scenario
            .facility(state.facility)
            .and_then(|facility| facility.reference())
            .map(|reference| reference.geometry())
        else {
            return;
        };
        state.reproject(geometry, position, direction);
        self.agents.route_state[index] = Some(state);
    }

    /// The facility reference geometry of an agent's route state, when it has
    /// one; `None` for an agent with no route state.
    fn route_geometry(&self, index: usize) -> Option<&CompiledReferencePath> {
        let state = self.agents.route_state[index]?;
        self.scenario
            .facility(state.facility)
            .and_then(|facility| facility.reference())
            .map(|reference| reference.geometry())
    }

    /// An agent's lateral steering target for this step, when its maneuver
    /// displaces it.
    ///
    /// The target is the tactical stage's request; this leaf only integrates it
    /// under the compiled limits and never selects it. A `committed` maneuver
    /// steers toward its fixed target, a `returning` or `aborted` one steers back
    /// to the offset the agent held before the attempt, and a `preparing` or
    /// `following` agent displaces nothing at all: a preparing maneuver has fixed
    /// a target but has not been granted a corridor for it.
    fn lateral_request(&self, index: usize) -> Option<f64> {
        let state = self.agents.route_state.get(index).copied().flatten()?;
        match state.maneuver {
            ManeuverState::Committed => state.target_offset_m,
            ManeuverState::Returning | ManeuverState::Aborted => Some(state.pre_maneuver_offset_m),
            ManeuverState::Following | ManeuverState::Preparing => None,
        }
    }
}

impl RelevantWorldQuery for Simulation {
    /// Stage 1: collect the immutable relevant world one agent sees this step.
    ///
    /// The kernel selects every constraint, leader, control, route target, and
    /// nearby body here; no model participates. A pedestrian carrying both a
    /// demand route and a sampled profile uses the world-steering observation;
    /// every other live slot — a vehicle, or the walking skeleton's scripted
    /// body — uses the path-following one.
    fn query_world(&mut self, index: usize) -> Observation {
        if self.agents.mode[index] == AgentMode::Pedestrian
            && self.agents.pedestrian_route[index].is_some()
            && self.agents.pedestrian_profile[index].is_some()
        {
            self.query_pedestrian_world(index)
        } else {
            self.query_vehicle_world(index)
        }
    }
}

/// One planned maneuver state change, applied after the whole batch has been
/// decided.
///
/// A plan is a pure function of the tick-start observation, so holding the
/// updates until the batch ends is what keeps a decision from reading a state
/// another decision wrote. `transition` is `None` for a state change that is not
/// a lifecycle transition, such as dropping an intent whose target has gone.
struct ManeuverPlan {
    index: usize,
    state: RouteState,
    transition: Option<ManeuverTransition>,
}

/// Why a lateral attempt is or is not admissible this step.
enum Attempt {
    /// The target and the candidate corridor are fixed; the maneuver prepares.
    Admissible {
        /// The corridor the maneuver claims, fixed from here on.
        corridor: ManeuverCorridor,
        /// Predicted minimum swept clearance over the prediction horizon, in
        /// metres.
        predicted_clearance_m: f64,
    },
    /// The passed obstacle has disappeared, so there is nothing to attempt.
    TargetLost,
    /// The candidate corridor is not feasible right now. The intent is kept and
    /// retried at the next decision, which is how a maneuver waits for a gap.
    Infeasible,
}

/// The batch-wide inputs every maneuver decision of one step reads: the
/// tick-start bodies, the scenario's commit policy, and the step's time and
/// length.
struct ManeuverBatch<'a> {
    bodies: &'a [PredictedBody],
    commit: &'a CommitPolicySource,
    now: SimTime,
    dt: f64,
}

/// Whether `timeout_s` has elapsed at `now` since `since`, measured in whole
/// ticks of the fixed step so the comparison is exact and never wall-clock.
fn hold_elapsed(now: SimTime, since: Option<SimTime>, timeout_s: f64, dt: f64) -> bool {
    since.is_some_and(|since| now.tick().saturating_sub(since.tick()) as f64 * dt >= timeout_s)
}

impl Simulation {
    /// The path-following observation: the profile, speed, and the constraints
    /// the kernel selected ahead.
    ///
    /// The signal decision and the crossing-yield state are recomputed first,
    /// so the stop-line and yield constraints are exactly the recorded state
    /// and the yield transition is emitted once.
    fn query_vehicle_world(&mut self, index: usize) -> Observation {
        // Recompute the signal decision before reading it, so the stop-line
        // constraint is exactly the recorded decision.
        self.update_signal_decision(index);
        // Recompute the crossing-yield state from the shared index before
        // reading it, so the yield constraint is exactly the recorded state.
        self.update_yield_state(index);

        let profile = self.agents.profile[index];
        // Profile vehicles select their constraints; the walking skeleton's
        // scripted population has none and keeps its constant speed.
        let (leader, stop_line, crossing_yield) = match profile {
            Some(_) => (
                self.nearest_leader(index),
                self.stop_line_constraint(index),
                self.crossing_yield_constraint(index),
            ),
            None => (None, None, None),
        };
        Observation::Vehicle(VehicleObservation {
            profile,
            narrow: self.agents.narrow_profile[index],
            speed_mps: self.agents.speed_mps[index],
            leader,
            stop_line,
            crossing_yield,
        })
    }

    /// The world-steering observation: the waypoint cursor, the signal
    /// decision, the target waypoint, and the ordered nearby bodies.
    fn query_pedestrian_world(&mut self, index: usize) -> Observation {
        let route_id = self.agents.pedestrian_route[index]
            .expect("the pedestrian stage is entered only with a demand route");
        let profile = self.agents.pedestrian_profile[index]
            .expect("the pedestrian stage is entered only with a sampled profile");
        let direction = self.agents.direction[index];
        let path_length_m = self
            .scenario
            .path(self.agents.path[index])
            .map_or(0.0, |path| path.length());

        // Advance the cursor past every waypoint already reached, so the target
        // is always ahead and the cursor is monotone.
        let progress_m =
            pedestrian::route_progress_m(self.agents.distance_m[index], direction, path_length_m);
        self.advance_waypoint(index, progress_m);
        let cursor = self.agents.pedestrian_waypoint_index[index];

        // The pedestrian signal compliance decision is a pure function of the
        // upcoming crossing's signal state and the pedestrian's state, so it is
        // recomputed here and recorded before the step integrates.
        let decision = self.pedestrian_signal_decision(index, route_id, cursor, progress_m);
        self.agents.pedestrian_decision[index] = decision;

        let target = self.route_waypoints(route_id).get(cursor).copied();
        let state = PedestrianState {
            agent: AgentId::from_index(index),
            position: self.agents.position[index],
            heading_rad: self.agents.heading_rad[index],
            speed_mps: self.agents.speed_mps[index],
            target: target.map_or(self.agents.position[index], PedestrianWaypoint::position),
        };

        // The nearby bodies are collected only when a waypoint remains to steer
        // toward; without a target the update is a no-op and the query stays
        // empty. The reused scan buffer moves into the observation and the
        // advance stage returns it, so the scan does not allocate per step.
        let conflicts = if target.is_some() {
            self.collect_conflicts(index);
            std::mem::take(&mut self.conflicts)
        } else {
            Vec::new()
        };

        Observation::Pedestrian(PedestrianObservation {
            profile,
            state,
            target,
            decision,
            conflicts,
        })
    }
}

impl TacticalChoice for Simulation {
    /// Stage 2: record the one maneuver the agent executes this step.
    ///
    /// The tactic's maneuver state is the agent's own lateral-maneuver state: a
    /// longitudinal tactic never claims a lateral maneuver, so a free-flowing,
    /// following, stopping, yielding, or waypoint-seeking agent is `following`
    /// unless the kernel's maneuver pass holds it in another state.
    fn choose_tactic(&self, index: usize, observation: &Observation) -> Tactic {
        let (reason, target, abort) = match observation {
            Observation::Vehicle(vehicle) => {
                if vehicle.stop_line.is_some() {
                    (
                        TacticReason::StopLine,
                        TacticTarget::StopLine,
                        AbortCondition::ConstraintClears,
                    )
                } else if let Some((crossing, _)) = vehicle.crossing_yield {
                    (
                        TacticReason::YieldCrossing,
                        TacticTarget::Crossing(crossing),
                        AbortCondition::ConstraintClears,
                    )
                } else if let Some((leader, _)) = vehicle.leader {
                    (
                        TacticReason::Follow,
                        TacticTarget::Leader(leader),
                        AbortCondition::ConstraintClears,
                    )
                } else {
                    (
                        TacticReason::FreeFlow,
                        TacticTarget::Route,
                        AbortCondition::RouteComplete,
                    )
                }
            }
            Observation::Pedestrian(pedestrian) => {
                if pedestrian
                    .decision
                    .is_some_and(|decision| decision.action == PedestrianSignalAction::Wait)
                {
                    let target = pedestrian
                        .target
                        .map_or(TacticTarget::Route, TacticTarget::Waypoint);
                    (
                        TacticReason::SignalWait,
                        target,
                        AbortCondition::ConstraintClears,
                    )
                } else if let Some(target) = pedestrian.target {
                    (
                        TacticReason::SeekWaypoint,
                        TacticTarget::Waypoint(target),
                        AbortCondition::WaypointReached,
                    )
                } else {
                    (
                        TacticReason::SeekWaypoint,
                        TacticTarget::Route,
                        AbortCondition::RouteComplete,
                    )
                }
            }
        };
        Tactic {
            reason,
            target,
            maneuver_state: self.agent_maneuver_state(index),
            abort,
            started_at: self.time(),
        }
    }
}

impl MotionControl for Simulation {
    /// Stage 3: convert the tactic into a bounded command.
    ///
    /// The raw command comes from the replaceable model through
    /// [`crate::controller`]; the profile bounds, the three kernel safety caps
    /// (leader rear, stop line, crossing yield), and the emergency counter are
    /// applied here, outside the model.
    fn command_motion(
        &mut self,
        index: usize,
        observation: &Observation,
        tactic: &Tactic,
        dt: f64,
    ) -> MotionCommand {
        match observation {
            Observation::Vehicle(vehicle) => {
                let Some(profile) = vehicle.profile else {
                    // The walking skeleton's scripted population has no model to
                    // command and keeps its constant speed.
                    return MotionCommand::Longitudinal {
                        speed_mps: vehicle.speed_mps,
                    };
                };
                let leader = vehicle.leader.map(|(_, constraint)| constraint);
                let stop_line = vehicle.stop_line;
                let crossing_yield = vehicle.crossing_yield.map(|(_, constraint)| constraint);

                let mut constraints = [Constraint {
                    gap_m: f64::INFINITY,
                    speed_mps: 0.0,
                    standstill_m: 0.0,
                }; 3];
                let mut count = 0;
                for constraint in [leader, stop_line, crossing_yield].into_iter().flatten() {
                    constraints[count] = constraint;
                    count += 1;
                }

                // A narrow mode reaches the narrow wheeled model; every other
                // path-following agent reaches the vehicle model. The dispatch
                // is on the agent's own profile, never on a mode name.
                let accel = match vehicle.narrow {
                    Some(narrow) => self.controllers.narrow.desired_acceleration(
                        &narrow,
                        vehicle.speed_mps,
                        &constraints[..count],
                    ),
                    None => self.controllers.vehicle.desired_acceleration(
                        &profile,
                        vehicle.speed_mps,
                        &constraints[..count],
                    ),
                };
                let mut new_speed =
                    (vehicle.speed_mps + accel * dt).clamp(0.0, profile.desired_speed_mps);

                if let Some(leader) = leader {
                    // The follower's front bumper must not pass the leader's
                    // rear this step. The gap already reflects the leader's
                    // post-step position (lower-index leaders integrate first),
                    // so the reachable speed is `gap / dt`.
                    new_speed = new_speed.min(leader.gap_m / dt);
                }
                if let Some(stop_line) = stop_line {
                    // A required stop holds the front bumper at the line.
                    new_speed = new_speed.min(stop_line.gap_m / dt);
                }
                if let Some(crossing_yield) = crossing_yield {
                    // Yielding holds the front bumper short of the crossing
                    // entry, so a vehicle never drives onto a pedestrian-occupied
                    // crossing. In the normal regime IDM's bounded braking stops
                    // it; this cap is the backstop.
                    new_speed = new_speed.min(crossing_yield.gap_m / dt);
                }
                let new_speed = new_speed.max(0.0);

                // Comfortable braking alone would leave the vehicle at this
                // speed, so anything below it was forced by a cap rather than
                // by the controller.
                let comfort_floor =
                    (vehicle.speed_mps - profile.comfortable_brake_mps2 * dt).max(0.0);
                if new_speed < comfort_floor - 1e-9 {
                    self.emergency_cap_steps += 1;
                }

                // The unsafe-commit policy's brake response: a committed maneuver
                // that lost predicted clearance decelerates within the profile's
                // comfortable braking and never accelerates. The cap is exactly
                // the comfort envelope, so it is not an emergency cap and no
                // maneuver ever leans on the kernel's position caps.
                let new_speed = if self.agents.route_state[index].is_some_and(|state| state.braking)
                {
                    new_speed.min(comfort_floor)
                } else {
                    new_speed
                };

                // A fixed lateral target turns this step into a bounded steering
                // step. The longitudinal speed above has already applied the
                // leader, stop-line, and crossing-yield caps, so the corridor
                // check and those caps constrain the same proposed world step; a
                // step that would leave the corridor holds instead of clipping.
                if let Some(target_offset_m) = self.lateral_request(index)
                    && let Some(steering) =
                        self.agents.route_state[index].and_then(|state| state.bounded_steering)
                    && let Some(geometry) = self.route_geometry(index)
                {
                    let request = SteeringRequest {
                        position: self.agents.position[index],
                        heading_rad: self.agents.heading_rad[index],
                        speed_mps: new_speed,
                        target_offset_m,
                        direction: self.agents.direction[index],
                    };
                    return match bounded_steering_step(geometry, request, steering, dt) {
                        Some(step) => MotionCommand::RouteSteering {
                            heading_rad: step.heading_rad,
                            speed_mps: step.speed_mps,
                        },
                        None => MotionCommand::RouteSteering {
                            heading_rad: self.agents.heading_rad[index],
                            speed_mps: 0.0,
                        },
                    };
                }

                MotionCommand::Longitudinal {
                    speed_mps: new_speed,
                }
            }
            Observation::Pedestrian(pedestrian) => {
                if pedestrian.target.is_none() {
                    return MotionCommand::Idle;
                }
                let mut steering = self.controllers.pedestrian.steer(
                    &pedestrian.profile,
                    &pedestrian.state,
                    &pedestrian.conflicts,
                    dt,
                );
                if tactic.reason == TacticReason::SignalWait
                    && let Some(decision) = pedestrian.decision
                {
                    // Obey the signal with a bounded stopping profile toward
                    // the crossing; the speed-change bound still applies.
                    let stop_target = pedestrian_compliance::wait_speed_target_mps(
                        decision.crossing_gap_m,
                        pedestrian.profile.compliance,
                        pedestrian::MAX_DECEL_MPS2,
                    );
                    steering.speed_target_mps = steering.speed_target_mps.min(stop_target);
                }
                let (speed_mps, capped) = self.controllers.pedestrian.advance_speed(
                    pedestrian.state.speed_mps,
                    steering.speed_target_mps,
                    steering.spacing_cap_mps,
                    dt,
                );
                if capped {
                    self.pedestrian_cap_steps += 1;
                }
                MotionCommand::Steering {
                    heading_rad: steering.heading_rad,
                    speed_mps,
                }
            }
        }
    }
}

impl PhysicalAdvance for Simulation {
    /// Stage 4: integrate the pose the command implies and emit the step's
    /// diagnostics.
    fn advance_physics(
        &mut self,
        index: usize,
        observation: Observation,
        command: &MotionCommand,
        dt: f64,
    ) {
        match observation {
            Observation::Vehicle(_) => match *command {
                MotionCommand::Longitudinal { speed_mps } => {
                    let path_id = self.agents.path[index];
                    let direction = self.agents.direction[index];
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
                    self.reproject_route_state(index, direction);

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
                MotionCommand::RouteSteering {
                    heading_rad,
                    speed_mps,
                } => {
                    // The world pose is the integrated truth: step the heading
                    // and position in world coordinates from the commanded step,
                    // then project the result back onto the route frame. No
                    // target offset is written to the pose or to `d`.
                    let path_id = self.agents.path[index];
                    let direction = self.agents.direction[index];
                    let length = self
                        .scenario
                        .path(path_id)
                        .map_or(0.0, |path| path.length());
                    let position = self.agents.position[index]
                        + DVec2::from_angle(heading_rad) * (speed_mps * dt);

                    self.agents.speed_mps[index] = speed_mps;
                    self.agents.heading_rad[index] = heading_rad;
                    self.agents.position[index] = position;
                    self.reproject_route_state(index, direction);
                    if let Some(state) = self.agents.route_state[index] {
                        self.agents.distance_m[index] = state.s_m;
                    }

                    let s_m = self.agents.distance_m[index];
                    if s_m <= 0.0 || s_m >= length {
                        self.agents.alive[index] = false;
                        self.despawned_total += 1;
                        self.events.push(Event::Despawned {
                            agent: AgentId::from_index(index),
                            path: path_id,
                            reason: DespawnReason::ExitedPath,
                        });
                    }
                }
                MotionCommand::Idle | MotionCommand::Steering { .. } => {}
            },
            Observation::Pedestrian(mut pedestrian) => {
                let MotionCommand::Steering {
                    heading_rad,
                    speed_mps,
                } = *command
                else {
                    return;
                };
                // Return the reused conflict buffer the query stage borrowed, so
                // the next pedestrian scan reuses its capacity.
                std::mem::swap(&mut self.conflicts, &mut pedestrian.conflicts);

                let path_id = self.agents.path[index];
                let direction = self.agents.direction[index];
                let path_length_m = self
                    .scenario
                    .path(path_id)
                    .map_or(0.0, |path| path.length());
                let position =
                    pedestrian.state.position + DVec2::from_angle(heading_rad) * (speed_mps * dt);

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
        }
    }
}

/// Axis-aligned bounding box of a polygon ring, as `(min, max)`.
///
/// An empty ring falls back to the origin, so a caller can always pass the box
/// to a query without a special case.
fn bounding_box(ring: &[DVec2]) -> (DVec2, DVec2) {
    let mut min = DVec2::splat(f64::INFINITY);
    let mut max = DVec2::splat(f64::NEG_INFINITY);
    for &vertex in ring {
        min = min.min(vertex);
        max = max.max(vertex);
    }
    if ring.is_empty() {
        return (DVec2::ZERO, DVec2::ZERO);
    }
    (min, max)
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
    use crate::controller::{VehicleController, WaypointController};
    use crate::narrow;
    use crate::units::Seconds;
    use tangle_model::{
        AgentBody, CompiledModeTemplate, CompiledScenario, ProfileRange, compile_mode_template,
        parse_scenario_source, parse_scenario_source_v2,
    };

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
            .map(|agent| agent.motion.as_ref().expect("full detail").path_distance_m)
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
        let lead = after_one.agents()[0].motion.as_ref().expect("full detail");
        // 12 m/s * 0.05 s = 0.6 m.
        assert!((lead.path_distance_m - 0.6).abs() < 1e-12);
        assert!((after_one.agents()[0].position - glam::DVec2::new(0.6, 0.0)).length() < 1e-12);

        for _ in 0..9 {
            sim.step();
        }
        assert_eq!(sim.time().tick(), 10);
        assert!((sim.time().seconds() - 0.5).abs() < 1e-12);
        let after_ten = sim.snapshot(SnapshotDetail::Full);
        assert!(
            (after_ten.agents()[0]
                .motion
                .as_ref()
                .expect("full detail")
                .path_distance_m
                - 6.0)
                .abs()
                < 1e-12
        );
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
        let (_, leader) = sim.nearest_leader(0).expect("a tied leader");
        assert_eq!(
            leader.speed_mps, 5.0,
            "the lowest tied id must win, even when it is the slower candidate"
        );

        // Swapping which tied candidate moves faster must not move the winner.
        sim.agents.speed_mps[1] = 9.0;
        sim.agents.speed_mps[2] = 5.0;
        let (_, leader) = sim.nearest_leader(0).expect("a tied leader");
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
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
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

    /// A minimal crossing scenario whose authored pedestrian radius is far
    /// above the candidate-margin floor.
    const WIDE_PEDESTRIAN_CROSSING: &str = r#"
    {
      schema_version: 1,
      id: 'wide_pedestrian_crossing',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      regions: [
        {
          id: 'crossing_zone',
          points: [
            { x: -2.0, y: -4.0 },
            { x: 2.0, y: -4.0 },
            { x: 2.0, y: 4.0 },
            { x: -2.0, y: 4.0 },
          ],
        },
      ],
      paths: [ { id: 'road', points: [ { x: -40.0, y: 0.0 }, { x: 40.0, y: 0.0 } ] } ],
      portals: [
        { id: 'west_entry', path: 'road', end: 'start', width_m: 7.0 },
        { id: 'east_exit', path: 'road', end: 'end', width_m: 7.0 },
      ],
      movements: [
        { id: 'ew_through', from: 'west_entry', to: 'east_exit', path: 'road', priority: 0 },
      ],
      crossings: [
        { id: 'road_crossing', region: 'crossing_zone', movements: [ 'ew_through' ] },
      ],
      rules: [ { id: 'yield_to_road_crossing', movement: 'ew_through', kind: 'yield' } ],
      demand: [
        {
          id: 'road_inflow',
          portal: 'west_entry',
          rate_vph: 1.0,
          routes: [ { movement: 'ew_through', weight: 1.0 } ],
        },
      ],
      pedestrian_profiles: {
        radius_m: { min: 4.0, max: 4.0 },
        speed_mps: { min: 1.0, max: 1.6 },
      },
    }
    "#;

    /// A body whose centre is outside the region's widened bounding box still
    /// reaches into the crossing, so the candidate query must widen by the
    /// authored radius rather than a fixed constant.
    #[test]
    fn the_crossing_candidate_query_covers_a_far_over_large_pedestrian() {
        let source = parse_scenario_source(WIDE_PEDESTRIAN_CROSSING).expect("scenario parses");
        let scenario = CompiledScenario::compile(source).expect("scenario compiles");
        let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("simulation builds");
        // The crossing spans x in [-2, 2]; the centre at x = 5.9 is 3.9 m past
        // the +x face, beyond both the 0.5 m floor and the grid cell that covers
        // the widened box, yet within the 4.0 m body radius. Only a margin
        // derived from the authored radius returns it as a candidate.
        sim.agents.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path: PathId::from_index(0),
            distance_m: 45.9,
            speed_mps: 0.0,
            position: DVec2::new(5.9, 0.0),
            heading_rad: 0.0,
            body_length_m: 8.0,
            body_width_m: 8.0,
            direction: 1.0,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
        });
        sim.spatial.rebuild(&sim.agents);
        assert!(
            sim.crossing_occupied(CrossingId::from_index(0)),
            "a far over-large pedestrian body reaching into the crossing must be a candidate"
        );
    }

    /// A road and a separate walking path, each with demand, so both modes are
    /// live and independent.
    const MIXED_MODES: &str = r#"
    {
      schema_version: 1,
      id: 'stage_contract',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [
        { id: 'road', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] },
        { id: 'walk', points: [ { x: 0.0, y: 40.0 }, { x: 0.0, y: 80.0 } ] },
      ],
      portals: [
        { id: 'road_entry', path: 'road', end: 'start', width_m: 7.0 },
        { id: 'road_exit', path: 'road', end: 'end', width_m: 7.0 },
        { id: 'walk_entry', path: 'walk', end: 'start', width_m: 3.0 },
        { id: 'walk_exit', path: 'walk', end: 'end', width_m: 3.0 },
      ],
      movements: [
        { id: 'through', from: 'road_entry', to: 'road_exit', path: 'road', priority: 0 },
      ],
      pedestrian_routes: [
        { id: 'walk_through', from: 'walk_entry', to: 'walk_exit', path: 'walk' },
      ],
      demand: [
        { id: 'road_inflow', portal: 'road_entry', rate_vph: 1200.0,
          routes: [ { movement: 'through', weight: 1.0 } ] },
      ],
      pedestrian_demand: [
        { id: 'footfall', portal: 'walk_entry', rate_pph: 900.0,
          routes: [ { route: 'walk_through', weight: 1.0 } ] },
      ],
      profiles: {
        speed_mps: { min: 10.0, max: 10.0 },
        length_m: { min: 4.0, max: 4.0 },
        width_m: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.5, max: 1.5 },
        max_accel_mps2: { min: 2.0, max: 2.0 },
        comfortable_brake_mps2: { min: 3.0, max: 3.0 },
      },
      pedestrian_profiles: {
        radius_m: { min: 0.25, max: 0.25 },
        speed_mps: { min: 1.25, max: 1.25 },
        compliance: { min: 1.0, max: 1.0 },
      },
    }
    "#;

    fn mixed_sim(seed: u64) -> Simulation {
        let source = parse_scenario_source(MIXED_MODES).expect("scenario parses");
        let scenario = CompiledScenario::compile(source).expect("scenario compiles");
        Simulation::new(scenario, RunConfig::new(seed)).expect("simulation builds")
    }

    /// Phase 1 output reports every body's envelope kind and, being a single
    /// box or circle envelope, no ordered segments.
    #[test]
    fn a_full_snapshot_reports_each_phase1_body_kind_with_no_segments() {
        use tangle_model::BodyKind;

        let mut sim = mixed_sim(3);
        let mut saw_vehicle = false;
        let mut saw_pedestrian = false;
        for _ in 0..1500 {
            sim.step();
            for sample in sim.snapshot(SnapshotDetail::Full).agents() {
                let motion = sample.motion.as_ref().expect("full detail");
                assert!(
                    motion.segments.is_empty(),
                    "a Phase 1 body carries no ordered segments"
                );
                assert!(
                    motion.route_state.is_none(),
                    "a legacy version-1 path-following body carries no route state"
                );
                match motion.mode {
                    AgentMode::Vehicle => {
                        assert_eq!(motion.body_kind, BodyKind::Box);
                        saw_vehicle = true;
                    }
                    AgentMode::Pedestrian => {
                        assert_eq!(motion.body_kind, BodyKind::Circle);
                        saw_pedestrian = true;
                    }
                }
            }
            if saw_vehicle && saw_pedestrian {
                break;
            }
        }
        assert!(
            saw_vehicle && saw_pedestrian,
            "both Phase 1 modes must be observed"
        );
    }

    fn constant_vehicle_profile(desired_speed_mps: f64) -> VehicleProfile {
        VehicleProfile {
            desired_speed_mps,
            length_m: 4.5,
            width_m: 1.8,
            time_gap_s: 1.5,
            max_accel_mps2: 2.0,
            comfortable_brake_mps2: 3.0,
            compliance: 1.0,
        }
    }

    /// Stage 1 and stage 2 on a vehicle: the query selects the leader, and the
    /// tactic records a follow with its target and lifecycle fields. The
    /// maneuver state is the agent's own, and a longitudinal tactic is not a
    /// lateral maneuver, so it stays `following`.
    #[test]
    fn the_query_and_tactic_stages_select_the_vehicle_leader() {
        let mut sim = walking_sim(1);
        let profile = constant_vehicle_profile(12.0);
        sim.agents.profile[0] = Some(profile);
        sim.agents.profile[1] = Some(profile);

        let observation = sim.query_world(0);
        let Observation::Vehicle(vehicle) = &observation else {
            panic!("a vehicle slot yields a vehicle observation");
        };
        assert_eq!(vehicle.profile, Some(profile));
        assert_eq!(vehicle.stop_line, None, "the guide path has no stop line");
        assert_eq!(
            vehicle.crossing_yield, None,
            "the guide path crosses no crossing"
        );
        let (leader, _) = vehicle.leader.expect("the follower sees the leader ahead");
        assert_eq!(leader, AgentId::from_index(1));

        let tactic = sim.choose_tactic(0, &observation);
        assert_eq!(tactic.reason, TacticReason::Follow);
        assert_eq!(tactic.target, TacticTarget::Leader(AgentId::from_index(1)));
        assert_eq!(tactic.maneuver_state, ManeuverState::Following);
        assert_eq!(tactic.abort, AbortCondition::ConstraintClears);
        assert_eq!(tactic.started_at, sim.time());
    }

    /// Stage 2 and stage 3 on an observation that carries a required stop: the
    /// tactic is the stop-line maneuver, and the motion stage bounds the command
    /// to the stop-line cap the kernel selected. A stop is a longitudinal tactic
    /// the agent holds, not a committed lateral maneuver.
    #[test]
    fn the_tactic_stage_commits_the_stop_line_maneuver() {
        let mut sim = walking_sim(1);
        let profile = constant_vehicle_profile(12.0);
        let dt = sim.config().step().as_secs();

        // A required stop is an observation input: the kernel already recorded
        // the decision to stop, and this test pins how stages 2 and 3 map it.
        let gap_m = 0.5;
        let observation = Observation::Vehicle(VehicleObservation {
            profile: Some(profile),
            narrow: None,
            speed_mps: 12.0,
            leader: None,
            stop_line: Some(Constraint {
                gap_m,
                speed_mps: 0.0,
                standstill_m: 0.0,
            }),
            crossing_yield: None,
        });
        let tactic = sim.choose_tactic(0, &observation);
        assert_eq!(tactic.reason, TacticReason::StopLine);
        assert_eq!(tactic.target, TacticTarget::StopLine);
        assert_eq!(tactic.maneuver_state, ManeuverState::Following);
        assert_eq!(tactic.abort, AbortCondition::ConstraintClears);
        assert_eq!(tactic.started_at, sim.time());

        let command = sim.command_motion(0, &observation, &tactic, dt);
        let MotionCommand::Longitudinal { speed_mps } = command else {
            panic!("a required stop still commands a longitudinal speed");
        };
        assert!(
            (0.0..=profile.desired_speed_mps).contains(&speed_mps),
            "the stop command must stay within the profile's speed bounds"
        );
        assert!(
            speed_mps <= gap_m / dt + 1e-9,
            "the stop-line cap must bound the command so the bumper reaches the line this step"
        );
    }

    /// Stage 3 and stage 4 on a vehicle: the motion stage bounds the command to
    /// the profile, and the advance stage integrates exactly that command.
    #[test]
    fn the_motion_and_advance_stages_command_and_integrate_the_vehicle() {
        let mut sim = walking_sim(1);
        let profile = constant_vehicle_profile(12.0);
        sim.agents.profile[0] = Some(profile);
        sim.agents.profile[1] = Some(profile);
        let dt = sim.config().step().as_secs();

        let observation = sim.query_world(0);
        let tactic = sim.choose_tactic(0, &observation);
        let command = sim.command_motion(0, &observation, &tactic, dt);
        let MotionCommand::Longitudinal { speed_mps } = command else {
            panic!("a path-following agent commands a longitudinal speed");
        };
        assert!(
            (0.0..=profile.desired_speed_mps).contains(&speed_mps),
            "the command must stay within the profile's speed bounds"
        );

        let before = sim.agents.distance_m[0];
        sim.advance_physics(0, observation, &command, dt);
        assert!((sim.agents.distance_m[0] - (before + speed_mps * dt)).abs() < 1e-12);
        assert_eq!(sim.agents.speed_mps[0], speed_mps);
    }

    /// A vehicle model that always commands a 1 m/s² brake, so the stage's
    /// command is unmistakably not IDM's.
    #[derive(Debug)]
    struct FixedBrakeModel;

    impl VehicleController for FixedBrakeModel {
        fn name(&self) -> &'static str {
            "fixed-brake stage test double"
        }

        fn desired_acceleration(
            &self,
            _profile: &VehicleProfile,
            _speed_mps: f64,
            _constraints: &[Constraint],
        ) -> f64 {
            -1.0
        }
    }

    /// The motion stage reaches the replaceable vehicle model through the
    /// interface: the installed stub, not IDM, sets the command.
    #[test]
    fn the_motion_stage_reaches_the_vehicle_model_through_the_interface() {
        let mut sim = walking_sim(1);
        sim.agents.profile[0] = Some(constant_vehicle_profile(12.0));
        sim.set_controller_models(ControllerModels {
            vehicle: Box::new(FixedBrakeModel),
            narrow: Box::new(narrow::IdmNarrowWheeledController),
            pedestrian: Box::new(WaypointController),
        });
        let dt = sim.config().step().as_secs();

        let observation = sim.query_world(0);
        let tactic = sim.choose_tactic(0, &observation);
        let command = sim.command_motion(0, &observation, &tactic, dt);
        let MotionCommand::Longitudinal { speed_mps } = command else {
            panic!("a path-following agent commands a longitudinal speed");
        };
        // IDM at the desired speed of 12 m/s would hold it; the stub's fixed
        // brake slows the vehicle, so the model really is reached.
        assert!(speed_mps < 12.0 - 1e-9);
    }

    /// All four stages on a demand pedestrian: the waypoint observation, the
    /// seek tactic, the steering command, and the world-space integration.
    #[test]
    fn the_four_stages_drive_a_demand_pedestrian() {
        let mut sim = mixed_sim(3);
        let mut pedestrian = None;
        for _ in 0..1500 {
            sim.step();
            pedestrian = (0..sim.agents.len()).find(|&index| {
                sim.agents.alive[index] && sim.agents.mode[index] == AgentMode::Pedestrian
            });
            if pedestrian.is_some() {
                break;
            }
        }
        let index = pedestrian.expect("a pedestrian arrives within 75 s");
        let dt = sim.config().step().as_secs();

        let observation = sim.query_world(index);
        let Observation::Pedestrian(pedestrian) = &observation else {
            panic!("a demand pedestrian yields a pedestrian observation");
        };
        assert_eq!(pedestrian.state.agent, AgentId::from_index(index));
        assert!(pedestrian.profile.desired_speed_mps > 0.0);
        assert!(
            pedestrian.decision.is_none(),
            "the walking path reaches no signal-controlled crossing"
        );
        let target = pedestrian.target.expect("the route has a waypoint ahead");
        assert_eq!(pedestrian.state.target, target.position());

        let tactic = sim.choose_tactic(index, &observation);
        assert_eq!(tactic.reason, TacticReason::SeekWaypoint);
        assert_eq!(tactic.target, TacticTarget::Waypoint(target));
        assert_eq!(tactic.maneuver_state, ManeuverState::Following);
        assert_eq!(tactic.abort, AbortCondition::WaypointReached);

        let command = sim.command_motion(index, &observation, &tactic, dt);
        let MotionCommand::Steering {
            heading_rad,
            speed_mps,
        } = command
        else {
            panic!("a pedestrian commands a steering heading and speed");
        };

        let before = pedestrian.state.position;
        sim.advance_physics(index, observation, &command, dt);
        assert_eq!(
            sim.agents.position[index],
            before + DVec2::from_angle(heading_rad) * (speed_mps * dt)
        );
        assert_eq!(sim.agents.speed_mps[index], speed_mps);
    }

    /// The checked-in synthetic mode-template fixture (TAS-070), shared with
    /// `tangle-model`'s compilation test.
    const SYNTHETIC_TEMPLATE_FIXTURE: &str =
        include_str!("../../tangle-model/tests/fixtures/synthetic_mode_template_v2.json5");

    /// A 400 m straight guide path carrying one scripted body, so a single
    /// agent can be driven through the shared stages with no leader and no
    /// interaction.
    const SYNTHETIC_GUIDE: &str = r#"
    {
      schema_version: 1,
      id: 'synthetic_guide_v1',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 400.0, y: 0.0 } ] } ],
      portals: [
        { id: 'west_entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'east_exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      population: {
        vehicle_count: 1,
        vehicle_speed_mps: 12.0,
        vehicle_spacing_m: 20.0,
        vehicle_length_m: 4.5,
        vehicle_width_m: 1.8,
      },
    }
    "#;

    fn synthetic_guide_sim(seed: u64) -> Simulation {
        let source = parse_scenario_source(SYNTHETIC_GUIDE).expect("scenario parses");
        let scenario = CompiledScenario::compile(source).expect("scenario compiles");
        Simulation::new(scenario, RunConfig::new(seed)).expect("simulation builds")
    }

    /// The checked-in synthetic template, compiled to its components.
    ///
    /// The fixture is not named by id here on purpose: the compiled bundle is
    /// the only handle shared code may take, and the no-branch guard
    /// (`crates/tangle-sim/tests/synthetic_template_no_branch.rs`) fails if the
    /// id reaches this module.
    fn compiled_synthetic_template() -> CompiledModeTemplate {
        let source = parse_scenario_source_v2(SYNTHETIC_TEMPLATE_FIXTURE)
            .expect("the synthetic fixture is a version-2 document");
        assert_eq!(
            source.mode_templates.len(),
            1,
            "the fixture declares exactly one mode template"
        );
        let template = source
            .mode_templates
            .first()
            .expect("the fixture declares one mode template");
        compile_mode_template(template).expect("the synthetic template compiles")
    }

    /// The shared longitudinal/body profile the kernel's controller reads,
    /// derived from a compiled mode template's components.
    ///
    /// The synthetic fixture authors constant ranges, so each component is a
    /// single sampled value: the function carries the altered body dimensions
    /// and kinematic limits as the [`VehicleProfile`] the shared model consumes.
    fn vehicle_profile_from_components(template: &CompiledModeTemplate) -> VehicleProfile {
        let constant = |range: ProfileRange| {
            assert_eq!(
                range.min(),
                range.max(),
                "the synthetic fixture authors constant ranges"
            );
            range.min()
        };
        let AgentBody::Box { length_m, width_m } = template.body() else {
            panic!("the synthetic template carries a box body");
        };
        let profile = template.profile();
        VehicleProfile {
            desired_speed_mps: constant(profile.desired_speed_mps()),
            length_m: constant(*length_m),
            width_m: constant(*width_m),
            time_gap_s: constant(
                profile
                    .time_gap_s()
                    .expect("a wheeled profile carries a gap"),
            ),
            max_accel_mps2: constant(
                profile
                    .max_accel_mps2()
                    .expect("a wheeled profile carries an acceleration bound"),
            ),
            comfortable_brake_mps2: constant(
                profile
                    .comfortable_brake_mps2()
                    .expect("a wheeled profile carries a braking bound"),
            ),
            compliance: constant(profile.compliance()),
        }
    }

    /// The synthetic template's compiled components drive the shared stages:
    /// stages 1-4 over a defined step sequence, with the altered limits binding
    /// the motion.
    ///
    /// Increment 0 does not yet wire `CompiledModeTemplate` into spawning, so
    /// this test installs the derived body and profile on one spawned agent —
    /// the step a later increment performs from a template — and then runs the
    /// kernel's own stage implementations, which never see a mode name.
    #[test]
    fn the_synthetic_template_runs_through_the_shared_stages() {
        let template = compiled_synthetic_template();
        let profile = vehicle_profile_from_components(&template);

        // The components carry the authored synthetic values, not the Increment
        // 0 passenger-car envelope.
        assert!(
            (profile.length_m - 6.4).abs() < 1e-9,
            "length {}",
            profile.length_m
        );
        assert!(
            (profile.width_m - 2.4).abs() < 1e-9,
            "width {}",
            profile.width_m
        );
        assert!(
            (profile.desired_speed_mps - 3.4).abs() < 1e-9,
            "speed {}",
            profile.desired_speed_mps
        );
        assert!((profile.max_accel_mps2 - 0.9).abs() < 1e-9);
        assert!((profile.comfortable_brake_mps2 - 1.6).abs() < 1e-9);
        assert!((profile.time_gap_s - 2.8).abs() < 1e-9);

        let mut sim = synthetic_guide_sim(0);
        assert_eq!(sim.agent_count(), 1, "the harness has one agent");
        // Install the compiled body dimensions and behavior profile.
        sim.agents.profile[0] = Some(profile);
        sim.agents.body_length_m[0] = profile.length_m;
        sim.agents.body_width_m[0] = profile.width_m;
        sim.agents.speed_mps[0] = 0.0;
        sim.spatial.rebuild(&sim.agents);

        let dt = sim.config().step().as_secs();
        let mut previous_speed: f64 = 0.0;
        let mut peak_speed: f64 = 0.0;
        // 400 ticks is 20 s: long enough for the bounded acceleration to bring
        // the vehicle from rest to the altered desired speed and settle there.
        for _ in 0..400 {
            let observation = sim.query_world(0);
            let tactic = sim.choose_tactic(0, &observation);
            let command = sim.command_motion(0, &observation, &tactic, dt);
            let MotionCommand::Longitudinal { speed_mps } = command else {
                panic!("a wheeled agent commands a longitudinal speed");
            };
            // The synthetic desired speed caps every step, and the synthetic
            // acceleration bound caps how fast the speed may rise.
            assert!(
                speed_mps <= profile.desired_speed_mps + 1e-9,
                "the altered speed limit did not bound the command: {speed_mps}"
            );
            assert!(
                speed_mps - previous_speed <= profile.max_accel_mps2 * dt + 1e-9,
                "the altered acceleration bound did not bound the step"
            );
            sim.advance_physics(0, observation, &command, dt);
            previous_speed = sim.agents.speed_mps[0];
            peak_speed = peak_speed.max(previous_speed);
        }
        // The run accelerates from rest and settles at the altered desired
        // speed, so the limit actually binds the motion rather than being inert.
        assert!(
            peak_speed >= profile.desired_speed_mps - 1e-6,
            "the run never reached the synthetic desired speed: {peak_speed}"
        );
        assert!(peak_speed <= profile.desired_speed_mps + 1e-9);
        // The altered body dimension travelled through the shared state.
        assert_eq!(sim.agents.body_length_m[0], profile.length_m);

        // A required stop through the same stage: the altered braking bound
        // caps the command's deceleration.
        let stop = Observation::Vehicle(VehicleObservation {
            profile: Some(profile),
            narrow: None,
            speed_mps: profile.desired_speed_mps,
            leader: None,
            stop_line: Some(Constraint {
                gap_m: 0.5,
                speed_mps: 0.0,
                standstill_m: 0.0,
            }),
            crossing_yield: None,
        });
        let tactic = sim.choose_tactic(0, &stop);
        assert_eq!(tactic.reason, TacticReason::StopLine);
        let command = sim.command_motion(0, &stop, &tactic, dt);
        let MotionCommand::Longitudinal { speed_mps } = command else {
            panic!("a required stop still commands a longitudinal speed");
        };
        let deceleration = (profile.desired_speed_mps - speed_mps) / dt;
        assert!(
            deceleration > 0.0,
            "the stop-line constraint must command braking"
        );
        assert!(
            deceleration <= profile.comfortable_brake_mps2 + 1e-9,
            "the altered braking bound did not cap the command: {deceleration}"
        );
    }

    /// A version-2 bikeway so a capsule rider spawns with route state and a
    /// sampled bounded-steering envelope; its demand puts the rider on the
    /// facility's reference path.
    const BOUNDED_STEERING_V2: &str = r#"
    {
      schema_version: 2,
      id: 'bounded_steering_v2',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
      portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      boundaries: [
        { id: 'world', points: [
          { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
          { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
        ] },
      ],
      regions: [
        { id: 'band', points: [
          { x: 0.0, y: -1.5 }, { x: 200.0, y: -1.5 },
          { x: 200.0, y: 1.5 }, { x: 0.0, y: 1.5 },
        ] },
      ],
      facilities: [
        { id: 'bikeway', region: 'band', reference_path: 'guide',
          width_m: 3.0, nominal_direction: 'forward',
          access: { modes: [ 'rider' ] }, lateral_use: 'shared',
          lateral_policy: { passing_side: 'left' },
          speed_policy: { limit_mps: null } },
      ],
      movements: [
        { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
          direction: 'forward' },
      ],
      mode_templates: [
        {
          id: 'rider',
          body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 },
            radius_m: { min: 0.35, max: 0.35 } },
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield', 'pass' ],
          access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
          occupancy: 'operator_only',
          profiles: {
            speed_mps: { min: 6.0, max: 6.0 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
            lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
            lateral_clearance_m: { min: 0.3, max: 0.3 },
            compliance: { min: 1.0, max: 1.0 },
          },
          lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
        },
      ],
      permissions: [],
      maneuver_policy: {
        commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
      },
      demand: [
        { id: 'rider_inflow', mode: 'rider',
          spawn: { rate: {
            portal: 'entry',
            rate_per_hour: 900.0,
            interval_s: { start_s: 0.0, end_s: null },
            choice: { movements: [ { movement: 'through', weight: 1.0 } ] },
          } } },
      ],
    }
    "#;

    /// A fixed lateral target turns the step into a bounded steering step: the
    /// world pose integrates from the commanded heading and speed, and the
    /// route coordinates are projected back from that pose. The offset closes
    /// continuously over many ticks and never snaps to the target in one.
    #[test]
    fn a_bounded_steering_request_integrates_in_world_and_reprojects_without_snapping() {
        let source =
            parse_scenario_source_v2(BOUNDED_STEERING_V2).expect("the document is version 2");
        let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
        let mut sim = Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds");

        // Advance until the first rider has spawned with route state and a
        // sampled bounded-steering envelope.
        let mut rider = None;
        for _ in 0..1200 {
            sim.step();
            if let Some(index) = (0..sim.agents.len())
                .find(|&index| sim.agents.alive[index] && sim.agents.route_state[index].is_some())
            {
                rider = Some(index);
                break;
            }
        }
        let index = rider.expect("a rider arrives within 60 s");
        let mut state = sim.agents.route_state[index].expect("the rider carries route state");
        assert!(
            state.bounded_steering.is_some(),
            "the rider carries a bounded-steering envelope"
        );
        // A committed maneuver is the state that steers toward a fixed target;
        // this stage-level test drives the steering step directly rather than
        // through the claim batch.
        state.maneuver = ManeuverState::Committed;
        state.target_offset_m = Some(0.6);
        sim.agents.route_state[index] = Some(state);

        let dt = sim.config().step().as_secs();
        let speed_limit = sim.agents.profile[index]
            .expect("a rider profile")
            .desired_speed_mps;
        let direction = sim.agents.direction[index];
        let mut previous_d = sim.agents.route_state[index].expect("route state").d_m;
        let mut first_d = None;
        for _ in 0..200 {
            let before = sim.agents.position[index];
            sim.step_agent(index, dt);

            // The route coordinates are exactly the projection of the
            // integrated world pose: world pose is the truth, projection is the
            // drift check.
            let state = sim.agents.route_state[index].expect("route state survives the step");
            let geometry = sim.route_geometry(index).expect("the facility reference");
            let coordinate = geometry.project(sim.agents.position[index]);
            assert!((coordinate.s() - state.s_m).abs() < 1e-9);
            assert!((coordinate.d() * direction - state.d_m).abs() < 1e-9);

            // The offset advances toward the target continuously and never
            // regresses or jumps further than the bounded step allows.
            assert!(state.d_m >= previous_d - 1e-12, "offset regressed");
            assert!(state.d_m - previous_d <= speed_limit * dt + 1e-9);
            if first_d.is_none() {
                first_d = Some(state.d_m);
            }
            previous_d = state.d_m;
            assert!(sim.agents.position[index] != before, "the pose advanced");
        }

        assert!(
            first_d.expect("at least one step") < 0.1,
            "no one-tick lane-centre snap"
        );
        assert!(
            previous_d > 0.3,
            "the bounded request made continuous progress, at {previous_d} m"
        );
    }

    /// A version-2 bikeway with the given commit policy, no demand, and no
    /// population, so a maneuver test places exactly the riders it needs and
    /// nothing else moves.
    ///
    /// Its horizon is 2.0 s, so a rider's predicted corridor reaches 12 m ahead
    /// at the mode's 6.0 m/s: a body beyond that is a candidate but never a
    /// clearance fact at a corridor sample.
    fn bikeway_v2(min_predicted_clearance_m: f64, hold_timeout_s: f64) -> String {
        format!(
            r#"
    {{
      schema_version: 2,
      id: 'maneuver_v2',
      coordinate_system: {{ x: 'east_m', y: 'north_m' }},
      paths: [
        {{ id: 'guide', points: [ {{ x: 0.0, y: 0.0 }}, {{ x: 200.0, y: 0.0 }} ] }},
        {{ id: 'passed_lane', points: [ {{ x: 0.0, y: -2.4 }}, {{ x: 200.0, y: -2.4 }} ] }},
        {{ id: 'hazard_lane', points: [ {{ x: 0.0, y: 1.6 }}, {{ x: 200.0, y: 1.6 }} ] }},
      ],
      portals: [
        {{ id: 'entry', path: 'guide', end: 'start', width_m: 3.0 }},
        {{ id: 'exit', path: 'guide', end: 'end', width_m: 3.0 }},
      ],
      boundaries: [
        {{ id: 'world', points: [
          {{ x: -10.0, y: -10.0 }}, {{ x: 210.0, y: -10.0 }},
          {{ x: 210.0, y: 10.0 }}, {{ x: -10.0, y: 10.0 }},
        ] }},
      ],
      regions: [
        {{ id: 'band', points: [
          {{ x: 0.0, y: -1.5 }}, {{ x: 200.0, y: -1.5 }},
          {{ x: 200.0, y: 1.5 }}, {{ x: 0.0, y: 1.5 }},
        ] }},
      ],
      facilities: [
        {{ id: 'bikeway', region: 'band', reference_path: 'guide',
          width_m: 3.0, nominal_direction: 'forward',
          access: {{ modes: [ 'rider' ] }}, lateral_use: 'shared',
          lateral_policy: {{ passing_side: 'left' }},
          speed_policy: {{ limit_mps: null }} }},
      ],
      movements: [
        {{ id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
          direction: 'forward' }},
      ],
      mode_templates: [
        {{
          id: 'rider',
          body: {{ kind: 'capsule', length_m: {{ min: 1.8, max: 1.8 }},
            radius_m: {{ min: 0.35, max: 0.35 }} }},
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield', 'pass' ],
          access: {{ facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: {{ limit_mps: null }} }},
          occupancy: 'operator_only',
          profiles: {{
            speed_mps: {{ min: 6.0, max: 6.0 }},
            max_accel_mps2: {{ min: 1.2, max: 1.2 }},
            comfortable_brake_mps2: {{ min: 2.0, max: 2.0 }},
            time_gap_s: {{ min: 1.0, max: 1.0 }},
            steering_rate_max_rad_s: {{ min: 0.9, max: 0.9 }},
            lateral_accel_max_mps2: {{ min: 2.0, max: 2.0 }},
            lateral_clearance_m: {{ min: 0.3, max: 0.3 }},
            compliance: {{ min: 1.0, max: 1.0 }},
          }},
          lateral: {{ target_clearance_m: 0.75, horizon_s: 2.0 }},
        }},
      ],
      permissions: [],
      maneuver_policy: {{
        commit: {{
          min_predicted_clearance_m: {min_predicted_clearance_m},
          hold_timeout_s: {hold_timeout_s},
        }},
      }},
      demand: [
        {{ id: 'none', mode: 'rider',
          spawn: {{ population: {{ path: 'guide', count: 0, speed_mps: 6.0, spacing_m: 20.0 }} }} }},
      ],
    }}
    "#
        )
    }

    /// Build the maneuver fixture at the given commit policy.
    fn rider_sim(min_predicted_clearance_m: f64, hold_timeout_s: f64) -> Simulation {
        let source =
            parse_scenario_source_v2(&bikeway_v2(min_predicted_clearance_m, hold_timeout_s))
                .expect("the document is version 2");
        let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
        Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds")
    }

    /// The fixture's rider mode template. The fixture authors exactly one, so
    /// its dense identifier is the first index.
    fn rider_mode(sim: &Simulation) -> ModeTemplateId {
        let templates = sim.scenario().mode_templates();
        assert_eq!(templates.len(), 1, "the fixture authors one mode");
        assert_eq!(templates[0].family(), Some(AgentFamily::WheeledCapsule));
        ModeTemplateId::from_index(0)
    }

    /// The narrow profile the fixture's template authors.
    fn rider_narrow() -> NarrowProfile {
        NarrowProfile {
            desired_speed_mps: 6.0,
            length_m: 1.8,
            radius_m: 0.35,
            time_gap_s: 1.0,
            max_accel_mps2: 1.2,
            comfortable_brake_mps2: 2.0,
            steering_rate_max_rad_s: 0.9,
            lateral_clearance_m: 0.3,
            lateral_accel_max_mps2: Some(2.0),
            compliance: 1.0,
        }
    }

    /// The rider body at `distance_m` and offset `d_m` on the guide path.
    fn rider_body(sim: &Simulation, distance_m: f64, d_m: f64) -> (DVec2, f64) {
        let path = sim
            .scenario
            .path(PathId::from_index(0))
            .expect("the guide path");
        let heading_rad = path.heading_at(distance_m);
        let normal = DVec2::from_angle(heading_rad + std::f64::consts::FRAC_PI_2);
        (path.position_at(distance_m) + normal * d_m, heading_rad)
    }

    /// Place one rider at `distance_m` on the guide path, offset `d_m` to the
    /// left, through the real spawn projection: its route state carries the
    /// compiled facility's own corridor, target clearance, and horizon.
    fn push_rider(sim: &mut Simulation, distance_m: f64, d_m: f64) -> AgentId {
        let path_id = PathId::from_index(0);
        let (position, heading_rad) = rider_body(sim, distance_m, d_m);
        let narrow = rider_narrow();
        let route_state = sim
            .route_state_for(rider_mode(sim), path_id, position, 1.0, Some(narrow))
            .expect("the rider mode steers on the compiled facility");
        sim.agents.push(AgentInit {
            mode: AgentMode::Vehicle,
            path: path_id,
            distance_m,
            speed_mps: narrow.desired_speed_mps,
            position,
            heading_rad,
            body_length_m: narrow.length_m,
            body_width_m: narrow.body_width_m(),
            direction: 1.0,
            movement: Some(MovementId::from_index(0)),
            profile: Some(narrow.vehicle_profile()),
            narrow_profile: Some(narrow),
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: Some(route_state),
        })
    }

    /// Place a stationary circular body of radius `radius_m` at `distance_m` on
    /// a parallel lane.
    ///
    /// A body on the guide path would be re-projected onto its reference by the
    /// Increment 1 longitudinal advance and lose any lateral offset, so a
    /// fixture body that has to keep a lane offset rides its own parallel lane.
    /// The lane also keeps the body out of the rider's leader selection, so a
    /// hazard test observes the maneuver's own response rather than a following
    /// constraint.
    fn push_obstacle(sim: &mut Simulation, lane: usize, distance_m: f64, radius_m: f64) -> AgentId {
        let path_id = PathId::from_index(lane);
        let path = sim.scenario.path(path_id).expect("the lane").clone();
        sim.agents.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path: path_id,
            distance_m,
            speed_mps: 0.0,
            position: path.position_at(distance_m),
            heading_rad: path.heading_at(distance_m),
            body_length_m: radius_m * 2.0,
            body_width_m: radius_m * 2.0,
            direction: 1.0,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
        })
    }

    /// One rider's route state.
    fn rider_state(sim: &Simulation, agent: AgentId) -> RouteState {
        sim.agents.route_state[agent.index()].expect("the rider carries route state")
    }

    /// The edges of one step's recorded transitions, in order.
    fn step_edges(transitions: &[ManeuverTransition]) -> Vec<ManeuverEdge> {
        transitions
            .iter()
            .map(|transition| transition.edge)
            .collect()
    }

    /// The edge an agent's transition took, or `None` when the step produced no
    /// transition for it.
    fn edge_for(transitions: &[ManeuverTransition], agent: AgentId) -> Option<ManeuverEdge> {
        transitions
            .iter()
            .find(|transition| transition.agent == agent)
            .map(|transition| transition.edge)
    }

    /// The abort reason recorded for an agent, when the step aborted it.
    fn reason_for(
        transitions: &[ManeuverTransition],
        agent: AgentId,
    ) -> Option<ManeuverAbortReason> {
        transitions
            .iter()
            .find(|transition| transition.agent == agent)
            .and_then(|transition| transition.reason)
    }

    /// The target offset every maneuver test steers toward: inside the usable
    /// corridor of the fixture's 3.0 m band and above its band-edge clearance
    /// for the 0.75 m target clearance.
    const MANEUVER_TARGET_M: f64 = 0.3;

    /// The fixture's parallel lane 2.4 m to the rider's right. A body there
    /// clears a corridor reaching `MANEUVER_TARGET_M` at the 0.75 m target
    /// clearance, so it can be the obstacle a maneuver passes.
    const PASSED_LANE: usize = 1;

    /// The fixture's parallel lane 1.6 m to the rider's left. A body there lies
    /// in the lateral space the corridor needs, at a clearance set by its
    /// radius.
    const HAZARD_LANE: usize = 2;

    /// A radius on `HAZARD_LANE` whose closest approach to the corridor leaves a
    /// clearance below the 0.75 m target and above the 0.25 m policy minimum:
    /// the brake-and-hold window.
    const HOLD_OBSTACLE_RADIUS_M: f64 = 0.5;

    /// A radius on `HAZARD_LANE` whose closest approach falls below the policy
    /// minimum.
    const LOST_OBSTACLE_RADIUS_M: f64 = 0.95;

    /// A requested maneuver: the rider and the body it has already passed.
    fn request_maneuver(sim: &mut Simulation, rider: AgentId, passed_body: AgentId) -> bool {
        sim.request_lateral_maneuver(
            rider,
            LateralManeuverRequest {
                target_offset_m: MANEUVER_TARGET_M,
                passed_body,
            },
        )
    }

    /// The whole legal transition table, end to end: `following` enters
    /// `preparing` only once a target and a candidate corridor are fixed,
    /// `preparing` commits only after its claim is granted, the committed rider
    /// returns once the passed obstacle is cleared, and `returning` reaches
    /// `following` back at its own offset. Each edge is recorded once.
    #[test]
    fn a_maneuver_follows_the_legal_transition_table_to_following() {
        let mut sim = rider_sim(0.25, 2.0);
        // The passed obstacle is behind the rider, so the completion guard — the
        // rider's rear envelope at least the target clearance ahead of the
        // obstacle's front — already holds once the claim is granted.
        let rider = push_rider(&mut sim, 60.0, 0.0);
        let passed = push_rider(&mut sim, 50.0, 0.0);
        assert!(request_maneuver(&mut sim, rider, passed));

        // `following -> preparing` fixes the target and the corridor, and
        // displaces nothing: a maneuver prepares before it is granted anything.
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Attempted]);
        let state = rider_state(&sim, rider);
        assert_eq!(state.maneuver, ManeuverState::Preparing);
        assert_eq!(state.target_offset_m, Some(MANEUVER_TARGET_M));
        assert_eq!(state.passed_body, Some(passed));
        assert!(state.corridor.is_some());
        assert!(state.predicted_min_clearance_m.expect("predicted") > 0.75);
        let offset_m = state.d_m;
        assert!(offset_m.abs() < 1e-9, "preparing displaces nothing");

        // A claim is sought at the decision after the attempt, so the rider
        // commits on the next step.
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Committed]);
        assert_eq!(rider_state(&sim, rider).maneuver, ManeuverState::Committed);

        // The target is fixed: the committed rider steers toward it and no
        // command moves the stored target.
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Completed]);
        assert_eq!(rider_state(&sim, rider).maneuver, ManeuverState::Returning);
        assert!(
            rider_state(&sim, rider).d_m > offset_m,
            "the rider displaces"
        );

        // `returning -> following` only once the rider is back at the offset it
        // held before the attempt.
        let mut edges = Vec::new();
        for _ in 0..600 {
            let output = sim.step();
            edges.extend(step_edges(output.transitions()));
            if rider_state(&sim, rider).maneuver == ManeuverState::Following {
                break;
            }
        }
        assert_eq!(edges, [ManeuverEdge::Completed]);
        let state = rider_state(&sim, rider);
        assert_eq!(state.maneuver, ManeuverState::Following);
        assert!(
            (state.d_m - offset_m).abs() <= SETTLE_TOLERANCE_M,
            "the rider returns to its own offset, at {}",
            state.d_m
        );
        assert_eq!(state.target_offset_m, None);
        assert_eq!(state.passed_body, None);
        assert_eq!(state.corridor, None);
        assert_eq!(state.predicted_min_clearance_m, None);
        assert_eq!(sim.emergency_cap_steps(), 0);
    }

    /// Two riders claiming the same corridor are decided as a batch: the winner
    /// commits and the loser aborts with the documented rejection reason,
    /// without displacing at all.
    #[test]
    fn a_losing_claimant_aborts_with_the_rejected_reason() {
        let mut sim = rider_sim(0.25, 2.0);
        let passed = push_rider(&mut sim, 80.0, 0.0);
        let near = push_rider(&mut sim, 60.0, 0.0);
        let far = push_rider(&mut sim, 54.0, 0.0);
        assert!(request_maneuver(&mut sim, far, passed));
        assert!(request_maneuver(&mut sim, near, passed));

        let output = sim.step();
        assert_eq!(
            step_edges(output.transitions()),
            [ManeuverEdge::Attempted, ManeuverEdge::Attempted]
        );
        let output = sim.step();
        assert_eq!(
            edge_for(output.transitions(), near),
            Some(ManeuverEdge::Committed)
        );
        assert_eq!(
            edge_for(output.transitions(), far),
            Some(ManeuverEdge::Aborted)
        );
        assert_eq!(
            reason_for(output.transitions(), far),
            Some(ManeuverAbortReason::ClaimRejected)
        );
        assert_eq!(rider_state(&sim, near).maneuver, ManeuverState::Committed);
        assert_eq!(rider_state(&sim, far).maneuver, ManeuverState::Aborted);
        assert!(
            rider_state(&sim, far).d_m.abs() < 1e-9,
            "a rejected claimant never displaced"
        );
    }

    /// The batch does not depend on the order the requests were recorded in:
    /// the same claimant wins whichever order the intents arrive in.
    #[test]
    fn a_batch_is_decided_independently_of_request_order() {
        let run = |reverse: bool| {
            let mut sim = rider_sim(0.25, 2.0);
            let passed = push_rider(&mut sim, 80.0, 0.0);
            let near = push_rider(&mut sim, 60.0, 0.0);
            let far = push_rider(&mut sim, 54.0, 0.0);
            let requests = if reverse { [far, near] } else { [near, far] };
            for rider in requests {
                assert!(request_maneuver(&mut sim, rider, passed));
            }
            for _ in 0..2 {
                sim.step();
            }
            (
                rider_state(&sim, near).maneuver,
                rider_state(&sim, far).maneuver,
            )
        };
        assert_eq!(
            run(false),
            (ManeuverState::Committed, ManeuverState::Aborted)
        );
        assert_eq!(run(false), run(true));
    }

    /// A same-gap tie — two claims for the same corridor at the same remaining
    /// distance — is decided by the stable agent id, the third clause of the
    /// winner key, whichever way the claims are discovered.
    #[test]
    fn a_same_gap_tie_is_won_by_the_lower_agent_id() {
        let mut sim = rider_sim(0.25, 2.0);
        let passed = push_rider(&mut sim, 80.0, 0.0);
        let first = push_rider(&mut sim, 60.0, 0.0);
        let second = push_rider(&mut sim, 54.0, 0.0);
        // Construct the exact tie directly: the same corridor and the same
        // remaining distance, in `preparing` and due at this decision.
        let corridor = ManeuverCorridor {
            facility: tangle_model::FacilityId::from_index(0),
            s_min_m: 70.0,
            s_max_m: 90.0,
            d_min_m: -0.75,
            d_max_m: 1.05,
        };
        for rider in [first, second] {
            let mut state = rider_state(&sim, rider);
            state.maneuver = ManeuverState::Preparing;
            state.corridor = Some(corridor);
            state.target_offset_m = Some(MANEUVER_TARGET_M);
            state.passed_body = Some(passed);
            state.state_since = Some(SimTime::from_tick(0, DEFAULT_STEP));
            sim.agents.route_state[rider.index()] = Some(state);
        }

        let output = sim.step();
        assert_eq!(
            edge_for(output.transitions(), first),
            Some(ManeuverEdge::Committed)
        );
        assert_eq!(
            edge_for(output.transitions(), second),
            Some(ManeuverEdge::Aborted)
        );
        assert_eq!(
            reason_for(output.transitions(), second),
            Some(ManeuverAbortReason::ClaimRejected)
        );
    }

    /// A commit policy with no recorded intent and no maneuver in flight changes
    /// nothing: every agent stays `following`, no target is fixed, and no
    /// transition is recorded.
    #[test]
    fn a_maneuver_policy_with_no_intent_changes_nothing() {
        let mut sim = rider_sim(0.25, 2.0);
        push_rider(&mut sim, 60.0, 0.0);
        push_rider(&mut sim, 80.0, 0.0);
        for _ in 0..40 {
            let output = sim.step();
            assert!(
                output.transitions().is_empty(),
                "no maneuver was attempted, so no transition exists"
            );
        }
        for index in 0..sim.agents.len() {
            let state = sim.agents.route_state[index].expect("route state");
            assert_eq!(state.maneuver, ManeuverState::Following);
            assert_eq!(state.target_offset_m, None);
            assert_eq!(state.predicted_min_clearance_m, None);
        }
    }

    /// A target that disappears ends the maneuver deterministically with the
    /// documented reason, and the rider returns to `following` at its own
    /// offset.
    #[test]
    fn a_disappearing_target_aborts_the_maneuver() {
        let mut sim = rider_sim(0.25, 2.0);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        let passed = push_rider(&mut sim, 80.0, 0.0);
        assert!(request_maneuver(&mut sim, rider, passed));
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Attempted]);

        // The passed body leaves the world before the claim is decided.
        sim.agents.alive[passed.index()] = false;
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Aborted]);
        assert_eq!(
            reason_for(output.transitions(), rider),
            Some(ManeuverAbortReason::TargetLost)
        );
        assert_eq!(rider_state(&sim, rider).maneuver, ManeuverState::Aborted);

        let mut back_to_following = false;
        for _ in 0..40 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Following {
                back_to_following = true;
                break;
            }
        }
        assert!(
            back_to_following,
            "an aborted maneuver returns to following"
        );
    }

    /// A hold timeout shorter than one decision cadence aborts a preparing
    /// maneuver at the next decision, before the claim can be sought: the
    /// documented timeout transition, and never an implicit fallback.
    #[test]
    fn a_preparing_hold_timeout_aborts_the_maneuver() {
        let mut sim = rider_sim(0.25, 0.01);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        let passed = push_rider(&mut sim, 80.0, 0.0);
        assert!(request_maneuver(&mut sim, rider, passed));
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Attempted]);

        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Aborted]);
        assert_eq!(
            reason_for(output.transitions(), rider),
            Some(ManeuverAbortReason::HoldTimeout)
        );
        let state = rider_state(&sim, rider);
        assert_eq!(state.maneuver, ManeuverState::Aborted);
        assert_eq!(state.target_offset_m, Some(MANEUVER_TARGET_M));
    }

    /// Commit a rider and keep it committed: the passed obstacle is a stationary
    /// body 4 m ahead of it on the parallel lane to its right, so the completion
    /// guard does not hold and the predicted corridor stays feasible.
    /// Returns the rider and the obstacle it is passing.
    fn committed_rider(sim: &mut Simulation) -> (AgentId, AgentId) {
        let rider = push_rider(sim, 60.0, 0.0);
        let passed = push_obstacle(sim, PASSED_LANE, 64.0, 0.5);
        assert!(request_maneuver(sim, rider, passed));
        sim.step();
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Committed]);
        (rider, passed)
    }

    /// A committed maneuver whose predicted clearance drops to the target
    /// clearance brakes within the profile's comfortable braking and holds: it
    /// never accelerates, it aborts nothing, and the brake is not an emergency
    /// cap.
    #[test]
    fn a_committed_maneuver_brakes_within_the_comfort_bound_and_holds() {
        let mut sim = rider_sim(0.25, 2.0);
        let (rider, _passed) = committed_rider(&mut sim);
        // A stationary obstacle in the lateral space the corridor needs, inside
        // the prediction horizon: the closest approach leaves a clearance below
        // the 0.75 m target and above the 0.25 m policy minimum.
        let distance_m = sim.agents.distance_m[rider.index()] + 6.0;
        push_obstacle(&mut sim, HAZARD_LANE, distance_m, HOLD_OBSTACLE_RADIUS_M);

        let speed_before = sim.agents.speed_mps[rider.index()];
        let output = sim.step();
        assert!(
            output.transitions().is_empty(),
            "a brake holds the maneuver instead of ending it"
        );
        let state = rider_state(&sim, rider);
        assert_eq!(state.maneuver, ManeuverState::Committed);
        assert!(state.braking, "the committed maneuver brakes");
        assert!(state.hold_since.is_some(), "and starts its hold");
        let clearance_m = state
            .predicted_min_clearance_m
            .expect("predicted clearance");
        assert!(
            (0.25..0.75).contains(&clearance_m),
            "the hold sits between the policy minimum and the target: {clearance_m}"
        );

        let profile = sim.agents.profile[rider.index()].expect("a rider profile");
        let speed_after = sim.agents.speed_mps[rider.index()];
        let dt = sim.config().step().as_secs();
        let floor = speed_before - profile.comfortable_brake_mps2 * dt;
        assert!(
            speed_after <= speed_before + 1e-12,
            "a braking maneuver never accelerates"
        );
        assert!(
            speed_after >= floor - 1e-9,
            "the brake stays within the comfortable braking: {speed_after} < {floor}"
        );
        assert_eq!(
            sim.emergency_cap_steps(),
            0,
            "a maneuver brake is inside the comfort envelope, not an emergency cap"
        );
    }

    /// A committed maneuver whose predicted clearance falls below the policy's
    /// minimum aborts, without ever reaching the kernel's position caps.
    #[test]
    fn a_committed_maneuver_aborts_below_the_policy_minimum() {
        let mut sim = rider_sim(0.25, 2.0);
        let (rider, _passed) = committed_rider(&mut sim);
        let distance_m = sim.agents.distance_m[rider.index()] + 6.0;
        push_obstacle(&mut sim, HAZARD_LANE, distance_m, LOST_OBSTACLE_RADIUS_M);

        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Aborted]);
        assert_eq!(
            reason_for(output.transitions(), rider),
            Some(ManeuverAbortReason::ClearanceLost)
        );
        assert_eq!(rider_state(&sim, rider).maneuver, ManeuverState::Aborted);
        assert_eq!(sim.emergency_cap_steps(), 0);
    }

    /// A hold that stays at or below the target clearance for the whole timeout
    /// aborts with the timeout reason.
    #[test]
    fn a_committed_hold_timeout_aborts_the_maneuver() {
        let mut sim = rider_sim(0.25, 0.5);
        let (rider, _passed) = committed_rider(&mut sim);
        let distance_m = sim.agents.distance_m[rider.index()] + 6.0;
        push_obstacle(&mut sim, HAZARD_LANE, distance_m, HOLD_OBSTACLE_RADIUS_M);

        let mut aborted_at = None;
        for step in 0..40 {
            let output = sim.step();
            if edge_for(output.transitions(), rider) == Some(ManeuverEdge::Aborted) {
                assert_eq!(
                    reason_for(output.transitions(), rider),
                    Some(ManeuverAbortReason::HoldTimeout)
                );
                aborted_at = Some(step);
                break;
            }
            assert!(
                rider_state(&sim, rider).braking,
                "the hold brakes every step"
            );
        }
        assert!(
            aborted_at.is_some_and(|step| step < 20),
            "the hold times out within its window: {aborted_at:?}"
        );
    }

    /// A winner that loses clearance never hands its corridor to another
    /// claimant in the same step: the committed claimant stays in the batch
    /// while it aborts, the contending claim is rejected, and the space is only
    /// re-arbitrated once the winner has left its maneuver.
    #[test]
    fn losing_clearance_does_not_revoke_another_winner_mid_step() {
        let mut sim = rider_sim(0.25, 2.0);
        let (winner, winner_passed) = committed_rider(&mut sim);

        // A second claimant contests the same corridor: it passes the same body,
        // so its claimed corridor is the same space.
        let claimant = push_rider(&mut sim, 54.0, 0.0);
        assert!(request_maneuver(&mut sim, claimant, winner_passed));
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Attempted]);

        // A hazard the winner alone can see — it lies beyond the claimant's own
        // prediction horizon — removes the winner's clearance in the step the
        // claimant's claim falls due.
        let distance_m = sim.agents.distance_m[winner.index()] + 8.0;
        push_obstacle(&mut sim, HAZARD_LANE, distance_m, LOST_OBSTACLE_RADIUS_M);
        let output = sim.step();
        assert_eq!(
            reason_for(output.transitions(), winner),
            Some(ManeuverAbortReason::ClearanceLost)
        );
        assert_eq!(
            reason_for(output.transitions(), claimant),
            Some(ManeuverAbortReason::ClaimRejected),
            "the loser may not inherit the winner's corridor mid-step"
        );
        assert_eq!(rider_state(&sim, claimant).maneuver, ManeuverState::Aborted);

        // Once the winner has left its maneuver, a fresh intent is granted at
        // the next decision.
        assert!(request_maneuver(&mut sim, claimant, winner_passed));
        let mut committed = false;
        for _ in 0..10 {
            sim.step();
            if rider_state(&sim, claimant).maneuver == ManeuverState::Committed {
                committed = true;
                break;
            }
        }
        assert!(
            committed,
            "the corridor is re-arbitrated once the previous winner is gone"
        );
    }

    /// The request seam refuses an agent that can never maneuver: an agent with
    /// no route state, a bogus target body, and an agent with no compiled commit
    /// policy.
    #[test]
    fn the_request_seam_refuses_an_agent_that_cannot_maneuver() {
        let mut sim = rider_sim(0.25, 2.0);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        let passed = push_rider(&mut sim, 80.0, 0.0);
        assert!(request_maneuver(&mut sim, rider, passed));
        assert!(
            !request_maneuver(&mut sim, rider, rider),
            "an agent cannot pass itself"
        );
        sim.agents.alive[passed.index()] = false;
        assert!(
            !request_maneuver(&mut sim, rider, passed),
            "a despawned target is not a body to displace around"
        );

        // A legacy version-1 walking population carries no route state and no
        // compiled facility, so it has no maneuver to request.
        let mut walking = walking_sim(0);
        assert!(!request_maneuver(
            &mut walking,
            AgentId::from_index(0),
            AgentId::from_index(1)
        ));
        assert!(
            walking.step().transitions().is_empty(),
            "a version-1 run records no maneuver transition"
        );
    }
}
