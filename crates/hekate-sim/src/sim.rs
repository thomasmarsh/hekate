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

use std::cell::Cell;

use glam::DVec2;
use hekate_model::{
    AdjacencySide, AgentFamily, CommitPolicySource, CompiledFacilityAdjacency, CompiledMovement,
    CompiledPath, CompiledPedestrianRoute, CompiledReferencePath, CompiledScenario, CrossingId,
    DemandId, FacilityId, FacilityTraversal, LateralTransition, ModeTemplateId, MovementDirection,
    MovementId, NominalDirection, PassingSide, PathEnd, PathId, PedestrianDemandId,
    PedestrianRouteId, PermissionEffect, PortalId, RuleKind, SignalColor, SignalId, TacticKind,
    TacticalCapability, TraversalTransitions,
};

use crate::agent::{AgentId, AgentInit, AgentMode, AgentStore, RouteState};
use crate::close_pass::{ClosePassTracker, OvertakeObservation};
use crate::compliance::{self, ComplianceDecision, ComplianceReason, SignalAction};
use crate::config::RunConfig;
use crate::control::{Constraint, IDM_STANDSTILL_GAP_M};
use crate::controller::{ControllerModelNames, ControllerModels};
use crate::demand::{DemandRuntime, MAX_PENDING_SPAWNS, sample_pedestrian_route, sample_route};
use crate::event::{DespawnReason, Event, ManeuverReasonCode, ViolationKind};
use crate::index::{self, SpatialIndex};
use crate::metrics::InteractionMetrics;
use crate::narrow::{self, sample_narrow_profile};
use crate::pedestrian::{self, Conflict, PedestrianState, PedestrianWaypoint, PedestrianZone};
use crate::pedestrian_compliance::{self, PedestrianComplianceDecision, PedestrianSignalAction};
use crate::prediction::{
    DEFAULT_SUBDIVISIONS, ManeuverInputs, ManeuverPrediction, PredictedBody,
    predict_crossing_corridor, predict_maneuver_corridor,
};
use crate::profile::{
    PedestrianProfile, VehicleProfile, WheeledLateralLimits, sample_pedestrian_profile,
    sample_profile, sample_wheeled_lateral_limits,
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
    AbortCondition, CorridorClaim, FacilityTransitionRecord, LateralManeuverRequest,
    ManeuverAbortReason, ManeuverCorridor, ManeuverEdge, ManeuverReason, ManeuverState,
    ManeuverTransition, MotionCommand, MotionControl, Observation, PassSide, PedestrianObservation,
    PhysicalAdvance, RelevantWorldQuery, SETTLE_TOLERANCE_M, Tactic, TacticReason, TacticTarget,
    TacticalChoice, TransitionKind, VehicleObservation, arbitrate_claims,
};
use crate::steering::{
    BoundedSteering, CORRIDOR_TOLERANCE_M, LateralCorridor, SteeringLimits, SteeringRequest,
    bounded_steering_step,
};
use crate::time::SimTime;
use crate::units::Seconds;
use crate::wrong_way::{
    self, OpposingTraversalObservation, OpposingTraversalTracker, WrongWayInputs, WrongWayOption,
};

thread_local! {
    /// Diagnostic prediction volume on this thread: the number of
    /// [`Simulation::predict_candidate`] evaluations since the last
    /// [`Simulation::reset_performance_counters`].
    ///
    /// The counter is inert: no decision, event, trace byte, or metric reads it
    /// and it is never serialized. It exists only to size the representative
    /// performance profile (`TAS-110`).
    static PREDICTIONS: Cell<u64> = const { Cell::new(0) };
    /// Diagnostic prediction volume on this thread: the candidate obstacle
    /// bodies those predictions considered, summed since the last
    /// [`Simulation::reset_performance_counters`]. Diagnostic only.
    static PREDICTION_CANDIDATES: Cell<u64> = const { Cell::new(0) };
}

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

/// Tolerance in metres within which a body centre counts as having reached the
/// compiled connector coincidence.
///
/// `docs/schema-v2-contract.md` *Transition targets and the geometric handoff*
/// fixes the connector handoff at the compiled connector coincidence within
/// `CONNECTOR_CONTINUITY_TOLERANCE_M`, and `hekate_model`'s compiler validates
/// that a connector's leaving end and entering end coincide within `1e-6 m`.
/// The kernel reads the same tolerance, so a body that reaches or passes the
/// leaving end within it hands off rather than despawns: a fixed step can carry
/// a body past the exact coincidence by `v * dt`, so the predicate is
/// "reached", not "exactly equal".
pub const CONNECTOR_CONTINUITY_TOLERANCE_M: f64 = 1e-6;

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
    facility_transitions: &'a [FacilityTransitionRecord],
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
    /// One record is produced per state change, and the kernel emits exactly
    /// one [`Event::Maneuver`] for each of them, from the state change this
    /// record is; the record itself keeps the maneuver's own facts.
    pub fn transitions(&self) -> &[ManeuverTransition] {
        self.transitions
    }

    /// The facility handoffs the step performed, in ascending agent id order.
    ///
    /// Each record is the contract's `FacilityTransition` fact, produced once
    /// at the handoff step; `permitted: false` is the forbidden-boundary fact.
    /// The kernel emits exactly one [`Event::FacilityTransition`] per record,
    /// mapped from the record's every field.
    pub fn facility_transitions(&self) -> &[FacilityTransitionRecord] {
        self.facility_transitions
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

/// Diagnostic counters for the representative performance profile
/// (`TAS-110`), read through [`Simulation::performance_counters`].
///
/// They are instrumentation, not simulation state: no decision, event, trace
/// byte, or metric reads them, and they are never serialized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PerformanceCounters {
    /// Candidate bodies the broad-phase queries returned, summed over the run:
    /// the spatial index, the interaction-metrics pass, and the safety monitor
    /// all count here, because they share [`crate::index::BroadPhase`].
    pub broad_phase_candidates: u64,
    /// [`Simulation::predict_candidate`] evaluations: the tactical predictor's
    /// total work per run.
    pub predictions: u64,
    /// Candidate obstacle bodies those predictions considered, summed over the
    /// run: `predictions` times the live population the predictor scans.
    pub prediction_candidates: u64,
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
    /// Close-pass tracking over the same integrated tick: detected overtaking
    /// intervals and the duration each configured clearance band accumulated
    /// over them. See [`crate::close_pass`] for the definitions. Like the
    /// metrics pass it only borrows state and emits nothing, so it cannot change
    /// the simulated trajectory, an event stream, or a golden.
    close_passes: ClosePassTracker,
    /// Opposing-traversal interval tracking over the same integrated tick: the
    /// wrong-way intervals a body's traversal against a facility's rule
    /// direction opens. See [`crate::wrong_way`] for the boundaries. Like the
    /// metrics and close-pass passes it only borrows state, so it cannot change
    /// the simulated trajectory.
    opposing_traversals: OpposingTraversalTracker,
    /// Reused candidate buffer for a crossing-occupancy query, so the query
    /// does not allocate inside the tick loop.
    candidates: Vec<AgentId>,
    events: Vec<Event>,
    /// The maneuver state transitions this step produced, in a deterministic
    /// order. Cleared at the start of every tick, like [`Self::events`], and
    /// exposed on [`StepOutput`]; the kernel emits one [`Event::Maneuver`] per
    /// record.
    transitions: Vec<ManeuverTransition>,
    /// The facility handoffs this step performed, in ascending agent id order.
    /// Cleared at the start of every tick, like [`Self::transitions`], and
    /// exposed on [`StepOutput`]; the kernel emits one
    /// [`Event::FacilityTransition`] per record.
    facility_transitions: Vec<FacilityTransitionRecord>,
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
            close_passes: ClosePassTracker::default(),
            opposing_traversals: OpposingTraversalTracker::default(),
            candidates: Vec::new(),
            events: Vec::new(),
            transitions: Vec::new(),
            facility_transitions: Vec::new(),
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
            facility_transitions: &self.facility_transitions,
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
                        route_state: self.agents.route_state[index].map(|state| {
                            let rule_state = self.wrong_way_rule_state(index, state);
                            RouteStateSample {
                                s_m: state.s_m,
                                d_m: state.d_m,
                                maneuver_state: state.maneuver,
                                target_offset_m: state.target_offset_m,
                                target_facility: state.target_facility,
                                predicted_min_clearance_m: state.predicted_min_clearance_m,
                                target_clearance_m: state.target_clearance_m,
                                horizon_s: state.horizon_s,
                                perceived_rule: rule_state.and_then(|(rule, _)| rule),
                                opposing_direction: rule_state.map(|(_, direction)| direction),
                            }
                        }),
                    }),
                },
            })
            .collect();

        Snapshot::new(self.scenario.id().to_owned(), self.time(), detail, agents)
    }

    /// The wrong-way rule state of one agent, or `None` when it carries none.
    ///
    /// The state is the contract's opposing-traversal rule state and is present
    /// exactly while the agent's body centre lies inside its object's compiled
    /// reference extent and its traversal direction is against the object's
    /// rule direction, the object's authored nominal direction. An `either`
    /// object has no rule direction, a facility without a reference path has no
    /// extent, and nominal travel is not counted at all, so each carries no
    /// state. That is the predicate the opposing-traversal interval opens and
    /// closes on ([`crate::wrong_way::traversal_record`]), so the direction and
    /// rule a row carries agree with the `OpposingTraversal` boundary of the same
    /// tick: the interval is the sparse event record, and this is the per-row
    /// state it marks.
    ///
    /// The value is the direction the agent travels — the direction opposing the
    /// rule — together with the rule it perceived: the most recent wrong-way
    /// decision's own perceived rule when that decision selected the opposing
    /// option on this facility, and otherwise the applicable compiled traversal
    /// policy's nominal effect, exactly as the interval record reads it.
    fn wrong_way_rule_state(
        &self,
        index: usize,
        state: RouteState,
    ) -> Option<(Option<PermissionEffect>, MovementDirection)> {
        let facility = self.scenario.facility(state.facility)?;
        let length_m = facility.reference()?.geometry().length();
        if !(0.0..=length_m).contains(&state.s_m) {
            return None;
        }
        let rule_direction = match facility.nominal_direction() {
            NominalDirection::Forward => MovementDirection::Forward,
            NominalDirection::Reverse => MovementDirection::Reverse,
            NominalDirection::Either => return None,
        };
        let direction = movement_direction(self.agents.direction[index]);
        if direction == rule_direction {
            return None;
        }
        let perceived_rule = state
            .wrong_way_decision
            .filter(|decision| {
                decision.option == WrongWayOption::Opposing && decision.facility == state.facility
            })
            .map(|decision| decision.perceived_rule)
            .unwrap_or_else(|| {
                self.scenario
                    .traversal_policy(
                        state.mode_template,
                        state.facility,
                        self.agents.movement[index],
                    )
                    .and_then(|policy| policy.nominal_effect())
            });
        Some((perceived_rule, direction))
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

    /// Diagnostic performance counters accumulated on this thread since the
    /// last [`Self::reset_performance_counters`].
    ///
    /// These are instrumentation for the representative profile (`TAS-110`),
    /// not simulation state: no decision, event, trace byte, or metric reads
    /// them, they are never serialized, and a run's outcome is identical with
    /// and without them. They are thread-scoped because a `Simulation` is
    /// stepped on the thread that created it.
    pub fn performance_counters() -> PerformanceCounters {
        PerformanceCounters {
            broad_phase_candidates: index::broad_phase_candidates(),
            predictions: PREDICTIONS.with(Cell::get),
            prediction_candidates: PREDICTION_CANDIDATES.with(Cell::get),
        }
    }

    /// Zero this thread's diagnostic performance counters. Instrumentation
    /// only; see [`Self::performance_counters`].
    pub fn reset_performance_counters() {
        index::reset_broad_phase_candidates();
        PREDICTIONS.with(|count| count.set(0));
        PREDICTION_CANDIDATES.with(|count| count.set(0));
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

    /// The most recent lateral maneuver eligibility outcome for an agent.
    ///
    /// Present for a lateral-capable agent the maneuver tactic has evaluated:
    /// [`ManeuverReason::SlowerLeader`] when the tactic recorded an intent for a
    /// visible slower leader, or the precondition that rejected the pass or
    /// overtake. `None` for a pedestrian, a version-1 path follower, a mode with
    /// no free lateral motion, and an agent the tactic has not yet evaluated.
    /// This is the inspectable reason a rejected precondition reports; no public
    /// event is emitted from it.
    pub fn lateral_maneuver_reason(&self, agent: AgentId) -> Option<ManeuverReason> {
        self.agents
            .route_state
            .get(agent.index())
            .copied()
            .flatten()
            .and_then(|state| state.maneuver_reason)
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

    /// Close-pass tracking for the run so far.
    ///
    /// Every completed overtaking interval, with the exact minimum clearance,
    /// its time and relative speed, and the duration each configured clearance
    /// band accumulated over it. See [`crate::close_pass`] for the detection
    /// definition, the band accumulation rule, and the determinism argument.
    pub fn close_pass_tracker(&self) -> &ClosePassTracker {
        &self.close_passes
    }

    /// Close every close-pass observation still open at the run's end.
    ///
    /// A pass still in progress when the run ends terminates with the run, so a
    /// consumer that has performed its last [`Self::step`] calls this once
    /// before reading [`Self::close_pass_tracker`], and every participant pair
    /// has exactly one observation. A run-end closure emits no event: no tick
    /// remains to carry one, so the closed observation is read from the tracker
    /// rather than from a step's event buffer.
    pub fn close_open_close_passes(&mut self) {
        self.close_passes.close_open();
    }

    /// Opposing-traversal interval tracking for the run so far.
    ///
    /// Every closed wrong-way interval, with the perceived rule, the decision
    /// reason, the affected facility and movement, and the legality the
    /// traversal carries. See [`crate::wrong_way`] for the boundaries and the
    /// determinism argument.
    pub fn opposing_traversal_tracker(&self) -> &OpposingTraversalTracker {
        &self.opposing_traversals
    }

    /// Close every wrong-way interval still open at the run's end.
    ///
    /// The run ending is the contract's last close boundary and its close time
    /// is the final simulation time, so a consumer that has performed its last
    /// [`Self::step`] calls this once before reading
    /// [`Self::opposing_traversal_tracker`]. A run-end closure emits no event:
    /// no tick remains to carry one, so the closed interval is read from the
    /// tracker rather than from a step's event buffer.
    pub fn close_open_opposing_traversals(&mut self) {
        self.opposing_traversals.close_open();
    }

    /// The [`Event::ClosePass`] of one closed observation, or `None` when the
    /// contract's applicability does not hold.
    ///
    /// The contract's *Applicability* gives a `ClosePass` record only to a pass
    /// the passing agent's mode carries a `pass` or `overtake` capability for,
    /// on a compiled facility, so a scenario that authors no such capability
    /// emits none. The observation always closes; only its event is gated.
    fn close_pass_event(&self, observation: &OvertakeObservation) -> Option<Event> {
        let facility = observation.facility?;
        let side = observation.side?;
        let mode = self.agents.route_state[observation.agent.index()]?.mode_template;
        let tactics = self.scenario.mode_template(mode)?.tactics();
        if !(tactics.supports(TacticalCapability::Pass)
            || tactics.supports(TacticalCapability::Overtake))
        {
            return None;
        }
        Some(Event::ClosePass {
            agent: observation.agent,
            partner: observation.partner,
            facility,
            side,
            min_clearance_m: observation.min_clearance_m,
            min_clearance_time_s: observation.min_clearance_time_s,
            relative_speed_mps: observation.relative_speed_mps,
            bands: observation.bands.clone(),
            violating_bands: observation.violating_bands.clone(),
            crossed_boundary: observation.crossed_boundary,
            entered_opposing: observation.entered_opposing,
        })
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
        // Facility handoffs are the step's own facts, produced during physical
        // advance; like the maneuver transitions they are recomputed each tick.
        self.facility_transitions.clear();

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
        self.close_passes.begin_tick(&self.agents);

        // The maneuver pass runs before any agent command, so every claim this
        // step is collected from one immutable observation of the tick-start
        // state and arbitrated as a batch: no claim can depend on the order the
        // agents are commanded in, or on a body a later step has already moved.
        // It is inert unless a maneuver is in flight or an intent is recorded.
        self.resolve_maneuvers(dt);

        // The wrong-way entry pass reads the same tick-start state as the
        // maneuver pass, so a selected opposing option moves route and
        // direction ownership before any agent command is produced and no
        // decision reads a pose another decision already moved. It is inert
        // unless the scenario authors `maneuver_policy.wrong_way`.
        self.resolve_wrong_way_entries();

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

        // Close-pass tracking reads the same integrated bodies and the same
        // tick-start shapes, so a detected overtaking interval and its band
        // durations describe exactly the state this tick produced. The kernel
        // emits one [`Event::ClosePass`] for every observation the tick closed.
        let closed_before = self.close_passes.overtakes().len();
        self.close_passes.observe(
            &self.agents,
            &self.scenario,
            &self.facility_transitions,
            self.tick,
            self.config.step(),
        );
        let closed: Vec<Event> = self.close_passes.overtakes()[closed_before..]
            .iter()
            .filter_map(|observation| self.close_pass_event(observation))
            .collect();
        self.events.extend(closed);

        // Opposing-traversal tracking reads the same integrated bodies and the
        // tick-start route state, so an interval's two boundaries describe
        // exactly the state this tick produced. The kernel emits one
        // [`Event::OpposingTraversal`] for each boundary the tick crossed.
        self.opposing_traversals.observe(
            &self.agents,
            &self.scenario,
            self.tick,
            self.config.step(),
        );
        let boundaries: Vec<Event> = self
            .opposing_traversals
            .closed()
            .iter()
            .map(|interval| opposing_traversal_event(interval, false))
            .chain(
                self.opposing_traversals
                    .opened()
                    .iter()
                    .map(|interval| opposing_traversal_event(interval, true)),
            )
            .collect();
        self.events.extend(boundaries);

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

    /// Record a wrong-way entry request for one agent.
    ///
    /// This is the seam a tactical leaf supplies for the contextual wrong-way
    /// decision, mirroring [`Self::request_lateral_maneuver`]: the leaf records
    /// that the agent asks for the decision, and the kernel's wrong-way pass
    /// evaluates the documented decision from the compiled policy and the
    /// tick-start observation and, on a selected opposing option, moves the
    /// agent's route state onto the connected opposing traversal. The kernel
    /// decides, so a requester asks for the entry and names nothing about the
    /// traversal.
    ///
    /// Returns `false`, and records nothing, when the agent can never reach the
    /// decision: it is not alive, it carries no route state, its mode's compiled
    /// tactics carry no `reverse_direction`, the scenario authors no
    /// `maneuver_policy.wrong_way`, or the compiled topology connects no
    /// opposing traversal.
    pub fn request_wrong_way_entry(&mut self, agent: AgentId) -> bool {
        if self.scenario.wrong_way_policy().is_none() {
            return false;
        }
        let index = agent.index();
        if index >= self.agents.len() || !self.agents.alive[index] {
            return false;
        }
        let Some(state) = self.agents.route_state[index] else {
            return false;
        };
        let Some(template) = self.scenario.mode_template(state.mode_template) else {
            return false;
        };
        if !template
            .tactics()
            .supports(TacticalCapability::ReverseNominalDirection)
        {
            return false;
        }
        let current = movement_direction(self.agents.direction[index]);
        if self
            .opposing_traversal_direction(state.facility, current)
            .is_none()
        {
            return false;
        }
        self.agents.route_state[index] = Some(RouteState {
            wrong_way_entry_requested: true,
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
        // The lateral maneuver tactic runs before the lifecycle, so an eligible
        // following agent's intent is consumed by this step's attempt clause in
        // the same batch. `maneuver_candidate` is a cheap scan, so a run with no
        // candidate and no maneuver in flight stays inert without a prediction.
        let has_maneuver_candidate =
            (0..self.agents.len()).any(|index| self.maneuver_candidate(index));
        if !has_maneuver_candidate && !self.maneuver_in_flight() {
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
        if has_maneuver_candidate {
            self.record_maneuver_intents(&batch);
        }
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
                        target_facility: request.target_facility,
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
                // The cross-facility crossing is forbidden by the destination's
                // own policy: the intent is discarded and the inspectable reason
                // is recorded, exactly as the contract's "prevented when
                // avoidable" clause requires.
                Attempt::BoundaryForbidden => plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        intent: None,
                        maneuver_reason: Some(ManeuverReason::BoundaryForbidden),
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
            // Only `preparing` and `committed` seek a claim; `following`,
            // `returning`, and `aborted` have no claim clause. The attempt that
            // puts an agent into a maneuver state requires the mode's resolved
            // target clearance, so a live maneuver always carries one, while an
            // agent that merely carries route state (a lateral-incapable mode on
            // a compiled facility) is skipped here.
            if !matches!(
                state.maneuver,
                ManeuverState::Preparing | ManeuverState::Committed
            ) {
                continue;
            }
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
            // A return leg whose target the ordinary predictor does not clear
            // holds: the agent keeps the offset it occupies and re-decides from
            // the next step's bodies, rather than steering back into the body
            // that obstructs the offset it would return to. It settles only once
            // the same prediction clears the return corridor again, and a return
            // that has nowhere left to move — the agent already sits at its target
            // — completes instead of deadlocking in the maneuver. A change of
            // lane that crossed into the adjacent band returns over the same
            // compiled shared boundary, so its return target is that crossing's
            // own offset and the corridor that holds it is the crossing's own.
            let at_target =
                (state.d_m - self.return_offset_m(index, &state)).abs() <= SETTLE_TOLERANCE_M;
            if !at_target {
                let obstructed = self.return_leg_obstructed(index, &state, &batch);
                plans.push(ManeuverPlan {
                    index,
                    state: RouteState {
                        return_blocked: obstructed,
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
                        return_blocked: false,
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
                    return_blocked: false,
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
            let left = self.agents.route_state[plan.index]
                .expect("a planned maneuver belongs to an agent carrying route state");
            self.agents.route_state[plan.index] = Some(plan.state);
            if let Some(transition) = plan.transition {
                self.transitions.push(transition);
                // The transition is the change this pass already decided, so
                // exactly one event follows from it here and no later step
                // re-derives it: a retry cannot duplicate a transition. The
                // maneuver's facts are fixed at the attempt and never revised,
                // so they are read from the state that carries them — the state
                // the attempt enters, and the state every other edge leaves,
                // which keeps them until the settle edge into `following`
                // clears them.
                let facts = if plan.state.target_offset_m.is_some() {
                    plan.state
                } else {
                    left
                };
                self.events.push(self.maneuver_event(transition, facts));
            }
        }
    }

    /// The `Maneuver` event of one recorded state change.
    ///
    /// The edge, its two states, its agent, and its reason are the recorded
    /// transition's own; the maneuver's other facts are read from the route
    /// state that carries them and fall back to the payload's absent values when
    /// the maneuver has no fixed target — no partner, no target facility, the
    /// agent's own reference centreline as its offset, and the positive-`d` side,
    /// which is the side `narrow::pass_side` resolves an exact tie to. The source
    /// facility is the traversal the maneuver was attempted on: the preserved
    /// `return_facility` of a cross-facility maneuver names it even after the
    /// outbound handoff has moved `facility` to the destination band.
    ///
    /// The reason is the transition's recorded termination reason on an `aborted`
    /// edge, `settled` on a completing edge — the code `TAS-119` added for the
    /// edge that ends the maneuver, which no source enum records — and the closed
    /// set's selection code `slower_leader` on the attempt and commit edges,
    /// because every lateral maneuver this kernel performs displaces past the
    /// passed body the contract calls its target body.
    fn maneuver_event(&self, transition: ManeuverTransition, state: RouteState) -> Event {
        let target_offset_m = state.target_offset_m.unwrap_or(0.0);
        let reason = match transition.reason {
            Some(reason) => ManeuverReasonCode::from(reason),
            None if transition.edge == ManeuverEdge::Completed => ManeuverReasonCode::Settled,
            None => ManeuverReasonCode::SlowerLeader,
        };
        Event::Maneuver {
            agent: transition.agent,
            kind: self.maneuver_kind(state),
            from: transition.from,
            to: transition.to,
            edge: transition.edge,
            partner: state.passed_body,
            source_facility: state.return_facility.unwrap_or(state.facility),
            target_facility: state.target_facility,
            target_offset_m,
            side: if target_offset_m < 0.0 {
                PassSide::Right
            } else {
                PassSide::Left
            },
            reason,
        }
    }

    /// The tactic a recorded lateral maneuver belongs to.
    ///
    /// A maneuver that targets another facility, or whose `return_facility`
    /// records that it has already crossed out of the band it was attempted from,
    /// is the kernel's cross-facility `change_lane`, which is what distinguishes
    /// the `preparing` state that fixed a target facility. The kernel's own
    /// within-facility tactic serves a `pass` and an `overtake` from one code path
    /// and the two differ only in which compiled capability admits them, so the
    /// payload reports `pass` for a mode that compiles it and `overtake`
    /// otherwise — the pair that tactic's own capability guard requires. The kind
    /// reads components and compiled capabilities, never a mode, template, or
    /// scenario name.
    fn maneuver_kind(&self, state: RouteState) -> TacticKind {
        if state.target_facility.is_some() || state.return_facility.is_some() {
            return TacticKind::ChangeLane;
        }
        let tactics = self
            .scenario
            .mode_template(state.mode_template)
            .map(|template| template.tactics());
        if tactics.is_some_and(|tactics| tactics.supports(TacticalCapability::Pass)) {
            TacticKind::Pass
        } else {
            TacticKind::Overtake
        }
    }

    /// The kernel's wrong-way entry pass: evaluate every recorded wrong-way
    /// entry request against one immutable view of the tick-start state and, on
    /// a selected opposing option, move that agent's route state onto the
    /// connected opposing traversal.
    ///
    /// It runs once at the start of a tick, immediately after the maneuver pass
    /// and before any agent command, so every decision reads the same tick-start
    /// state as the maneuver batch and no decision reads a state another
    /// decision wrote. Requests are evaluated in ascending agent id order, and
    /// each agent's draw is the keyed `maneuver`-stream value for its own
    /// decision ordinal ([`crate::wrong_way::maneuver_draw`]), so an agent's
    /// decision never depends on how many other agents were evaluated or in what
    /// order. The pass is inert unless the scenario authors
    /// `maneuver_policy.wrong_way`, so a scenario that authors none has no
    /// wrong-way machinery at all.
    fn resolve_wrong_way_entries(&mut self) {
        if self.scenario.wrong_way_policy().is_none() {
            return;
        }
        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            let Some(state) = self.agents.route_state[index] else {
                continue;
            };
            if !state.wrong_way_entry_requested {
                continue;
            }
            let agent = AgentId::from_index(index);
            let ordinal = state.wrong_way_decisions;
            let draw = wrong_way::maneuver_draw(self.config.seed(), agent, ordinal);
            let decision = self
                .wrong_way_inputs(index, &state)
                .map(|inputs| wrong_way::decide(inputs, draw));
            // Every evaluation consumes its request and advances the agent's
            // decision ordinal, so a later request draws its own value rather
            // than reusing the first, exactly as the decision record fixes. The
            // decision is recorded on the route state too, so the
            // opposing-traversal interval can present the decision's own
            // perceived rule, reason, and affected movement.
            self.agents.route_state[index] = Some(RouteState {
                wrong_way_entry_requested: false,
                wrong_way_decisions: ordinal + 1,
                wrong_way_decision: decision.or(state.wrong_way_decision),
                ..state
            });
            if let Some(decision) = decision
                && decision.option == WrongWayOption::Opposing
            {
                self.enter_opposing_traversal(index);
            }
        }
    }

    /// The decision-instant wrong-way context of one agent, read from the
    /// compiled policy and the agent's tick-start route state.
    ///
    /// Every input is the contract's own: the opposing traversal's physical
    /// connectivity from the compiled topology, the facility's authored nominal
    /// direction, the nominal and opposing remaining lengths and expected
    /// speeds, the observed opposing density, the applicable permission effect,
    /// the agent's sampled compliance and desired speed, and the scenario's
    /// thresholds. The nominal option is the agent's own current travel
    /// direction, which is the rule direction an entering agent holds. `None`
    /// when the traversal has no compiled policy to decide from, so no decision
    /// is taken.
    fn wrong_way_inputs(&self, index: usize, state: &RouteState) -> Option<WrongWayInputs> {
        let facility = self.scenario.facility(state.facility)?;
        let profile = self.agents.profile[index]?;
        let policy = self.scenario.wrong_way_policy()?;
        let reference = facility.reference()?;
        let length_m = reference.geometry().length();

        let nominal = movement_direction(self.agents.direction[index]);
        let opposing = opposite_movement_direction(nominal);
        let opposing_connected = self
            .opposing_traversal_direction(state.facility, nominal)
            .is_some();

        // Remaining length to the leaving end of the reference in each travel
        // direction: a forward traversal leaves at the reference end and a
        // reverse one at its start.
        let remaining = |direction: MovementDirection| match direction {
            MovementDirection::Forward => (length_m - state.s_m).max(0.0),
            MovementDirection::Reverse => state.s_m.max(0.0),
        };
        let nominal_remaining_length_m = remaining(nominal);
        let opposing_remaining_length_m = remaining(opposing);

        // The expected speed is the agent's own desired free-flow speed capped
        // by the facility's enforced limit; the same cap applies to both
        // options, so the time saving is the difference of the two travel
        // times at that speed.
        let limit_mps = facility.speed_policy().limit_mps().unwrap_or(f64::INFINITY);
        let expected_speed_mps = profile.desired_speed_mps.min(limit_mps).max(0.0);

        Some(WrongWayInputs {
            opposing_connected,
            nominal_direction: facility.nominal_direction(),
            nominal_remaining_length_m,
            nominal_expected_speed_mps: expected_speed_mps,
            opposing_remaining_length_m,
            opposing_expected_speed_mps: expected_speed_mps,
            observed_opposing_density_per_km: self.opposing_density_per_km(index, state.facility),
            permission: self
                .scenario
                .traversal_policy(
                    state.mode_template,
                    state.facility,
                    self.agents.movement[index],
                )
                .and_then(|policy| policy.nominal_effect()),
            compliance: profile.compliance,
            desired_speed_mps: profile.desired_speed_mps,
            policy,
            facility: state.facility,
            movement: self.agents.movement[index],
        })
    }

    /// The observed opposing density the wrong-way decision reads: the live
    /// bodies whose traversal on `facility` is oncoming to the opposing
    /// traversal, per kilometre of that facility's compiled reference.
    ///
    /// A body is oncoming when it rides `facility` travelling the rule
    /// direction, which is the traffic an agent entering the opposing traversal
    /// would meet. A facility with no reference, or one of zero length, has no
    /// length to measure against, so the density is zero.
    fn opposing_density_per_km(&self, index: usize, facility: FacilityId) -> f64 {
        let Some(reference) = self
            .scenario
            .facility(facility)
            .and_then(|facility| facility.reference())
        else {
            return 0.0;
        };
        let length_m = reference.geometry().length();
        if length_m <= 0.0 {
            return 0.0;
        }
        let oncoming = self.agents.direction[index];
        let count = (0..self.agents.len())
            .filter(|other| *other != index && self.agents.alive[*other])
            .filter(|other| self.agents.direction[*other] == oncoming)
            .filter(|other| {
                self.agents.route_state[*other].is_some_and(|state| state.facility == facility)
            })
            .count();
        count as f64 / (length_m / 1000.0)
    }

    /// The opposing traversal direction connected to `(facility, direction)` in
    /// the compiled topology, or `None` when the topology connects none.
    ///
    /// The reverse traversal of the same facility is connected when
    /// [`hekate_model::CompiledFacility::physically_possible_directions`] names
    /// it, which a connector leaving or entering along that direction produces;
    /// an adjacency can carry the same connection laterally. The wrong-way
    /// entry moves route ownership on the same reference, so this slice resolves
    /// the same-facility case and a laterally connected opposing traversal is
    /// left to the ordinary lateral maneuver.
    fn opposing_traversal_direction(
        &self,
        facility: FacilityId,
        direction: MovementDirection,
    ) -> Option<MovementDirection> {
        let opposing = opposite_movement_direction(direction);
        self.scenario
            .facility(facility)
            .filter(|compiled| compiled.is_physically_possible(opposing))
            .map(|_| opposing)
    }

    /// Move one agent's route state onto its connected opposing traversal,
    /// through the same one-step ownership move the facility handoffs perform.
    ///
    /// The move changes the travel direction and the route coordinates only: the
    /// world pose is the integrated truth and is never moved, and the new
    /// coordinates are that same pose projected onto the same reference in the
    /// new travel frame, so there is no despawn, no re-spawn, and no snap. The
    /// stored heading stays the reference tangent, exactly as every other stage
    /// keeps it, so the travel heading is [`travel_heading`] of the new sign. The
    /// movement route is left behind, exactly as a handoff does, because the
    /// agent now travels against the movement's own direction. Returns `false`
    /// when no opposing traversal is connected.
    fn enter_opposing_traversal(&mut self, index: usize) -> bool {
        let Some(state) = self.agents.route_state[index] else {
            return false;
        };
        let current = movement_direction(self.agents.direction[index]);
        let Some(opposing) = self.opposing_traversal_direction(state.facility, current) else {
            return false;
        };
        let sign = travel_sign(opposing);
        self.agents.direction[index] = sign;
        self.agents.movement[index] = None;
        self.reproject_route_state(index, sign);
        if let Some(next) = self.agents.route_state[index] {
            self.agents.distance_m[index] = next.s_m;
        }
        true
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

    /// Whether one agent is a candidate for the lateral maneuver tactic this
    /// step: a live `following` agent that carries a bounded-steering envelope,
    /// a target clearance, and a horizon (a lateral-capable mode on a compiled
    /// facility) and has no maneuver or intent already recorded.
    fn maneuver_candidate(&self, index: usize) -> bool {
        self.agents.alive[index]
            && self.agents.route_state[index].is_some_and(|state| {
                state.maneuver == ManeuverState::Following
                    && state.intent.is_none()
                    && state.bounded_steering.is_some()
                    && state.target_clearance_m.is_some()
                    && state.horizon_s.is_some()
            })
    }

    /// Record the lateral maneuver tactic's intent for every eligible
    /// candidate.
    ///
    /// The one tactic covers a within-facility `pass` (a bicycle or scooter
    /// passing a slower narrow user) and an `overtake` (a motor vehicle
    /// displacing past a slower user): both select a deterministic lateral
    /// target on the facility's passing side and name the slower leader they
    /// displace around. Each candidate's decision is a pure function of the
    /// tick-start bodies and route state, so the decisions are computed before
    /// any is written and the result never depends on agent iteration order.
    /// The recorded intent is consumed by this step's `following -> preparing`
    /// attempt clause; a rejected candidate records only its inspectable
    /// [`ManeuverReason`].
    fn record_maneuver_intents(&mut self, batch: &ManeuverBatch<'_>) {
        let decisions: Vec<(usize, ManeuverDecision)> = (0..self.agents.len())
            .filter(|&index| self.maneuver_candidate(index))
            .map(|index| (index, self.maneuver_decision(index, batch)))
            .collect();
        for (index, decision) in decisions {
            let (reason, intent) = match decision {
                ManeuverDecision::Selected {
                    target_offset_m,
                    passed_body,
                } => (
                    ManeuverReason::SlowerLeader,
                    Some(LateralManeuverRequest {
                        target_offset_m,
                        passed_body,
                        // The kernel's own tactic selects a within-facility pass
                        // or overtake; a cross-facility change of lane is a
                        // tactical leaf's request.
                        target_facility: None,
                    }),
                ),
                ManeuverDecision::Rejected(reason) => (reason, None),
            };
            if let Some(state) = self.agents.route_state[index].as_mut() {
                state.maneuver_reason = Some(reason);
                if let Some(intent) = intent {
                    state.intent = Some(intent);
                }
            }
        }
    }

    /// The lateral maneuver tactic: the documented eligibility preconditions
    /// and the deterministic target a within-facility pass or overtake selects.
    ///
    /// The same component-driven tactic serves a `pass` and an `overtake`: the
    /// two differ only in which compiled capability selects them, never in a
    /// mode, body, or scenario name. Every precondition reads a component, a
    /// compiled policy value, or a measured geometric fact — never a mode,
    /// template, or scenario name. The checks run in a fixed order, so a
    /// rejected candidate reports the first precondition that failed:
    ///
    /// 1. capability — the mode's compiled tactics carry [`TacticalCapability::Pass`]
    ///    or [`TacticalCapability::Overtake`];
    /// 2. target clearance and horizon — the mode declares a lateral policy;
    /// 3. applicable permission — no `overtake` statement prohibits passing and
    ///    the facility names a side;
    /// 4. route benefit — a visible slower leader is within the maneuver reach
    ///    and its desired speed is below the agent's;
    /// 5. the deterministic target — the offset that balances the clearance to
    ///    the leader and the clearance to the band edge, on the resolved side,
    ///    so the `self_half_m` term scales the required room with the agent's
    ///    own body width (a wider motor box needs a wider corridor);
    /// 6. sufficient usable width — the balanced clearance reaches the target
    ///    clearance and the target lies inside the usable corridor;
    /// 7. feasible horizon — the ordinary predictor's candidate corridor on the
    ///    selected side stays clear over the whole horizon against the passed
    ///    body, every front, rear, and side body, the facility band edge, and
    ///    the mode's target clearance.
    fn maneuver_decision(&self, index: usize, batch: &ManeuverBatch<'_>) -> ManeuverDecision {
        let state =
            self.agents.route_state[index].expect("a maneuver candidate carries route state");
        // 1. capability: either an explicit `pass` or an explicit `overtake`.
        let Some(template) = self.scenario.mode_template(state.mode_template) else {
            return ManeuverDecision::Rejected(ManeuverReason::Capability);
        };
        let tactics = template.tactics();
        if !tactics.supports(TacticalCapability::Pass)
            && !tactics.supports(TacticalCapability::Overtake)
        {
            return ManeuverDecision::Rejected(ManeuverReason::Capability);
        }
        // 2. target clearance and horizon: the mode's compiled lateral policy.
        let Some(lateral) = template.lateral() else {
            return ManeuverDecision::Rejected(ManeuverReason::NoCorridor);
        };
        let target_clearance_m = lateral.target_clearance_m();
        let horizon_s = lateral.horizon_s();
        let Some(steering) = state.bounded_steering else {
            return ManeuverDecision::Rejected(ManeuverReason::Capability);
        };
        // 3. applicable permission and the facility's passing side.
        let movement = self.agents.movement[index];
        let Some(policy) =
            self.scenario
                .traversal_policy(state.mode_template, state.facility, movement)
        else {
            return ManeuverDecision::Rejected(ManeuverReason::NoPermission);
        };
        if policy.overtake() == Some(PermissionEffect::Prohibit) {
            return ManeuverDecision::Rejected(ManeuverReason::NoPermission);
        }
        let Some(passing_side) = policy.passing_side() else {
            return ManeuverDecision::Rejected(ManeuverReason::NoPermission);
        };
        // 4. route benefit: a visible slower leader inside the maneuver reach.
        let desired_speed_mps =
            self.agents.profile[index].map_or(f64::INFINITY, |profile| profile.desired_speed_mps);
        let Some(leader) = self.maneuver_leader(index) else {
            return ManeuverDecision::Rejected(ManeuverReason::NoBenefit);
        };
        if leader.desired_speed_mps >= desired_speed_mps {
            return ManeuverDecision::Rejected(ManeuverReason::NoBenefit);
        }
        let speed_mps = self.agents.speed_mps[index];
        if leader.gap_m > speed_mps * horizon_s {
            return ManeuverDecision::Rejected(ManeuverReason::NoBenefit);
        }
        // 5. the deterministic target on the selected side: the offset that
        // equalizes the clearance to the passed leader and the clearance to the
        // facility band edge, i.e. the offset maximizing the least of the two.
        let self_half_m = self.agents.body_width_m[index] * 0.5;
        let leader_half_m = leader.body_width_m * 0.5;
        let facility = self.scenario.facility(state.facility);
        let half_width_m = facility.map_or(0.0, |facility| facility.width_m() * 0.5);
        // Along the chosen side's axis, `u = (W/2 + leader_u + leader_half) / 2`,
        // where `leader_u` is the leader's offset measured along that side.
        let target_on = |side: PassSide| {
            let s = side.sign();
            let leader_u = s * leader.offset_m;
            let u = (half_width_m + leader_u + leader_half_m) * 0.5;
            let target_offset_m = s * u;
            let leader_clearance_m = u - leader_u - self_half_m - leader_half_m;
            let band_clearance_m = half_width_m - u - self_half_m;
            let balanced_clearance_m = leader_clearance_m.min(band_clearance_m);
            (target_offset_m, balanced_clearance_m)
        };
        // Resolve the side from policy and geometry; `most_clearance` reads the
        // ordinary predictor's swept clearance for a candidate on each side.
        let (target_offset_m, balanced_clearance_m) = match passing_side {
            PassingSide::Left | PassingSide::Right => {
                target_on(narrow::pass_side(passing_side, 0.0, 0.0))
            }
            PassingSide::MostClearance => {
                let (left_target_m, _) = target_on(PassSide::Left);
                let (right_target_m, _) = target_on(PassSide::Right);
                let left_clearance_m = self.maneuver_clearance(index, left_target_m, batch);
                let right_clearance_m = self.maneuver_clearance(index, right_target_m, batch);
                target_on(narrow::pass_side(
                    passing_side,
                    left_clearance_m,
                    right_clearance_m,
                ))
            }
        };
        // 6. sufficient usable width: the balanced clearance must reach the
        // target clearance and the target must lie inside the usable corridor.
        let corridor_bound_m = steering.corridor.d_max.max(0.0);
        if balanced_clearance_m < target_clearance_m
            || target_offset_m.abs() > corridor_bound_m + CORRIDOR_TOLERANCE_M
        {
            return ManeuverDecision::Rejected(ManeuverReason::InsufficientWidth);
        }
        // 7. feasible horizon: the ordinary predictor's candidate corridor must
        // stay at or above the target clearance over the whole horizon and the
        // usable corridor.
        let Some(prediction) = self.predict_maneuver(index, target_offset_m, batch) else {
            return ManeuverDecision::Rejected(ManeuverReason::NoCorridor);
        };
        if !prediction.is_feasible() {
            return ManeuverDecision::Rejected(ManeuverReason::NoCorridor);
        }
        ManeuverDecision::Selected {
            target_offset_m,
            passed_body: leader.agent,
        }
    }

    /// The predicted minimum swept clearance of a candidate maneuver target, or
    /// negative infinity when no prediction exists (no geometry or envelope).
    fn maneuver_clearance(
        &self,
        index: usize,
        target_offset_m: f64,
        batch: &ManeuverBatch<'_>,
    ) -> f64 {
        self.predict_maneuver(index, target_offset_m, batch)
            .map_or(f64::NEG_INFINITY, |prediction| {
                prediction.clears.swept.clearance_m
            })
    }

    /// The nearest visible leader ahead of one agent on the same facility
    /// traversal and travelling the same way, or `None` when there is none.
    ///
    /// The scan is in ascending [`AgentId`] order and replaces the leader only
    /// for a strictly smaller gap, so two candidates at exactly equal gaps
    /// resolve to the lowest id, exactly as [`Self::nearest_leader`] does. Only
    /// a body with a sampled profile and route state is a maneuver leader, so
    /// the scripted population and a body on another facility are never it.
    fn maneuver_leader(&self, index: usize) -> Option<ManeuverLeader> {
        let state = self.agents.route_state[index]?;
        let direction = self.agents.direction[index];
        let own_progress = direction * state.s_m;
        let own_front = own_progress + self.agents.body_length_m[index] * 0.5;
        let mut best: Option<(usize, f64)> = None;
        for other in 0..self.agents.len() {
            if other == index || !self.agents.alive[other] {
                continue;
            }
            let Some(other_state) = self.agents.route_state[other] else {
                continue;
            };
            if other_state.facility != state.facility
                || self.agents.direction[other] != direction
                || self.agents.profile[other].is_none()
            {
                continue;
            }
            let other_progress = direction * other_state.s_m;
            if other_progress <= own_progress {
                continue;
            }
            let gap = other_progress - self.agents.body_length_m[other] * 0.5 - own_front;
            if best.is_none_or(|(_, best_gap)| gap < best_gap) {
                best = Some((other, gap));
            }
        }
        best.map(|(other, gap)| ManeuverLeader {
            agent: AgentId::from_index(other),
            gap_m: gap.max(0.0),
            desired_speed_mps: self.agents.profile[other]
                .map_or(0.0, |profile| profile.desired_speed_mps),
            body_width_m: self.agents.body_width_m[other],
            offset_m: self.agents.route_state[other].map_or(0.0, |state| state.d_m),
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

    /// Predict one agent's within-facility candidate maneuver corridor and
    /// clearances from the tick-start state, or `None` when the agent carries
    /// no bounded-steering envelope or no compiled facility reference.
    fn predict_maneuver(
        &self,
        index: usize,
        target_offset_m: f64,
        batch: &ManeuverBatch<'_>,
    ) -> Option<ManeuverPrediction> {
        let steering = self.agents.route_state[index]?.bounded_steering?;
        self.predict_candidate(index, target_offset_m, steering, None, batch)
    }

    /// Predict the outbound leg of an in-flight cross-facility change of lane,
    /// or `None` when its route state names no compiled crossing.
    ///
    /// The leg's target is the compiled shared boundary plus the mode's target
    /// clearance in the agent's own travel frame, and its corridor is bounded by
    /// the combined source and destination band edges, so the shared boundary it
    /// crosses is not read as a wall while each band's far edge is. Its
    /// bounded-steering envelope is the widening the crossing motion needs to
    /// reach the boundary.
    fn predict_outbound(
        &self,
        index: usize,
        state: &RouteState,
        batch: &ManeuverBatch<'_>,
    ) -> Option<ManeuverPrediction> {
        let target_facility = state.target_facility?;
        let crossing = self.crossing_lateral(
            state.facility,
            self.agents.direction[index],
            target_facility,
        )?;
        let clearance_m = state.target_clearance_m?;
        let steering = self.steering_envelope(index)?;
        self.predict_candidate(
            index,
            crossing.target_offset_m(clearance_m),
            steering,
            Some(crossing.band_bounds_m),
            batch,
        )
    }

    /// Predict one candidate maneuver: the shared body of the within-facility
    /// and the cross-facility predictor.
    ///
    /// `steering` is the bounded-steering envelope the candidate motion
    /// integrates under, and `band_bounds_m` is a crossing corridor's combined
    /// band edges in the agent's own travel frame, or `None` for the compiled
    /// constant-width band of the facility the agent rides.
    fn predict_candidate(
        &self,
        index: usize,
        target_offset_m: f64,
        steering: BoundedSteering,
        band_bounds_m: Option<[f64; 2]>,
        batch: &ManeuverBatch<'_>,
    ) -> Option<ManeuverPrediction> {
        let state = self.agents.route_state[index]?;
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
        // Diagnostic instrumentation only (`TAS-110`): count this predictor
        // evaluation and the candidate bodies it scans. Nothing below reads
        // these counters.
        PREDICTIONS.with(|count| count.set(count.get() + 1));
        PREDICTION_CANDIDATES.with(|count| count.set(count.get() + others.len() as u64));
        // The bounded-steering envelope is expressed in the agent's own travel
        // frame, so the body it starts from faces the direction of travel. A
        // reverse traveller's stored heading is its reference tangent, so it
        // turns it into the travel heading here exactly as `predicted_bodies`
        // turns the same tangent into its travel velocity.
        let direction = self.agents.direction[index];
        let body = match query::agent_body(&self.agents, index) {
            query::BodyShape::Box {
                centre,
                length_m,
                width_m,
                ..
            } => query::BodyShape::Box {
                centre,
                heading_rad: travel_heading(direction, self.agents.heading_rad[index]),
                length_m,
                width_m,
            },
            circle => circle,
        };
        let inputs = ManeuverInputs {
            geometry,
            facility_width_m: facility.width_m(),
            direction,
            body,
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
        };
        Some(match band_bounds_m {
            Some(bounds) => predict_crossing_corridor(inputs, bounds, &others),
            None => predict_maneuver_corridor(inputs, &others),
        })
    }

    /// The offset one agent's return leg steers toward and settles at, in the
    /// agent's own current travel frame.
    ///
    /// A maneuver that stayed within one band returns to the offset the agent
    /// held before the attempt. A change of lane that has crossed into the
    /// destination band returns over the same compiled adjacency it crossed:
    /// while the agent rides that band the target is the crossing's own steering
    /// offset back over the shared boundary, and `pre_maneuver_offset_m` names
    /// the target again once the return crossing has moved ownership back to the
    /// source band. A return
    /// whose crossing no compiled adjacency carries falls back to that offset,
    /// because there is no crossing to steer to.
    fn return_offset_m(&self, index: usize, state: &RouteState) -> f64 {
        let Some(return_facility) = state.return_facility else {
            return state.pre_maneuver_offset_m;
        };
        let Some(clearance_m) = state.target_clearance_m else {
            return state.pre_maneuver_offset_m;
        };
        self.crossing_lateral(
            state.facility,
            self.agents.direction[index],
            return_facility,
        )
        .map_or(state.pre_maneuver_offset_m, |crossing| {
            crossing.target_offset_m(clearance_m)
        })
    }

    /// Whether a body obstructs the return leg's target, so a `returning` or
    /// `aborted` maneuver holds rather than steering back into it.
    ///
    /// The check is the ordinary predictor's own verdict on the candidate motion
    /// the return leg integrates — the compiled bounded-steering envelope
    /// steering from the current pose to the return offset — so no second
    /// geometry path decides it. A maneuver that stayed within one band reads the
    /// compiled constant-width band of the facility it rides. A change of lane
    /// that has crossed out reads the compiled crossing corridor back over the
    /// adjacency it crossed, bounded by the two bands' combined edges, so a body
    /// anywhere in the corridor the return sweeps holds the return rather than
    /// only a body inside the destination band's own compiled interval.
    ///
    /// A body the return corridor sweeps closer than the mode's target clearance
    /// over the horizon holds the return. The band edge alone never does: the
    /// corridor the return integrates is the one the bounded step keeps it
    /// inside, and the compiled band edge a return crosses back over is the
    /// shared boundary itself, so a body centre still on it would otherwise hold
    /// a return that has nowhere to go but home. The entry transient is the one
    /// leg with no verdict: the just-entered band's own corridor does not hold
    /// the body centre yet, so a return that stays in that band has no ordinary
    /// prediction to read.
    fn return_leg_obstructed(
        &self,
        index: usize,
        state: &RouteState,
        batch: &ManeuverBatch<'_>,
    ) -> bool {
        let Some(steering) = self.steering_envelope(index) else {
            return false;
        };
        let Some(target_clearance_m) = state.target_clearance_m else {
            return false;
        };
        let band_bounds_m = match state.return_facility {
            Some(return_facility) => {
                let Some(crossing) = self.crossing_lateral(
                    state.facility,
                    self.agents.direction[index],
                    return_facility,
                ) else {
                    return false;
                };
                Some(crossing.band_bounds_m)
            }
            None => {
                if state.entering_facility {
                    return false;
                }
                None
            }
        };
        let Some(prediction) = self.predict_candidate(
            index,
            self.return_offset_m(index, state),
            steering,
            band_bounds_m,
            batch,
        ) else {
            return false;
        };
        // The classes partition the bodies, so folding over the front, side, and
        // rear facts is exactly the least clearance of every body the corridor
        // sweeps, without the band-edge fact the swept minimum also carries.
        [
            prediction.clears.front,
            prediction.clears.side,
            prediction.clears.rear,
        ]
        .into_iter()
        .flatten()
        .any(|fact| fact.clearance_m < target_clearance_m)
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
        // A request that names a destination facility is a cross-facility change
        // of lane; it crosses the compiled adjacency's shared boundary rather
        // than displacing within one band.
        if let Some(target_facility) = request.target_facility
            && target_facility != state.facility
        {
            return self.attempt_facility_transition(index, state, request, target_facility);
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

    /// Fix a cross-facility change of lane's target and candidate corridor, or
    /// report why it is not admissible this step.
    ///
    /// The compiled adjacency names the destination traversal, the crossing
    /// side, and the [shared boundary](CompiledFacilityAdjacency::shared_boundary_offset)
    /// the handoff fires on, so the crossing bound is the compiled geometric
    /// datum rather than the source band's half-width. The destination traversal's
    /// own policy answers whether the continuation direction is permitted — a
    /// destination it does not permit is the contract's preventable
    /// forbidden-boundary crossing — and the destination target is validated
    /// against the destination facility's own usable interval. The world pose
    /// stays the truth at the handoff, where progress is the projection of the
    /// unchanged pose onto the destination reference.
    fn attempt_facility_transition(
        &self,
        index: usize,
        state: &RouteState,
        request: &LateralManeuverRequest,
        target_facility: FacilityId,
    ) -> Attempt {
        let direction = self.agents.direction[index];
        // No compiled adjacency joins the current band to the target, so there
        // is no crossing to attempt.
        let Some(crossing) = self.crossing_lateral(state.facility, direction, target_facility)
        else {
            return Attempt::Infeasible;
        };
        let destination_traversal = crossing.transition.target();
        if !self.traversal_permits(
            state.mode_template,
            destination_traversal.facility(),
            destination_traversal.direction(),
            None,
        ) {
            return Attempt::BoundaryForbidden;
        }
        let Some(source) = self.scenario.facility(state.facility) else {
            return Attempt::Infeasible;
        };
        let Some(destination) = self.scenario.facility(destination_traversal.facility()) else {
            return Attempt::Infeasible;
        };
        let Some(target_clearance_m) = state.target_clearance_m else {
            return Attempt::Infeasible;
        };
        let Some(geometry) = self.route_geometry(index) else {
            return Attempt::Infeasible;
        };
        let envelope = self.agents.body_width_m[index];
        // The destination target must lie inside the destination band's own
        // usable interval; a band that cannot hold it offers no corridor.
        let interval = destination.usable_lateral_interval(envelope, target_clearance_m);
        if interval.d_max() < 0.0
            || request.target_offset_m < interval.d_min() - CORRIDOR_TOLERANCE_M
            || request.target_offset_m > interval.d_max() + CORRIDOR_TOLERANCE_M
        {
            return Attempt::Infeasible;
        }
        // The claimed corridor spans the passed obstacle's footprint and the
        // source band's usable interval extended to just past the compiled
        // shared boundary on the crossing side, in the facility's own frame.
        let passed = request.passed_body.index();
        let passed_s_m = geometry.project(self.agents.position[passed]).s();
        let half_self_m = self.agents.body_length_m[index] * 0.5;
        let half_passed_m = self.agents.body_length_m[passed] * 0.5;
        let crossing_bound = direction * crossing.target_offset_m(target_clearance_m);
        let source_interval = source.usable_lateral_interval(envelope, target_clearance_m);
        let mut d_min_m = source_interval.d_min();
        let mut d_max_m = source_interval.d_max();
        if crossing_bound > 0.0 {
            d_max_m = d_max_m.max(crossing_bound);
        } else {
            d_min_m = d_min_m.min(crossing_bound);
        }
        // The predicted clearance is the destination band edge's clearance at
        // the destination target, in the destination band's own frame.
        let predicted_clearance_m =
            destination.width_m() * 0.5 - request.target_offset_m.abs() - envelope * 0.5;
        Attempt::Admissible {
            corridor: ManeuverCorridor {
                facility: state.facility,
                s_min_m: passed_s_m - half_passed_m - half_self_m - target_clearance_m,
                s_max_m: passed_s_m + half_passed_m + half_self_m + target_clearance_m,
                d_min_m,
                d_max_m,
            },
            predicted_clearance_m,
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
        // The leg's prediction: the ordinary within-facility corridor for a
        // within-facility maneuver, the outbound crossing corridor while a
        // change of lane is in flight, and none while the agent is still on the
        // entry transient of the destination it has just been handed off to —
        // that leg is bounded by the corridor until its body centre is inside
        // the destination's usable interval. A crossing that no compiled
        // adjacency carries any more has no corridor left.
        let cross_facility = state
            .target_facility
            .is_some_and(|target| target != state.facility);
        let prediction = if state.entering_facility {
            None
        } else if cross_facility {
            match self.predict_outbound(index, &state, batch) {
                Some(prediction) => Some(prediction),
                None => {
                    return self.abort_plan(
                        index,
                        state,
                        ManeuverAbortReason::CorridorInfeasible,
                        batch.now,
                    );
                }
            }
        } else {
            match self.predict_maneuver(index, target_offset_m, batch) {
                Some(prediction) => Some(prediction),
                None => {
                    return self.abort_plan(
                        index,
                        state,
                        ManeuverAbortReason::CorridorInfeasible,
                        batch.now,
                    );
                }
            }
        };

        if self.passed_body_cleared(index, passed, target_clearance_m) {
            return ManeuverPlan {
                index,
                state: RouteState {
                    maneuver: ManeuverState::Returning,
                    predicted_min_clearance_m: prediction
                        .as_ref()
                        .map(|prediction| prediction.clears.swept.clearance_m),
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
        // The entry transient has no prediction yet: it holds its crossing and
        // lets the bounded entry leg carry the body centre into the destination.
        let Some(prediction) = prediction else {
            return ManeuverPlan {
                index,
                state,
                transition: None,
            };
        };
        let swept_m = prediction.clears.swept.clearance_m;

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
    /// it crosses ([`hekate_model::CompiledCrossing::movements`]) and a
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

    /// Nearest live leader ahead on the same path in this agent's direction of
    /// travel, and, for an agent whose active maneuver crosses to another
    /// facility, the nearest leader on the far side of that crossing too.
    ///
    /// The gap is bumper to bumper along the path. Iterating in ascending agent
    /// order means the lowest agent id wins a tie, which keeps tie-breaking
    /// stable across runs. A body travelling the reference the other way is a
    /// leader too, because it is ahead in this agent's travel frame; its
    /// constraint speed is the leader's speed along that same axis, so a
    /// head-on body closes at the sum of the two speeds. Crossing-path
    /// interaction is later work.
    ///
    /// A body on the far side of an active cross-facility maneuver is a leader
    /// before the handoff moves ownership there, so both facilities' leader
    /// constraints stay active through the handoff and no gap opens across the
    /// ownership change: the destination band of a change of lane constrains the
    /// approach, and the band a `returning` or `aborted` maneuver crosses back
    /// to constrains the return. Its world pose is projected onto the facility
    /// the agent rides, exactly as the crossing predictor projects every body it
    /// reads, so both sides' gaps are measured in the one travel frame the
    /// agent's own corridor lives in.
    fn nearest_leader(&self, index: usize) -> Option<(AgentId, Constraint)> {
        let direction = self.agents.direction[index];
        let own_path = self.agents.path[index];
        let own_progress = direction * self.agents.distance_m[index];
        let own_front = own_progress + self.agents.body_length_m[index] * 0.5;
        // A body an active lateral maneuver is passing is not a longitudinal
        // leader: the maneuver's own corridor clearance governs the approach to
        // it, so the passing agent displaces alongside and overtakes it. The
        // body remains a collision and safety subject through the ordinary
        // passes.
        let passed_body = self.agents.route_state[index].and_then(|state| match state.maneuver {
            ManeuverState::Committed | ManeuverState::Returning | ManeuverState::Aborted => {
                state.passed_body
            }
            ManeuverState::Following | ManeuverState::Preparing => None,
        });
        // The traversal this agent's active maneuver crosses to, when it makes
        // one, and the ridden facility's reference the far side's bodies are
        // read through.
        let far_side = self.crossing_far_traversal(index);
        let ridden_geometry = far_side.and_then(|_| self.route_geometry(index));
        let mut best: Option<(usize, f64)> = None;
        for other in 0..self.agents.len() {
            if other == index || !self.agents.alive[other] {
                continue;
            }
            if passed_body == Some(AgentId::from_index(other)) {
                continue;
            }
            let other_progress = if self.agents.path[other] == own_path {
                direction * self.agents.distance_m[other]
            } else if let Some(far_side) = far_side
                && self.agents.direction[other] == travel_sign(far_side.direction())
                && self.agents.route_state[other]
                    .is_some_and(|state| state.facility == far_side.facility())
                && let Some(geometry) = ridden_geometry
            {
                let progress = direction * geometry.project(self.agents.position[other]).s();
                // A body on the far side is a *following* leader only once it is
                // genuinely ahead: its rear envelope past this agent's front. A
                // body alongside or overlapping the agent still lies in the lane
                // the crossing would enter, but no longitudinal step can ever
                // touch it while the agent rides its own band, so the anti-overlap
                // cap must not freeze the agent for it: the crossing's own corridor
                // clearance governs that body, and holds or aborts the maneuver
                // instead.
                if progress - self.agents.body_length_m[other] * 0.5 <= own_front {
                    continue;
                }
                progress
            } else {
                continue;
            };
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
                    // The leader's speed along the agent's own travel axis. A
                    // body riding the same reference the other way closes on
                    // this agent at the sum of the two speeds, so a same-way
                    // leader keeps its own speed and an oncoming one carries the
                    // opposite sign. That closure is what the longitudinal
                    // model's closing-speed term reads, so a wrong-way rider's
                    // approach to oncoming traffic is bounded by the ordinary
                    // leader constraint rather than by the anti-overlap cap.
                    speed_mps: direction
                        * self.agents.direction[other]
                        * self.agents.speed_mps[other],
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
        let mut profile_rng = derive_stream(self.config.seed(), STREAM_PROFILE, agent_id.get());
        let mut compliance_rng =
            derive_stream(self.config.seed(), STREAM_COMPLIANCE, agent_id.get());
        let narrow_profile = match mode.and_then(|id| self.scenario.mode_template(id)) {
            Some(template) if template.family() == Some(AgentFamily::WheeledCapsule) => Some(
                sample_narrow_profile(template, &mut profile_rng, &mut compliance_rng),
            ),
            _ => None,
        };
        let profile = match narrow_profile {
            Some(narrow) => narrow.vehicle_profile(),
            None => sample_profile(
                self.scenario.profiles(),
                &mut profile_rng,
                &mut compliance_rng,
            ),
        };
        // The bounded-steering limits a mode steers under come from its own
        // compiled profile. A capsule already carries them on its narrow
        // profile; a wheeled box samples them from its mode template, so a
        // lateral-capable car gets an envelope without a mode-id branch. A
        // version-1 source and a box with no `lateral` object carry none.
        let lateral = match narrow_profile {
            Some(narrow) => narrow.lateral_limits(),
            None => mode
                .and_then(|id| self.scenario.mode_template(id))
                .and_then(|template| sample_wheeled_lateral_limits(template, &mut profile_rng)),
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
            self.route_state_for(mode, path_id, position, direction, profile.width_m, lateral)
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
    /// declares one, and its sampled bounded-steering limits seed the usable
    /// corridor for its own body width; `None` leaves each of them absent
    /// exactly as Increment 1.
    fn route_state_for(
        &self,
        mode: ModeTemplateId,
        path: PathId,
        position: DVec2,
        direction: f64,
        body_width_m: f64,
        lateral: Option<WheeledLateralLimits>,
    ) -> Option<RouteState> {
        let template = self.scenario.mode_template(mode)?;
        if template.family() == Some(AgentFamily::HolonomicCircle) {
            return None;
        }
        let facility = self.scenario.facilities().iter().find(|facility| {
            facility.reference_path() == Some(path) && facility.permits_mode(mode)
        })?;
        let geometry = facility.reference()?.geometry();
        let policy = template.lateral();
        let state = RouteState::project(
            mode,
            facility.id(),
            geometry,
            position,
            direction,
            policy.map(|policy| policy.target_clearance_m()),
            policy.map(|policy| policy.horizon_s()),
        );
        // Only a mode whose sampled limits carry a lateral-acceleration bound
        // can steer laterally; the corridor is the facility's usable interval
        // for its own envelope width and preferred clearance. A mode with no
        // compiled limit stays longitudinal-only.
        let bounded = lateral.map(|lateral| {
            let interval =
                facility.usable_lateral_interval(body_width_m, lateral.lateral_clearance_m);
            BoundedSteering {
                limits: SteeringLimits {
                    heading_rate_max_rad_s: lateral.heading_rate_max_rad_s,
                    lateral_accel_max_mps2: lateral.lateral_accel_max_mps2,
                },
                corridor: LateralCorridor {
                    d_min: interval.d_min(),
                    d_max: interval.d_max(),
                },
            }
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

    /// Whether `mode` may travel `direction` on `facility`, which is the legal
    /// question behind [`FacilityTransitionRecord::permitted`]: a destination
    /// traversal the applicable rule does not permit is the contract's forbidden
    /// boundary crossing.
    fn traversal_permits(
        &self,
        mode: ModeTemplateId,
        facility: FacilityId,
        direction: MovementDirection,
        movement: Option<MovementId>,
    ) -> bool {
        self.scenario
            .traversal_policy(mode, facility, movement)
            .is_some_and(|policy| policy.permits(direction))
    }

    /// The compiled adjacency joining two bands, in either authored order, or
    /// `None` when no authored adjacency declares the pair.
    fn facility_adjacency(
        &self,
        first: FacilityId,
        second: FacilityId,
    ) -> Option<&CompiledFacilityAdjacency> {
        self.scenario
            .facility_adjacencies()
            .iter()
            .find(|adjacency| {
                (adjacency.first() == first && adjacency.second() == second)
                    || (adjacency.first() == second && adjacency.second() == first)
            })
    }

    /// The compiled geometry of one cross-facility lateral crossing from
    /// `facility` travelling at `direction`, or `None` when no authored
    /// adjacency joins the two bands.
    ///
    /// The adjacency's shared boundary is the contract's lateral handoff point:
    /// [`CrossingLateral::boundary_offset_m`] is that boundary's signed offset
    /// from the source reference in the agent's own travel frame, where the body
    /// centre crosses. `shared_boundary_offset` on both bands gives the offset
    /// between the two references, so the destination band's compiled
    /// constant-width band lands in the source band's travel frame and the
    /// outbound corridor's outer bounds are the two bands' combined edges with
    /// the shared boundary between them open — one compiled geometric datum
    /// where the source band's half-width was approximated.
    fn crossing_lateral(
        &self,
        facility: FacilityId,
        direction: f64,
        target_facility: FacilityId,
    ) -> Option<CrossingLateral> {
        if target_facility == facility {
            return None;
        }
        let travel = movement_direction(direction);
        let transition =
            lateral_transition(self.scenario.transitions(facility, travel), target_facility)?;
        let adjacency = self.facility_adjacency(facility, target_facility)?;
        let boundary_offset_m = adjacency.shared_boundary_offset(facility, travel)?;
        let source = self.scenario.facility(facility)?;
        let destination = self.scenario.facility(target_facility)?;
        let to_direction = transition.target().direction();
        let destination_boundary_m =
            adjacency.shared_boundary_offset(target_facility, to_direction)?;
        // The destination reference's offset from the source reference is the
        // difference of the two boundary offsets, so the destination band's own
        // constant-width band lands in the source band's travel frame.
        let shift_m = boundary_offset_m - destination_boundary_m;
        let source_half_m = source.width_m() * 0.5;
        let destination_half_m = destination.width_m() * 0.5;
        Some(CrossingLateral {
            transition,
            side: travel_pass_side(transition.side()),
            boundary_offset_m,
            band_bounds_m: [
                (-source_half_m).min(shift_m - destination_half_m),
                source_half_m.max(shift_m + destination_half_m),
            ],
        })
    }

    /// The traversal across the compiled shared boundary of an agent's active
    /// cross-facility maneuver, or `None` when the agent makes no crossing.
    ///
    /// The target facility is fixed when the attempt fixes it, so a `preparing`
    /// maneuver already knows the traversal its approach is closing on, a
    /// `committed` one knows the traversal it crosses into, and a `returning` or
    /// `aborted` one knows the band it crosses back to. Reading the far side
    /// from the first of those states is what keeps the destination leader and
    /// the body closing from behind active across the whole approach and through
    /// the handoff, rather than only once ownership has already moved.
    fn crossing_far_traversal(&self, index: usize) -> Option<FacilityTraversal> {
        let state = self.agents.route_state[index]?;
        let target_facility = match state.maneuver {
            ManeuverState::Following => return None,
            ManeuverState::Preparing | ManeuverState::Committed => state.target_facility,
            ManeuverState::Returning | ManeuverState::Aborted => state.return_facility,
        }?;
        if target_facility == state.facility {
            return None;
        }
        self.crossing_lateral(
            state.facility,
            self.agents.direction[index],
            target_facility,
        )
        .map(|crossing| crossing.transition.target())
    }

    /// Record one facility handoff this step performed: the step's own fact on
    /// the exposed record buffer, and the one [`Event::FacilityTransition`] for
    /// it.
    ///
    /// The event maps the record's every field, so the event and
    /// [`FacilityTransitionRecord`] never disagree, and it is pushed at the
    /// handoff that performed the change — the change itself, not a per-step
    /// poll of it — so exactly one event exists per handoff and no later step can
    /// re-emit one.
    fn record_facility_transition(&mut self, record: FacilityTransitionRecord) {
        let event = Event::FacilityTransition {
            agent: record.agent,
            from_facility: record.from_facility,
            to_facility: record.to_facility,
            from_direction: record.from_direction,
            to_direction: record.to_direction,
            via: record.via,
            side: record.side,
            s_m: record.s_m,
            d_m: record.d_m,
            permitted: record.permitted,
        };
        self.facility_transitions.push(record);
        self.events.push(event);
    }

    /// The contract's connector handoff, performed when the agent has reached
    /// the leaving end of its traversal.
    ///
    /// The transition target is the connector that leaves the agent's current
    /// `(facility, direction)` traversal at its coincidence
    /// ([`CONNECTOR_CONTINUITY_TOLERANCE_M`]); when several leave, the first in
    /// authored order is taken, which is the compiled order the contract keeps.
    /// The handoff updates route and facility ownership in one step while the
    /// world pose is unchanged: no despawn, no re-spawn, and no snap to the
    /// destination reference, because the destination progress is the projection
    /// of that same pose onto the destination reference. Returns `false` when the
    /// agent carries no route state, no connector continues its traversal, or the
    /// destination offers no reference, so the ordinary path exit still applies.
    fn handoff_connector(&mut self, index: usize) -> bool {
        let Some(state) = self.agents.route_state[index] else {
            return false;
        };
        let from_direction = movement_direction(self.agents.direction[index]);
        let Some(connector) = self
            .scenario
            .transitions(state.facility, from_direction)
            .and_then(|transitions| transitions.longitudinal().first().copied())
            .and_then(|id| self.scenario.facility_connector(id))
        else {
            return false;
        };
        let to = connector.to();
        if to.facility() == state.facility {
            return false;
        }
        let Some(destination) = self.scenario.facility(to.facility()) else {
            return false;
        };
        let Some(reference) = destination.reference() else {
            return false;
        };
        let Some(path) = destination.reference_path() else {
            return false;
        };
        let to_direction = to.direction();
        let travel_sign = travel_sign(to_direction);
        let position = self.agents.position[index];
        let coordinate = reference.geometry().project(position);
        let permitted =
            self.traversal_permits(state.mode_template, to.facility(), to_direction, None);
        let side = connector_crossing_side(
            travel_heading(self.agents.direction[index], self.agents.heading_rad[index]),
            travel_heading(travel_sign, reference.geometry().heading_at(coordinate.s())),
        );

        self.agents.path[index] = path;
        self.agents.distance_m[index] = coordinate.s();
        self.agents.direction[index] = travel_sign;
        self.agents.heading_rad[index] = reference.geometry().heading_at(coordinate.s());
        // The route was a movement on the source path; the destination is a
        // facility traversal, so the movement route is left behind rather than
        // carried onto a path it does not describe.
        self.agents.movement[index] = None;
        let mut next = state;
        next.facility = to.facility();
        next.s_m = coordinate.s();
        next.d_m = coordinate.d() * travel_sign;
        self.agents.route_state[index] = Some(next);

        self.record_facility_transition(FacilityTransitionRecord {
            agent: AgentId::from_index(index),
            from_facility: state.facility,
            to_facility: to.facility(),
            from_direction,
            to_direction,
            via: TransitionKind::Connector,
            side,
            s_m: state.s_m,
            d_m: state.d_m,
            permitted,
        });
        true
    }

    /// Perform the contract's lateral handoff when a committed cross-facility
    /// change of lane's body centre has crossed the compiled shared boundary
    /// between the two bands, or when a `returning` or `aborted` one has crossed
    /// back over that same boundary to the band the maneuver was attempted from.
    ///
    /// The compiled adjacency names the destination traversal, the crossing
    /// side, and the shared boundary, so the crossing is the body centre
    /// reaching [`CrossingLateral::crossing_m`] on that side. The handoff moves
    /// route and facility ownership in one step while the world pose is
    /// unchanged — the destination progress is the projection of that same pose
    /// onto the destination reference, so there is no despawn, no re-spawn, and
    /// no snap — and the maneuver continues on the destination with its target
    /// and its policy re-expressed in the destination's own frame. The source
    /// band and the offset the agent held before the attempt are preserved
    /// across the outbound move, so the return leg crosses back over the same
    /// adjacency instead of settling in the band the maneuver passed through,
    /// and the return move clears that intent again so a change of lane crosses
    /// out and back exactly once. Returns `false` when no committed, returning,
    /// or aborted cross-facility maneuver is crossing.
    fn handoff_lateral(&mut self, index: usize) -> bool {
        let Some(state) = self.agents.route_state[index] else {
            return false;
        };
        // A committed change of lane crosses out to the destination traversal it
        // was attempted for; a returning or aborted one crosses back to the band
        // it was attempted from.
        let (target_facility, returning) = match state.maneuver {
            ManeuverState::Committed => match state.target_facility {
                Some(target_facility) => (target_facility, false),
                None => return false,
            },
            ManeuverState::Returning | ManeuverState::Aborted => match state.return_facility {
                Some(return_facility) => (return_facility, true),
                None => return false,
            },
            ManeuverState::Following | ManeuverState::Preparing => return false,
        };
        let from_direction = movement_direction(self.agents.direction[index]);
        let Some(crossing) = self.crossing_lateral(
            state.facility,
            self.agents.direction[index],
            target_facility,
        ) else {
            return false;
        };
        let side = crossing.side;
        if side.sign() * state.d_m < crossing.crossing_m() - CONNECTOR_CONTINUITY_TOLERANCE_M {
            return false;
        }
        let destination_traversal = crossing.transition.target();
        let Some(destination) = self.scenario.facility(destination_traversal.facility()) else {
            return false;
        };
        let Some(reference) = destination.reference() else {
            return false;
        };
        let Some(path) = destination.reference_path() else {
            return false;
        };
        let permitted = self.traversal_permits(
            state.mode_template,
            destination_traversal.facility(),
            destination_traversal.direction(),
            None,
        );
        let to_direction = destination_traversal.direction();
        let to_sign = travel_sign(to_direction);
        let coordinate = reference.geometry().project(self.agents.position[index]);

        self.agents.path[index] = path;
        self.agents.distance_m[index] = coordinate.s();
        self.agents.direction[index] = to_sign;
        self.agents.heading_rad[index] = reference.geometry().heading_at(coordinate.s());
        self.agents.movement[index] = None;
        let mut next = state;
        next.facility = destination_traversal.facility();
        next.s_m = coordinate.s();
        next.d_m = coordinate.d() * to_sign;
        // The return leg settles at the source band's own offset again once the
        // return crossing has carried ownership back, and no further crossing
        // is owed. On the outbound crossing the maneuver keeps the source band
        // and the offset the agent held before the attempt, so the return leg
        // crosses back over this same adjacency rather than settling in the
        // destination frame. Either way the body centre is still on the shared
        // boundary, so the entry transient holds the ordinary destination
        // corridor back until it is inside that band's usable interval.
        if returning {
            next.return_facility = None;
        } else {
            next.target_facility = None;
            next.return_facility = Some(state.facility);
        }
        next.entering_facility = true;
        self.agents.route_state[index] = Some(next);

        self.record_facility_transition(FacilityTransitionRecord {
            agent: AgentId::from_index(index),
            from_facility: state.facility,
            to_facility: destination_traversal.facility(),
            from_direction,
            to_direction,
            via: TransitionKind::Lateral,
            side,
            s_m: state.s_m,
            d_m: state.d_m,
            permitted,
        });
        true
    }

    /// Clear the entry-transient flag once a laterally transitioned agent's body
    /// centre is inside the destination band's compiled usable interval, so the
    /// ordinary corridor and predictor govern again.
    fn settle_facility_entry(&mut self, index: usize) {
        let Some(mut state) = self.agents.route_state[index] else {
            return;
        };
        if !state.entering_facility {
            return;
        }
        let inside = state.bounded_steering.is_none_or(|steering| {
            let offset = state.d_m * self.agents.direction[index];
            offset >= steering.corridor.d_min - CORRIDOR_TOLERANCE_M
                && offset <= steering.corridor.d_max + CORRIDOR_TOLERANCE_M
        });
        if inside {
            state.entering_facility = false;
            self.agents.route_state[index] = Some(state);
        }
    }

    /// An agent's lateral steering target for this step, when its maneuver
    /// displaces it.
    ///
    /// The target is the tactical stage's request; this leaf only integrates it
    /// under the compiled limits and never selects it. A `committed` maneuver
    /// steers toward its fixed target — except on the outbound leg of a
    /// cross-facility change of lane, where it steers just past the shared
    /// boundary — a `returning` or `aborted` one steers back to the offset the
    /// agent held before the attempt, or back over the compiled shared boundary
    /// when the maneuver has crossed into the adjacent band, and a `preparing` or
    /// `following` agent displaces nothing at all: a preparing maneuver has fixed
    /// a target but has not been granted a corridor for it. A return leg the
    /// ordinary predictor holds obstructed steers to the offset the agent already
    /// occupies instead, so it holds its place rather than displacing into the
    /// body the return corridor is not clear of.
    fn lateral_request(&self, index: usize) -> Option<f64> {
        let state = self.agents.route_state.get(index).copied().flatten()?;
        match state.maneuver {
            ManeuverState::Committed => self.committed_lateral_target(index, &state),
            ManeuverState::Returning | ManeuverState::Aborted => Some(if state.return_blocked {
                state.d_m
            } else {
                self.return_offset_m(index, &state)
            }),
            ManeuverState::Following | ManeuverState::Preparing => None,
        }
    }

    /// The target offset a committed maneuver steers toward: just past the
    /// compiled shared boundary on the crossing side of a cross-facility change
    /// of lane, or the fixed destination target within one band.
    fn committed_lateral_target(&self, index: usize, state: &RouteState) -> Option<f64> {
        if let Some(target_facility) = state.target_facility
            && target_facility != state.facility
        {
            let clearance = state.target_clearance_m?;
            let crossing = self.crossing_lateral(
                state.facility,
                self.agents.direction[index],
                target_facility,
            )?;
            return Some(crossing.target_offset_m(clearance));
        }
        state.target_offset_m
    }

    /// The compiled bounded-steering envelope of an agent this step, widened on
    /// the crossing side while it crosses the compiled shared boundary of a
    /// cross-facility change of lane — on the outbound committed leg toward the
    /// destination traversal, and again on the `returning` or `aborted` leg back
    /// to the band the maneuver was attempted from — so the bounded step may
    /// reach the crossing's own target.
    fn steering_envelope(&self, index: usize) -> Option<BoundedSteering> {
        let state = self.agents.route_state.get(index).copied().flatten()?;
        let mut steering = state.bounded_steering?;
        // The crossing the maneuver is making this step: the committed leg's
        // destination traversal, or the returning leg's source band.
        let crossing_facility = match state.maneuver {
            ManeuverState::Committed => state.target_facility,
            ManeuverState::Returning | ManeuverState::Aborted => state.return_facility,
            ManeuverState::Following | ManeuverState::Preparing => None,
        }
        .filter(|facility| *facility != state.facility);
        if let Some(crossing_facility) = crossing_facility
            && let Some(clearance) = state.target_clearance_m
            && let Some(crossing) = self.crossing_lateral(
                state.facility,
                self.agents.direction[index],
                crossing_facility,
            )
        {
            let bound = self.agents.direction[index] * crossing.target_offset_m(clearance);
            if bound > 0.0 {
                steering.corridor.d_max = steering.corridor.d_max.max(bound);
            } else {
                steering.corridor.d_min = steering.corridor.d_min.min(bound);
            }
        }
        // While the body centre is still inside the just-entered band's shared
        // boundary, the entry leg's corridor is widened to where the body is, so
        // the bounded step may move it into the destination's usable interval.
        if state.entering_facility {
            let entry = state.d_m * self.agents.direction[index];
            if entry > 0.0 {
                steering.corridor.d_max = steering.corridor.d_max.max(entry);
            } else {
                steering.corridor.d_min = steering.corridor.d_min.min(entry);
            }
        }
        Some(steering)
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

/// The compiled geometry of one cross-facility lateral crossing, read from the
/// adjacency that joins the source band to the destination band.
///
/// Every offset is in the agent's own travel frame: `d` is positive to the left
/// of its direction of travel. The shared boundary is the contract's lateral
/// handoff point, and the two band bounds are the outer edges the outbound
/// predicted corridor is limited by — the source band's far edge on one side and
/// the destination band's far edge on the other, with the shared boundary
/// between them open because the two compiled bands are adjacent.
struct CrossingLateral {
    /// The compiled lateral transition: the destination traversal that
    /// continues the agent's travel, and the crossing side.
    transition: LateralTransition,
    /// The side of the crossing in the agent's own travel frame.
    side: PassSide,
    /// The shared boundary's signed offset from the source reference in the
    /// agent's own travel frame.
    boundary_offset_m: f64,
    /// The outer edges of the two bands combined, in the agent's own travel
    /// frame as `[low, high]`.
    band_bounds_m: [f64; 2],
}

impl CrossingLateral {
    /// The shared boundary's distance from the source reference on the side the
    /// agent crosses toward.
    fn crossing_m(&self) -> f64 {
        self.side.sign() * self.boundary_offset_m
    }

    /// The outbound steering target in the agent's own travel frame: just past
    /// the compiled shared boundary by the mode's target clearance, so the
    /// bounded step reaches and crosses the handoff point.
    fn target_offset_m(&self, clearance_m: f64) -> f64 {
        self.side.sign() * (self.crossing_m() + clearance_m)
    }
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
    /// A cross-facility change of lane would enter a traversal the applicable
    /// rule does not permit. The intent is discarded and the inspectable reason
    /// is the contract's `boundary_forbidden`: the preventable half of a
    /// forbidden-boundary crossing.
    BoundaryForbidden,
}

/// One candidate's view of the visible slower leader ahead it might pass or
/// overtake.
///
/// Every field is a component, a compiled policy value, or a measured geometric
/// fact, so a maneuver decision never reads a mode or scenario name.
struct ManeuverLeader {
    /// The leader's stable identifier, which becomes the maneuver's passed body.
    agent: AgentId,
    /// Bumper-to-bumper gap ahead along the reference in metres.
    gap_m: f64,
    /// The leader's sampled desired free-flow speed in m/s.
    desired_speed_mps: f64,
    /// The leader's body width in metres.
    body_width_m: f64,
    /// The leader's signed lateral offset in metres, in the shared travel frame.
    offset_m: f64,
}

/// The lateral maneuver tactic's outcome for one agent this step.
enum ManeuverDecision {
    /// A pass or overtake is selected: the deterministic target offset (whose
    /// sign is the resolved side in the agent's own travel frame) and the
    /// slower leader it displaces around.
    Selected {
        /// The target signed offset in metres, in the agent's own travel frame.
        target_offset_m: f64,
        /// The slower leader the maneuver passes or overtakes.
        passed_body: AgentId,
    },
    /// No maneuver: the precondition that failed, an inspectable
    /// [`ManeuverReason`].
    Rejected(ManeuverReason),
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

/// The world heading a wheeled agent travels along.
///
/// A wheeled body's stored heading is its reference tangent — the same tangent
/// `predicted_bodies` turns into a velocity with the travel sign — so a reverse
/// traveller faces the opposite way. The bounded-steering envelope is expressed
/// in the travel frame, so every steering call and prediction converts here.
fn travel_heading(direction: f64, reference_rad: f64) -> f64 {
    if direction < 0.0 {
        wrap_pi(reference_rad + std::f64::consts::PI)
    } else {
        reference_rad
    }
}

/// The stored reference-tangent heading of a wheeled body travelling at
/// `travel_rad`, the inverse of [`travel_heading`].
fn reference_heading(direction: f64, travel_rad: f64) -> f64 {
    if direction < 0.0 {
        wrap_pi(travel_rad - std::f64::consts::PI)
    } else {
        travel_rad
    }
}

/// The [`Event::OpposingTraversal`] record of one interval boundary.
///
/// The contract gives an `OpposingTraversal` record to a traversal against a
/// rule direction, which is exactly what an interval is, so every boundary of
/// an interval carries one: `entering` is `true` at the open boundary and
/// `false` at the close boundary, and the record's other fields are the ones
/// the interval latched when it opened.
fn opposing_traversal_event(interval: &OpposingTraversalObservation, entering: bool) -> Event {
    Event::OpposingTraversal {
        agent: interval.agent,
        facility: interval.facility,
        movement: interval.movement,
        direction: interval.direction,
        nominal_direction: interval.nominal_direction,
        perceived_rule: interval.perceived_rule,
        reason: interval.reason,
        violating: interval.violating,
        entering,
    }
}

/// The compiled traversal direction an agent's travel sign names.
fn movement_direction(sign: f64) -> MovementDirection {
    if sign < 0.0 {
        MovementDirection::Reverse
    } else {
        MovementDirection::Forward
    }
}

/// The traversal direction opposite `direction`: the opposing traversal of the
/// same facility traversal.
fn opposite_movement_direction(direction: MovementDirection) -> MovementDirection {
    match direction {
        MovementDirection::Forward => MovementDirection::Reverse,
        MovementDirection::Reverse => MovementDirection::Forward,
    }
}

/// The longitudinal travel sign a compiled traversal direction names.
fn travel_sign(direction: MovementDirection) -> f64 {
    if direction == MovementDirection::Reverse {
        -1.0
    } else {
        1.0
    }
}

/// The side of a connector crossing in the agent's own travel frame: the side
/// the destination travel heading turns toward, with an exact straight
/// continuation resolving to `left` so the record is always deterministic.
fn connector_crossing_side(incoming_rad: f64, destination_rad: f64) -> PassSide {
    if wrap_pi(destination_rad - incoming_rad) < 0.0 {
        PassSide::Right
    } else {
        PassSide::Left
    }
}

/// The resolved travel-frame side of a compiled adjacency side.
fn travel_pass_side(side: AdjacencySide) -> PassSide {
    match side {
        AdjacencySide::Left => PassSide::Left,
        AdjacencySide::Right => PassSide::Right,
    }
}

/// The compiled lateral transition of a traversal that reaches
/// `target_facility`, or `None` when no adjacency joins the two bands.
fn lateral_transition(
    transitions: Option<&TraversalTransitions>,
    target_facility: FacilityId,
) -> Option<LateralTransition> {
    transitions?
        .lateral()
        .iter()
        .copied()
        .find(|transition| transition.target().facility() == target_facility)
}

/// Wrap an angle in radians to `(-pi, pi]`, so a heading stays canonical however
/// long a run is.
fn wrap_pi(angle_rad: f64) -> f64 {
    let mut wrapped = (angle_rad + std::f64::consts::PI) % std::f64::consts::TAU;
    if wrapped <= 0.0 {
        wrapped += std::f64::consts::TAU;
    }
    wrapped - std::f64::consts::PI
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
                    && let Some(steering) = self.steering_envelope(index)
                    && let Some(geometry) = self.route_geometry(index)
                {
                    let request = SteeringRequest {
                        position: self.agents.position[index],
                        heading_rad: travel_heading(
                            self.agents.direction[index],
                            self.agents.heading_rad[index],
                        ),
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
                        // A compiled connector continues the route: the agent
                        // hands off through the coincidence instead of leaving
                        // the world. The handoff is only reached when it carries
                        // route state and a connector leaves its traversal.
                        if self.handoff_connector(index) {
                            return;
                        }
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
                    // The commanded heading is the travel heading; the stored
                    // heading stays the reference tangent, as the longitudinal
                    // stage keeps it, so every reader sees one convention.
                    self.agents.heading_rad[index] = reference_heading(direction, heading_rad);
                    self.agents.position[index] = position;
                    self.reproject_route_state(index, direction);
                    if let Some(state) = self.agents.route_state[index] {
                        self.agents.distance_m[index] = state.s_m;
                    }

                    // A committed change of lane crosses the shared band boundary
                    // mid-step: the handoff moves ownership to the destination
                    // band without moving the pose, before any route-end check.
                    if self.handoff_lateral(index) {
                        return;
                    }
                    self.settle_facility_entry(index);

                    let s_m = self.agents.distance_m[index];
                    if s_m <= 0.0 || s_m >= length {
                        if self.handoff_connector(index) {
                            return;
                        }
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
    use crate::narrow::{self, NarrowProfile};
    use crate::prediction::LimitingObject;
    use crate::units::Seconds;
    use hekate_model::{
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
        use hekate_model::BodyKind;

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
    /// `hekate-model`'s compilation test.
    const SYNTHETIC_TEMPLATE_FIXTURE: &str =
        include_str!("../../hekate-model/tests/fixtures/synthetic_mode_template_v2.json5");

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
    /// (`crates/hekate-sim/tests/synthetic_template_no_branch.rs`) fails if the
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

    /// A version-2 wide road with a lateral-capable box mode (`passenger_car`, an
    /// `overtake` template with a `lateral` object) and a plain box mode
    /// (`plain_car`, no `lateral`), so the route-state seed for a box body can
    /// be exercised without a demand schedule.
    const CAR_LATERAL_V2: &str = r#"
    {
      schema_version: 2,
      id: 'car_lateral_v2',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0.0, y: 0.0 }, { x: 200.0, y: 0.0 } ] } ],
      portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 10.0 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 10.0 },
      ],
      boundaries: [
        { id: 'world', points: [
          { x: -10.0, y: -10.0 }, { x: 210.0, y: -10.0 },
          { x: 210.0, y: 10.0 }, { x: -10.0, y: 10.0 },
        ] },
      ],
      regions: [
        { id: 'band', points: [
          { x: 0.0, y: -5.0 }, { x: 200.0, y: -5.0 },
          { x: 200.0, y: 5.0 }, { x: 0.0, y: 5.0 },
        ] },
      ],
      facilities: [
        { id: 'road', region: 'band', reference_path: 'guide',
          width_m: 10.0, nominal_direction: 'forward',
          access: { modes: [ 'passenger_car', 'plain_car' ] }, lateral_use: 'shared',
          lateral_policy: { passing_side: 'left' },
          speed_policy: { limit_mps: null } },
      ],
      movements: [
        { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0,
          direction: 'forward' },
      ],
      mode_templates: [
        {
          id: 'passenger_car',
          body: { kind: 'box', length_m: { min: 4.5, max: 4.5 },
            width_m: { min: 1.8, max: 1.8 } },
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield', 'overtake' ],
          access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
          occupancy: 'operator_only',
          profiles: {
            speed_mps: { min: 9.0, max: 9.0 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
            lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
            lateral_clearance_m: { min: 0.3, max: 0.3 },
            compliance: { min: 1.0, max: 1.0 },
          },
          lateral: { target_clearance_m: 0.75, horizon_s: 2.0 },
        },
        {
          id: 'plain_car',
          body: { kind: 'box', length_m: { min: 4.5, max: 4.5 },
            width_m: { min: 1.8, max: 1.8 } },
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield' ],
          access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
          occupancy: 'operator_only',
          profiles: {
            speed_mps: { min: 9.0, max: 9.0 },
            max_accel_mps2: { min: 1.2, max: 1.2 },
            comfortable_brake_mps2: { min: 2.0, max: 2.0 },
            time_gap_s: { min: 1.0, max: 1.0 },
            compliance: { min: 1.0, max: 1.0 },
          },
        },
      ],
      permissions: [],
      maneuver_policy: {
        commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
      },
      demand: [],
    }
    "#;

    /// A lateral box mode seeds a bounded-steering envelope and the mode's
    /// target clearance and horizon, while a plain box mode on the same facility
    /// carries route state but no envelope: the device that lets a motor vehicle
    /// overtake without a mode-id branch.
    #[test]
    fn a_lateral_box_mode_seeds_a_bounded_steering_envelope_and_a_plain_box_does_not() {
        let source = parse_scenario_source_v2(CAR_LATERAL_V2).expect("the document is version 2");
        let scenario = CompiledScenario::compile_v2(source).expect("the scenario compiles");
        let sim = Simulation::new(scenario, RunConfig::new(0)).expect("the simulation builds");
        let mode_of = |id: &str| {
            let templates = sim.scenario().mode_templates();
            let index = templates
                .iter()
                .position(|template| template.id() == id)
                .unwrap_or_else(|| panic!("the fixture authors '{id}'"));
            ModeTemplateId::from_index(index)
        };
        let path = PathId::from_index(0);
        let position = DVec2::new(10.0, 0.0);

        let lateral = WheeledLateralLimits {
            heading_rate_max_rad_s: 0.9,
            lateral_accel_max_mps2: 2.0,
            lateral_clearance_m: 0.3,
        };
        let state = sim
            .route_state_for(
                mode_of("passenger_car"),
                path,
                position,
                1.0,
                1.8,
                Some(lateral),
            )
            .expect("the lateral box steers on the compiled facility");
        assert_eq!(state.target_clearance_m, Some(0.75));
        assert_eq!(state.horizon_s, Some(2.0));
        let bounded = state
            .bounded_steering
            .expect("a lateral box carries an envelope");
        assert!((bounded.limits.heading_rate_max_rad_s - 0.9).abs() < 1e-9);
        assert!((bounded.limits.lateral_accel_max_mps2 - 2.0).abs() < 1e-9);
        // Half width 5.0 minus half body 0.9 minus clearance 0.3.
        assert!((bounded.corridor.d_max - 3.8).abs() < 1e-9);
        assert!((bounded.corridor.d_min + 3.8).abs() < 1e-9);

        let state = sim
            .route_state_for(mode_of("plain_car"), path, position, 1.0, 1.8, None)
            .expect("a plain box still carries route state on the facility");
        assert!(state.bounded_steering.is_none());
        assert_eq!(state.target_clearance_m, None);
        assert_eq!(state.horizon_s, None);
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
        {{ id: 'return_lane', points: [ {{ x: 0.0, y: -0.7 }}, {{ x: 200.0, y: -0.7 }} ] }},
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
            .route_state_for(
                rider_mode(sim),
                path_id,
                position,
                1.0,
                narrow.body_width_m(),
                narrow.lateral_limits(),
            )
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
        push_travelling_obstacle(sim, lane, distance_m, radius_m, 0.0)
    }

    /// Place a circular body of radius `radius_m` at `distance_m` on a parallel
    /// lane, travelling at `speed_mps` in the lane's own direction.
    ///
    /// A body that holds station alongside the rider keeps its clearance against
    /// the rider's predicted corridor for the whole horizon, so a return-leg
    /// obstruction is a stable state to observe rather than one the rider's own
    /// travel races past.
    fn push_travelling_obstacle(
        sim: &mut Simulation,
        lane: usize,
        distance_m: f64,
        radius_m: f64,
        speed_mps: f64,
    ) -> AgentId {
        let path_id = PathId::from_index(lane);
        let path = sim.scenario.path(path_id).expect("the lane").clone();
        sim.agents.push(AgentInit {
            mode: AgentMode::Pedestrian,
            path: path_id,
            distance_m,
            speed_mps,
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

    /// The fixture's parallel lane 0.7 m to the rider's right: a body there
    /// overlaps the progress interval of a rider returning to its own offset at
    /// a clearance below the 0.75 m target, and clears the corridor a rider
    /// committed to the 0.3 m target occupies.
    const RETURN_LANE: usize = 3;

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
                target_facility: None,
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

    /// The `Maneuver` events one step emitted, in buffer order.
    fn maneuver_events(output: &StepOutput<'_>) -> Vec<Event> {
        output
            .events()
            .iter()
            .filter(|event| matches!(event, Event::Maneuver { .. }))
            .cloned()
            .collect()
    }

    /// One `Maneuver` event per recorded transition, and the event maps the
    /// record's edge and the maneuver's own fixed facts: the passed body, the
    /// source facility, the target offset, and the side. An abort carries its
    /// termination reason and a completing edge carries `settled`, and a step
    /// that records no transition emits no event, so no state a maneuver holds
    /// can re-emit one.
    #[test]
    fn a_maneuver_transition_emits_one_event_carrying_the_maneuver_facts() {
        let mut sim = rider_sim(0.25, 2.0);
        // The passed obstacle is behind the rider, so the completion guard
        // already holds once the claim is granted.
        let rider = push_rider(&mut sim, 60.0, 0.0);
        let passed = push_rider(&mut sim, 50.0, 0.0);
        assert!(request_maneuver(&mut sim, rider, passed));
        let fact = |from, to, edge, reason| Event::Maneuver {
            agent: rider,
            kind: TacticKind::Pass,
            from,
            to,
            edge,
            partner: Some(passed),
            source_facility: FacilityId::from_index(0),
            target_facility: None,
            target_offset_m: MANEUVER_TARGET_M,
            side: PassSide::Left,
            reason,
        };

        // `following -> preparing`: the attempt is the step that fixes the
        // target and the passed body, and the selection reason is the attempt's.
        let output = sim.step();
        assert_eq!(
            maneuver_events(&output),
            [fact(
                ManeuverState::Following,
                ManeuverState::Preparing,
                ManeuverEdge::Attempted,
                ManeuverReasonCode::SlowerLeader,
            )]
        );

        // `preparing -> committed` on the granted claim.
        let output = sim.step();
        assert_eq!(
            maneuver_events(&output),
            [fact(
                ManeuverState::Preparing,
                ManeuverState::Committed,
                ManeuverEdge::Committed,
                ManeuverReasonCode::SlowerLeader,
            )]
        );

        // `committed -> returning`, the completing edge, carries `settled`.
        let output = sim.step();
        assert_eq!(
            maneuver_events(&output),
            [fact(
                ManeuverState::Committed,
                ManeuverState::Returning,
                ManeuverEdge::Completed,
                ManeuverReasonCode::Settled,
            )]
        );

        // `returning -> following` settles the maneuver, and the facts of the
        // maneuver that ended are still the ones the event reports.
        let mut settled = None;
        for _ in 0..600 {
            let output = sim.step();
            settled = maneuver_events(&output).first().cloned();
            if settled.is_some() {
                break;
            }
        }
        assert_eq!(
            settled,
            Some(fact(
                ManeuverState::Returning,
                ManeuverState::Following,
                ManeuverEdge::Completed,
                ManeuverReasonCode::Settled,
            ))
        );

        // A step that records no transition emits no `Maneuver` event: the event
        // is the transition, never a per-step poll of the maneuver state.
        for _ in 0..5 {
            let output = sim.step();
            assert!(output.transitions().is_empty());
            assert!(maneuver_events(&output).is_empty());
        }
        assert_eq!(sim.emergency_cap_steps(), 0);
    }

    /// A transition whose maneuver has no fixed target reports the payload's
    /// absent values: no partner, no target facility, the agent's own reference
    /// centreline as its offset, and the positive-`d` side.
    #[test]
    fn a_transition_with_no_fixed_target_reports_the_payload_defaults() {
        let mut sim = rider_sim(0.25, 2.0);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        push_rider(&mut sim, 80.0, 0.0);
        // A `preparing` maneuver that never fixed a target or a passed body: its
        // due claim finds no target and aborts.
        let mut state = rider_state(&sim, rider);
        state.maneuver = ManeuverState::Preparing;
        state.state_since = Some(SimTime::from_tick(0, DEFAULT_STEP));
        state.corridor = Some(ManeuverCorridor {
            facility: FacilityId::from_index(0),
            s_min_m: 60.0,
            s_max_m: 70.0,
            d_min_m: -1.0,
            d_max_m: 1.0,
        });
        sim.agents.route_state[rider.index()] = Some(state);

        let output = sim.step();
        assert_eq!(
            maneuver_events(&output),
            [Event::Maneuver {
                agent: rider,
                kind: TacticKind::Pass,
                from: ManeuverState::Preparing,
                to: ManeuverState::Aborted,
                edge: ManeuverEdge::Aborted,
                partner: None,
                source_facility: FacilityId::from_index(0),
                target_facility: None,
                target_offset_m: 0.0,
                side: PassSide::Left,
                reason: ManeuverReasonCode::TargetLost,
            }]
        );
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
            facility: hekate_model::FacilityId::from_index(0),
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

    /// The maneuver edges one agent's events recorded, in buffer order.
    fn maneuver_edges_for(stream: &[Event], agent: AgentId) -> Vec<ManeuverEdge> {
        stream
            .iter()
            .filter_map(|event| match event {
                Event::Maneuver {
                    agent: record,
                    edge,
                    ..
                } if *record == agent => Some(*edge),
                _ => None,
            })
            .collect()
    }

    /// The step buffer keeps the documented within-tick order with the new
    /// records in it — ascending agent, then kind order, then the variant's own
    /// stable key — and the whole stream is a property of the decisions rather
    /// than of the order the maneuver requests were recorded in.
    #[test]
    fn the_event_buffer_is_key_ordered_and_invariant_to_request_order() {
        let run = |reverse: bool| {
            let mut sim = rider_sim(0.25, 2.0);
            let passed = push_rider(&mut sim, 80.0, 0.0);
            let near = push_rider(&mut sim, 60.0, 0.0);
            let far = push_rider(&mut sim, 54.0, 0.0);
            let requests = if reverse { [far, near] } else { [near, far] };
            for rider in requests {
                assert!(request_maneuver(&mut sim, rider, passed));
            }
            let mut stream = Vec::new();
            let mut recorded = 0;
            for _ in 0..6 {
                let output = sim.step();
                let events = output.events();
                // The documented within-tick order: ascending agent, then kind
                // order, then the variant's own stable key, with the new
                // records in the buffer.
                for window in events.windows(2) {
                    assert!(
                        window[0].order_key() <= window[1].order_key(),
                        "the buffer is non-decreasing by the documented order key: {:?} then {:?}",
                        window[0].order_key(),
                        window[1].order_key()
                    );
                }
                stream.extend(events.iter().cloned());
                recorded += output.transitions().len();
            }
            (stream, recorded, near, far)
        };

        let (forward, recorded, near, far) = run(false);
        let (reversed, ..) = run(true);
        assert_eq!(
            forward, reversed,
            "the emitted stream is invariant to request insertion order"
        );

        // One `Maneuver` event per recorded transition: the event is the state
        // change, so no step can emit a second one for the same edge.
        let maneuvers: Vec<Event> = forward
            .iter()
            .filter(|event| matches!(event, Event::Maneuver { .. }))
            .cloned()
            .collect();
        assert_eq!(
            maneuvers.len(),
            recorded,
            "exactly one Maneuver event per recorded transition"
        );
        let winner = maneuver_edges_for(&forward, near);
        assert!(
            winner.contains(&ManeuverEdge::Attempted) && winner.contains(&ManeuverEdge::Committed),
            "the granted claimant attempts and commits: {winner:?}"
        );
        let loser = maneuver_edges_for(&forward, far);
        assert!(
            loser.contains(&ManeuverEdge::Aborted),
            "the losing claimant aborts: {loser:?}"
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

    /// The maneuver fixture's own commit policy minimum and the rider mode's
    /// target clearance: the two thresholds the ordered unsafe-commit response
    /// reads.
    const POLICY_MIN_CLEARANCE_M: f64 = 0.25;
    const TARGET_CLEARANCE_M: f64 = 0.75;

    /// The rider mode's sampled speed, in metres per second.
    const RIDER_SPEED_MPS: f64 = 6.0;

    /// The world displacement one hazard step may carry: the rider mode's
    /// 6.0 m/s plus the lateral motion its bounded steering can add in one fixed
    /// step, the allowance the Increment 2 passing suite bounds a step by. A
    /// response that teleported or snapped a body breaks it by metres.
    const HAZARD_STEP_LIMIT_MPS: f64 = 7.5;

    /// The rear intrusion's body: it holds station behind the rider, travelling
    /// at the rider's own speed, so its progress footprint stays entirely behind
    /// the rider's own — the rear fact — and its gap and radius put the
    /// committed leg's closest approach below the target clearance and above the
    /// policy minimum: the brake-and-hold window.
    const REAR_INTRUSION_GAP_M: f64 = 1.9;
    const REAR_INTRUSION_RADIUS_M: f64 = 0.85;

    /// The corridor-narrowing body: it holds station alongside the rider,
    /// travelling at the rider's own speed, so it occupies the lateral space the
    /// committed leg must sweep. The first radius leaves the closest approach in
    /// the brake-and-hold window and the second puts it below the policy
    /// minimum.
    const NARROWED_CORRIDOR_HOLD_RADIUS_M: f64 = 0.6;
    const NARROWED_CORRIDOR_LOST_RADIUS_M: f64 = 1.0;

    /// The ordered unsafe-commit response the kernel implements, as the test's
    /// own executable copy of the contract's clause list: abort below the
    /// policy's minimum predicted clearance, otherwise brake within the
    /// profile's comfortable braking and hold at or below the target clearance,
    /// otherwise continue.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum HazardResponse {
        Continue,
        Brake,
        Abort(ManeuverAbortReason),
    }

    /// The documented ordered response for one committed step's swept
    /// prediction, against the fixture's own policy and mode.
    fn documented_response(swept_m: f64, min_m: f64, target_m: f64) -> HazardResponse {
        if swept_m < min_m {
            HazardResponse::Abort(ManeuverAbortReason::ClearanceLost)
        } else if swept_m <= target_m {
            HazardResponse::Brake
        } else {
            HazardResponse::Continue
        }
    }

    /// The response the abort clause's removal leaves: a clearance below the
    /// policy minimum is bracketed by the brake-and-hold clause, so it only
    /// brakes instead of ending the maneuver.
    fn response_without_the_abort_clause(swept_m: f64, target_m: f64) -> HazardResponse {
        if swept_m <= target_m {
            HazardResponse::Brake
        } else {
            HazardResponse::Continue
        }
    }

    /// One hazard step: the edge the rider recorded, its abort reason when it
    /// aborted, and the state and speeds it reached.
    struct HazardStep {
        edge: Option<ManeuverEdge>,
        reason: Option<ManeuverAbortReason>,
        state: RouteState,
        speed_before_mps: f64,
        speed_after_mps: f64,
    }

    /// The prediction production reads for one committed rider this step: the
    /// same call `committed_plan` makes, from the tick-start bodies and the
    /// fixture's own commit policy.
    fn committed_prediction(sim: &Simulation, rider: AgentId) -> ManeuverPrediction {
        let bodies = sim.predicted_bodies();
        let commit = sim
            .scenario
            .commit_policy()
            .expect("the fixture authors a commit policy");
        let batch = ManeuverBatch {
            bodies: &bodies,
            commit: &commit,
            now: sim.time(),
            dt: sim.config().step().as_secs(),
        };
        sim.predict_maneuver(rider.index(), MANEUVER_TARGET_M, &batch)
            .expect("a committed rider predicts its own corridor")
    }

    /// The live bodies that overlap each other, or `None` when every pair is
    /// clear: a hazard response may never overlap a body silently.
    fn overlapping_bodies(sim: &Simulation) -> Option<(AgentId, AgentId)> {
        for first in 0..sim.agents.len() {
            if !sim.agents.alive[first] {
                continue;
            }
            for second in (first + 1)..sim.agents.len() {
                if !sim.agents.alive[second] {
                    continue;
                }
                if query::bodies_intersect(
                    &query::agent_body(&sim.agents, first),
                    &query::agent_body(&sim.agents, second),
                ) {
                    return Some((AgentId::from_index(first), AgentId::from_index(second)));
                }
            }
        }
        None
    }

    /// Step one hazard once under the contract's own limits on every response:
    /// the rider never teleports, no body overlaps another, no boundary is
    /// crossed at all — so none can be the silent forbidden crossing the clause
    /// forbids — and no kernel position cap is needed.
    fn hazard_step(sim: &mut Simulation, rider: AgentId) -> HazardStep {
        let position = sim.agents.position[rider.index()];
        let speed_before_mps = sim.agents.speed_mps[rider.index()];
        let dt = sim.config().step().as_secs();
        // The step's own records are read out before the kernel is observed
        // again: `step` borrows the simulation mutably for its output.
        let (edge, reason, handoffs) = {
            let output = sim.step();
            (
                edge_for(output.transitions(), rider),
                reason_for(output.transitions(), rider),
                output.facility_transitions().len(),
            )
        };
        let stepped_m = (sim.agents.position[rider.index()] - position).length();
        let step = HazardStep {
            edge,
            reason,
            state: rider_state(sim, rider),
            speed_before_mps,
            speed_after_mps: sim.agents.speed_mps[rider.index()],
        };
        assert!(
            stepped_m <= HAZARD_STEP_LIMIT_MPS * dt + 1e-6,
            "a hazard response never teleports the rider: {stepped_m} m in one {dt} s step"
        );
        assert_eq!(
            handoffs, 0,
            "a hazard response inside one band never crosses a boundary"
        );
        assert_eq!(
            overlapping_bodies(sim),
            None,
            "a hazard response never overlaps a body silently"
        );
        assert_eq!(
            sim.emergency_cap_steps(),
            0,
            "a hazard response stays inside the motion limits, not a kernel position cap"
        );
        step
    }

    /// Step one committed hazard once and assert that the recorded response is
    /// the documented ordered response read from the step's own predicted swept
    /// clearance, so the clause list the probe mutates is this kernel's
    /// behavior rather than a second opinion. Returns the step and the swept
    /// clearance it responded to.
    fn committed_hazard_step(sim: &mut Simulation, rider: AgentId) -> (HazardStep, f64) {
        let swept_m = committed_prediction(sim, rider).clears.swept.clearance_m;
        let step = hazard_step(sim, rider);
        let observed = match (step.edge, step.reason, step.state.braking) {
            (Some(ManeuverEdge::Aborted), Some(reason), _) => HazardResponse::Abort(reason),
            (None, None, true) => HazardResponse::Brake,
            (None, None, false) => HazardResponse::Continue,
            (edge, reason, braking) => panic!(
                "a committed hazard step records a brake, a continue, or an abort: \
                 {edge:?} {reason:?} braking={braking}"
            ),
        };
        assert_eq!(
            observed,
            documented_response(swept_m, POLICY_MIN_CLEARANCE_M, TARGET_CLEARANCE_M),
            "the recorded response is the documented response for a {swept_m} m swept clearance"
        );
        (step, swept_m)
    }

    /// The committed corridor's front intrusion: a body in the lateral space the
    /// committed leg sweeps, ahead of the rider, whose closest approach falls
    /// below the target clearance and above the policy minimum. The front fact is
    /// what limits the prediction, and the documented response is the bounded
    /// brake-and-hold: the step records no edge, never accelerates, and stays
    /// inside the profile's comfortable braking.
    #[test]
    fn a_committed_front_intrusion_brakes_within_the_comfort_bound_and_holds() {
        let mut sim = rider_sim(POLICY_MIN_CLEARANCE_M, 2.0);
        let (rider, _passed) = committed_rider(&mut sim);
        let distance_m = sim.agents.distance_m[rider.index()] + 6.0;
        let hazard = push_obstacle(&mut sim, HAZARD_LANE, distance_m, HOLD_OBSTACLE_RADIUS_M);

        let prediction = committed_prediction(&sim, rider);
        assert_eq!(
            prediction
                .clears
                .front
                .expect("the intrusion is ahead of the rider")
                .object,
            LimitingObject::Agent(hazard),
            "the intrusion is the committed leg's front fact"
        );
        assert!(
            prediction.clears.rear.is_none(),
            "no body is behind the rider in this case"
        );
        assert_eq!(
            prediction.clears.swept.object,
            LimitingObject::Agent(hazard),
            "the front intrusion is the clearance minimum the response follows"
        );

        let (step, swept_m) = committed_hazard_step(&mut sim, rider);
        assert_eq!(
            documented_response(swept_m, POLICY_MIN_CLEARANCE_M, TARGET_CLEARANCE_M),
            HazardResponse::Brake
        );
        assert_eq!(
            step.edge, None,
            "a brake holds the maneuver instead of ending it"
        );
        assert_eq!(step.state.maneuver, ManeuverState::Committed);
        assert!(step.state.braking, "the committed maneuver brakes");
        assert!(step.state.hold_since.is_some(), "and starts its hold");
        let profile = sim.agents.profile[rider.index()].expect("a rider profile");
        let dt = sim.config().step().as_secs();
        assert!(
            step.speed_after_mps <= step.speed_before_mps + 1e-12,
            "a braking maneuver never accelerates"
        );
        assert!(
            step.speed_after_mps
                >= step.speed_before_mps - profile.comfortable_brake_mps2 * dt - 1e-9,
            "the brake stays within the comfortable braking: {} < {}",
            step.speed_after_mps,
            step.speed_before_mps - profile.comfortable_brake_mps2 * dt
        );
    }

    /// The committed corridor's rear intrusion: a body holding station behind
    /// the rider in the lateral space the committed leg sweeps, whose closest
    /// approach over that leg's own corridor falls below the target clearance and
    /// above the policy minimum. The rear fact is what limits the prediction, and
    /// the documented response is the same bounded brake-and-hold.
    #[test]
    fn a_committed_rear_intrusion_brakes_within_the_comfort_bound_and_holds() {
        let mut sim = rider_sim(POLICY_MIN_CLEARANCE_M, 2.0);
        let (rider, _passed) = committed_rider(&mut sim);
        let distance_m = sim.agents.distance_m[rider.index()] - REAR_INTRUSION_GAP_M;
        let hazard = push_travelling_obstacle(
            &mut sim,
            HAZARD_LANE,
            distance_m,
            REAR_INTRUSION_RADIUS_M,
            RIDER_SPEED_MPS,
        );

        let prediction = committed_prediction(&sim, rider);
        assert_eq!(
            prediction
                .clears
                .rear
                .expect("the intrusion is behind the rider")
                .object,
            LimitingObject::Agent(hazard),
            "the intrusion is the committed leg's rear fact"
        );
        assert_eq!(
            prediction.clears.swept.object,
            LimitingObject::Agent(hazard),
            "the rear intrusion is the clearance minimum the response follows"
        );

        let (step, swept_m) = committed_hazard_step(&mut sim, rider);
        assert_eq!(
            documented_response(swept_m, POLICY_MIN_CLEARANCE_M, TARGET_CLEARANCE_M),
            HazardResponse::Brake
        );
        assert_eq!(
            step.edge, None,
            "a brake holds the maneuver instead of ending it"
        );
        assert_eq!(step.state.maneuver, ManeuverState::Committed);
        assert!(step.state.braking, "the committed maneuver brakes");
        assert!(step.state.hold_since.is_some(), "and starts its hold");
        let profile = sim.agents.profile[rider.index()].expect("a rider profile");
        let dt = sim.config().step().as_secs();
        assert!(
            step.speed_after_mps <= step.speed_before_mps + 1e-12,
            "a braking maneuver never accelerates"
        );
        assert!(
            step.speed_after_mps
                >= step.speed_before_mps - profile.comfortable_brake_mps2 * dt - 1e-9,
            "the brake stays within the comfortable braking: {} < {}",
            step.speed_after_mps,
            step.speed_before_mps - profile.comfortable_brake_mps2 * dt
        );
    }

    /// The committed corridor's narrowing: a body holding station alongside the
    /// rider in the lateral space the committed leg must occupy, so the usable
    /// corridor the leg sweeps narrows to the space between the two bodies. The
    /// side fact is what limits the prediction, and the ordered response is the
    /// documented escalation — a closest approach inside the brake-and-hold
    /// window brakes and holds, and one below the policy minimum aborts with
    /// `clearance_lost` rather than steering into the body.
    #[test]
    fn a_committed_corridor_narrowed_by_a_side_body_brakes_and_holds_then_aborts() {
        for (radius_m, aborts) in [
            (NARROWED_CORRIDOR_HOLD_RADIUS_M, false),
            (NARROWED_CORRIDOR_LOST_RADIUS_M, true),
        ] {
            let mut sim = rider_sim(POLICY_MIN_CLEARANCE_M, 2.0);
            let (rider, _passed) = committed_rider(&mut sim);
            let distance_m = sim.agents.distance_m[rider.index()];
            let hazard = push_travelling_obstacle(
                &mut sim,
                HAZARD_LANE,
                distance_m,
                radius_m,
                RIDER_SPEED_MPS,
            );

            let prediction = committed_prediction(&sim, rider);
            assert_eq!(
                prediction
                    .clears
                    .side
                    .expect("the narrowing body is alongside the rider")
                    .object,
                LimitingObject::Agent(hazard),
                "the narrowing body is the committed leg's side fact (radius {radius_m})"
            );
            assert_eq!(
                prediction.clears.swept.object,
                LimitingObject::Agent(hazard),
                "the narrowed corridor is the clearance minimum the response follows"
            );

            let (step, swept_m) = committed_hazard_step(&mut sim, rider);
            if aborts {
                assert_eq!(
                    documented_response(swept_m, POLICY_MIN_CLEARANCE_M, TARGET_CLEARANCE_M),
                    HazardResponse::Abort(ManeuverAbortReason::ClearanceLost)
                );
                assert_eq!(step.edge, Some(ManeuverEdge::Aborted));
                assert_eq!(step.reason, Some(ManeuverAbortReason::ClearanceLost));
                assert_eq!(step.state.maneuver, ManeuverState::Aborted);
                assert!(
                    !step.state.braking,
                    "an aborted maneuver holds no brake of its own"
                );
            } else {
                assert_eq!(
                    documented_response(swept_m, POLICY_MIN_CLEARANCE_M, TARGET_CLEARANCE_M),
                    HazardResponse::Brake
                );
                assert_eq!(step.edge, None, "the hold never ends the maneuver");
                assert_eq!(step.state.maneuver, ManeuverState::Committed);
                assert!(step.state.braking, "the narrowed corridor brakes");
                assert!(step.state.hold_since.is_some(), "and starts its hold");
            }
        }
    }

    /// The committed crossing whose connector disappears: a change of lane whose
    /// committed leg reads a destination no compiled adjacency carries. The
    /// crossing the leg needed is gone, so the documented response is the
    /// `aborted` edge with `corridor_infeasible` — the leg never crosses, never
    /// snaps onto a destination reference, and steers back to the offset it held.
    ///
    /// A compiled crossing is immutable within a run, so the one state under
    /// which it disappears is a live leg naming a destination with no adjacency;
    /// the test fixes that state directly, as the cross-facility abort's own
    /// precondition, the way the payload test fixes a `preparing` state.
    #[test]
    fn a_committed_crossing_whose_connector_disappears_aborts_without_crossing() {
        let mut sim = rider_sim(POLICY_MIN_CLEARANCE_M, 2.0);
        let (rider, _passed) = committed_rider(&mut sim);
        let mut state = rider_state(&sim, rider);
        state.target_facility = Some(FacilityId::from_index(1));
        sim.agents.route_state[rider.index()] = Some(state);

        let step = hazard_step(&mut sim, rider);
        assert_eq!(step.edge, Some(ManeuverEdge::Aborted));
        assert_eq!(step.reason, Some(ManeuverAbortReason::CorridorInfeasible));
        assert_eq!(step.state.maneuver, ManeuverState::Aborted);
        assert!(
            !step.state.entering_facility,
            "a leg with no crossing never enters a destination"
        );

        // The aborted leg steers back to its own offset and settles, so the
        // response never strands the rider in a maneuver.
        let mut followed = false;
        for _ in 0..400 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Following {
                followed = true;
                break;
            }
        }
        assert!(followed, "an aborted crossing returns to following");
    }

    /// The blocked return: a returning leg whose return corridor a body holds
    /// station in keeps the offset it occupies, records no transition, and never
    /// reaches for a kernel position cap; it settles only once the same
    /// prediction clears the corridor. The step invariants hold on this leg too,
    /// so a held return never teleports, overlaps a body, or crosses a boundary.
    #[test]
    fn a_returning_leg_blocked_by_a_body_holds_within_the_motion_limits() {
        let mut sim = rider_sim(POLICY_MIN_CLEARANCE_M, 2.0);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        let passed = push_obstacle(&mut sim, PASSED_LANE, 64.0, 0.5);
        assert!(request_maneuver(&mut sim, rider, passed));

        let mut returning = false;
        for _ in 0..80 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Returning {
                returning = true;
                break;
            }
        }
        assert!(
            returning,
            "the committed rider clears the passed body and returns"
        );

        // A body holds station in the return corridor, travelling at the rider's
        // own speed, so the clearance the predictor reads does not open as the
        // rider travels.
        let distance_m = sim.agents.distance_m[rider.index()];
        let obstruction =
            push_travelling_obstacle(&mut sim, RETURN_LANE, distance_m, 0.2, RIDER_SPEED_MPS);
        for _ in 0..40 {
            let step = hazard_step(&mut sim, rider);
            assert_eq!(step.edge, None, "a held return records no transition");
            assert_eq!(step.state.maneuver, ManeuverState::Returning);
            assert!(step.state.return_blocked, "the predictor holds the return");
        }

        // The obstruction leaves the world: the same prediction clears and the
        // rider returns to the offset it held before the attempt.
        sim.agents.alive[obstruction.index()] = false;
        let mut followed = false;
        for _ in 0..400 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Following {
                followed = true;
                break;
            }
        }
        assert!(followed, "a cleared return settles at the rider's offset");
        let state = rider_state(&sim, rider);
        assert!((state.d_m - state.pre_maneuver_offset_m).abs() <= SETTLE_TOLERANCE_M);
    }

    /// The abort clause of the ordered unsafe-commit response, exactly as the
    /// checked-in kernel spells it: the response the probe removes.
    const ABORT_RESPONSE_CLAUSE: &str = r#"if swept_m < batch.commit.min_predicted_clearance_m {
            return self.abort_plan(index, state, ManeuverAbortReason::ClearanceLost, batch.now);
        }"#;

    /// The brake-and-hold clause of the same response, exactly as the kernel
    /// spells it: the clause a source with the abort response removed falls
    /// through to.
    const HOLD_RESPONSE_CLAUSE: &str =
        "if !prediction.is_feasible() || swept_m <= target_clearance_m {";

    /// The checked-in kernel source the ordered response lives in.
    const KERNEL_SOURCE: &str = include_str!("sim.rs");

    /// The falsification probe: remove the ordered response's abort clause from
    /// the checked-in kernel and the hazard suite's required state is no longer
    /// reached, so that suite fails for the intended reason.
    ///
    /// The probe mutates the source text the kernel is compiled from rather than
    /// a copy of its logic. The clause it removes is the checked-in text, so a
    /// reformat or a rename fails the probe loudly instead of passing silently,
    /// and the mutant is checked to keep the brake-and-hold clause, which makes
    /// it exactly the documented response minus its abort. The probe then reads
    /// one committed hazard's fact from the kernel's own predictor — the same
    /// call `committed_plan` makes — and shows that the documented response
    /// aborts on it while the mutant only brakes: the rider would stay
    /// `committed` at the step
    /// `a_committed_corridor_narrowed_by_a_side_body_brakes_and_holds_then_aborts`
    /// requires `aborted`, so that test fails on the state it asserts, for the
    /// missing abort response.
    #[test]
    fn removing_the_abort_response_from_the_kernel_is_falsified_by_the_hazard_suite() {
        assert!(
            KERNEL_SOURCE.contains(ABORT_RESPONSE_CLAUSE),
            "the checked-in kernel must spell the abort response the probe removes"
        );
        assert!(
            KERNEL_SOURCE.contains(HOLD_RESPONSE_CLAUSE),
            "the checked-in kernel must spell the brake-and-hold response"
        );
        let mutant = KERNEL_SOURCE.replace(ABORT_RESPONSE_CLAUSE, "");
        assert_ne!(mutant, KERNEL_SOURCE, "the mutation removes a response");
        assert!(
            !mutant.contains(ABORT_RESPONSE_CLAUSE) && mutant.contains(HOLD_RESPONSE_CLAUSE),
            "the mutant is the ordered response with its abort clause removed"
        );

        // One committed hazard of the suite: a body alongside the rider in the
        // lateral space the leg must occupy, big enough that the leg's closest
        // approach falls below the policy minimum.
        let mut sim = rider_sim(POLICY_MIN_CLEARANCE_M, 2.0);
        let (rider, _passed) = committed_rider(&mut sim);
        let distance_m = sim.agents.distance_m[rider.index()];
        push_travelling_obstacle(
            &mut sim,
            HAZARD_LANE,
            distance_m,
            NARROWED_CORRIDOR_LOST_RADIUS_M,
            RIDER_SPEED_MPS,
        );
        let swept_m = committed_prediction(&sim, rider).clears.swept.clearance_m;
        assert!(
            swept_m < POLICY_MIN_CLEARANCE_M,
            "the probe's hazard must fall below the policy minimum: {swept_m}"
        );

        // The documented rule and the mutant read that one fact differently...
        assert_eq!(
            documented_response(swept_m, POLICY_MIN_CLEARANCE_M, TARGET_CLEARANCE_M),
            HazardResponse::Abort(ManeuverAbortReason::ClearanceLost)
        );
        assert_eq!(
            response_without_the_abort_clause(swept_m, TARGET_CLEARANCE_M),
            HazardResponse::Brake,
            "without the abort response the same fact only brakes"
        );

        // ...and the checked-in kernel takes the documented response, so the
        // mutant's rider would remain `committed` and braking at the step the
        // suite requires the abort with `clearance_lost`.
        let (step, _swept) = committed_hazard_step(&mut sim, rider);
        assert_eq!(step.reason, Some(ManeuverAbortReason::ClearanceLost));
        assert_eq!(step.state.maneuver, ManeuverState::Aborted);
    }

    /// A returning rider whose return corridor a body occupies holds at the
    /// offset it occupies — the ordinary predictor's own verdict on the return
    /// target, not a second geometry path — and re-decides from each step's
    /// bodies. It never settles while the corridor stays blocked, and it returns
    /// to its own offset once the same prediction clears again.
    #[test]
    fn a_blocked_return_leg_holds_and_re_decides_until_the_corridor_clears() {
        let mut sim = rider_sim(0.25, 2.0);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        // The passed obstacle is a stationary body ahead on the parallel lane to
        // the rider's right, so the maneuver stays committed until the rider has
        // displaced and cleared it.
        let passed = push_obstacle(&mut sim, PASSED_LANE, 64.0, 0.5);
        assert!(request_maneuver(&mut sim, rider, passed));

        let mut returning = false;
        for _ in 0..80 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Returning {
                returning = true;
                break;
            }
        }
        assert!(
            returning,
            "the committed rider clears the passed body and returns"
        );
        let displaced_m = rider_state(&sim, rider).d_m;
        assert!(
            displaced_m > 0.05,
            "the committed rider has displaced before the return: {displaced_m}"
        );

        // A body holds station in the return corridor: it travels at the rider's
        // own speed, so the clearance the predictor reads over the horizon does
        // not open as the rider travels, and the return stays blocked.
        let distance_m = sim.agents.distance_m[rider.index()];
        let obstruction = push_travelling_obstacle(&mut sim, RETURN_LANE, distance_m, 0.2, 6.0);

        let mut edges = Vec::new();
        for step in 0..80 {
            let output = sim.step();
            edges.extend(step_edges(output.transitions()));
            let state = rider_state(&sim, rider);
            assert_eq!(
                state.maneuver,
                ManeuverState::Returning,
                "a blocked return never settles"
            );
            assert!(state.return_blocked, "the predictor holds the return");
            assert!(
                (state.d_m - displaced_m).abs() < 0.02,
                "a blocked return holds the offset it occupies, at {} not {displaced_m}",
                state.d_m
            );
            if step == 0 {
                assert!(
                    state
                        .predicted_min_clearance_m
                        .expect("the committed prediction is kept")
                        > 0.75,
                    "the committed leg's clearance was fine, so the hold is the return leg's"
                );
            }
        }
        assert!(
            edges.is_empty(),
            "a held return records no transition: {edges:?}"
        );
        assert_eq!(sim.emergency_cap_steps(), 0);

        // The obstruction leaves the world: the same prediction clears and the
        // rider returns to the offset it held before the attempt.
        sim.agents.alive[obstruction.index()] = false;
        let mut followed = false;
        for _ in 0..400 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Following {
                followed = true;
                break;
            }
        }
        assert!(followed, "a cleared return completes at the rider's offset");
        let state = rider_state(&sim, rider);
        assert!((state.d_m - state.pre_maneuver_offset_m).abs() <= SETTLE_TOLERANCE_M);
        assert!(!state.return_blocked);
        assert_eq!(state.maneuver, ManeuverState::Following);
    }

    /// An aborted rider that had displaced holds the offset it occupies while a
    /// body obstructs its return to the pre-maneuver offset, and returns once
    /// the corridor clears: the same policy on the `aborted` leg.
    #[test]
    fn a_blocked_aborted_leg_holds_until_the_corridor_clears() {
        let mut sim = rider_sim(0.25, 2.0);
        let rider = push_rider(&mut sim, 60.0, 0.0);
        // The passed obstacle is far ahead on the parallel lane to the rider's
        // right, so the completion guard never holds and the maneuver stays
        // committed while the rider displaces.
        let passed = push_obstacle(&mut sim, PASSED_LANE, 100.0, 0.5);
        assert!(request_maneuver(&mut sim, rider, passed));
        sim.step();
        let output = sim.step();
        assert_eq!(step_edges(output.transitions()), [ManeuverEdge::Committed]);
        for _ in 0..20 {
            sim.step();
        }
        let displaced_m = rider_state(&sim, rider).d_m;
        assert!(
            displaced_m > 0.05,
            "the committed rider displaces before the abort: {displaced_m}"
        );

        // A committed clearance loss aborts the maneuver.
        let hazard_m = sim.agents.distance_m[rider.index()] + 6.0;
        push_obstacle(&mut sim, HAZARD_LANE, hazard_m, LOST_OBSTACLE_RADIUS_M);
        let output = sim.step();
        assert_eq!(
            edge_for(output.transitions(), rider),
            Some(ManeuverEdge::Aborted)
        );
        assert_eq!(
            reason_for(output.transitions(), rider),
            Some(ManeuverAbortReason::ClearanceLost)
        );
        assert_eq!(rider_state(&sim, rider).maneuver, ManeuverState::Aborted);

        // A body holds station in the corridor the rider would return through.
        let distance_m = sim.agents.distance_m[rider.index()];
        let obstruction = push_travelling_obstacle(&mut sim, RETURN_LANE, distance_m, 0.2, 6.0);
        for _ in 0..40 {
            let output = sim.step();
            assert!(
                output.transitions().is_empty(),
                "a blocked abort records no transition"
            );
            let state = rider_state(&sim, rider);
            assert_eq!(state.maneuver, ManeuverState::Aborted);
            assert!(state.return_blocked, "the aborted leg holds its offset");
            assert!((state.d_m - displaced_m).abs() < 0.02);
        }
        assert_eq!(sim.emergency_cap_steps(), 0);

        sim.agents.alive[obstruction.index()] = false;
        let mut followed = false;
        for _ in 0..400 {
            sim.step();
            if rider_state(&sim, rider).maneuver == ManeuverState::Following {
                followed = true;
                break;
            }
        }
        assert!(followed, "a cleared abort returns to following");
        assert!(!rider_state(&sim, rider).return_blocked);
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

    /// The handoff helpers name each compiled direction and side once: a travel
    /// sign maps to one `MovementDirection`, a crossing side follows the turn the
    /// destination heading makes, and a straight continuation resolves left.
    #[test]
    fn handoff_helpers_name_directions_and_sides_deterministically() {
        assert_eq!(movement_direction(1.0), MovementDirection::Forward);
        assert_eq!(movement_direction(-1.0), MovementDirection::Reverse);
        assert_eq!(travel_sign(MovementDirection::Forward), 1.0);
        assert_eq!(travel_sign(MovementDirection::Reverse), -1.0);
        assert_eq!(travel_pass_side(AdjacencySide::Left), PassSide::Left);
        assert_eq!(travel_pass_side(AdjacencySide::Right), PassSide::Right);
        assert_eq!(connector_crossing_side(0.0, 0.2), PassSide::Left);
        assert_eq!(connector_crossing_side(0.0, -0.2), PassSide::Right);
        assert_eq!(
            connector_crossing_side(0.0, 0.0),
            PassSide::Left,
            "a straight continuation resolves left"
        );
    }
}
