//! The deterministic, fixed-step simulation kernel.
//!
//! `Simulation` owns the authoritative clock. It advances exactly one
//! configured tick per [`Simulation::step`] call and never reads wall-clock
//! time, so frame rate, pause, and speed controls cannot change results.
//! Increment 0 moves constant-speed agents along a guide path and emits typed
//! spawn/despawn events; it deliberately omits interaction.

use tangle_model::CompiledScenario;

use crate::agent::{AgentId, AgentInit, AgentStore};
use crate::config::RunConfig;
use crate::event::{DespawnReason, Event};
use crate::snapshot::{AgentSample, MotionSample, Snapshot, SnapshotDetail};
use crate::time::SimTime;
use crate::units::Seconds;

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
    events: Vec<Event>,
    population_announced: bool,
    spawned_total: u64,
    despawned_total: u64,
}

impl Simulation {
    /// Build a simulation from a compiled scenario and run configuration.
    ///
    /// The initial population is placed immediately along the first guide path
    /// at the configured spacing, so a snapshot before the first step already
    /// shows the starting state. The first [`Self::step`] announces those
    /// agents with [`Event::Spawned`] before advancing the clock.
    pub fn new(scenario: CompiledScenario, config: RunConfig) -> Result<Self, InitError> {
        let step = config.step();
        if !step.is_finite_positive() {
            return Err(InitError::InvalidStep {
                step: step.as_secs(),
            });
        }

        let population = *scenario.population();
        let mut agents = AgentStore::default();
        if population.vehicle_count > 0 {
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
                    path: path.id(),
                    distance_m,
                    speed_mps: population.vehicle_speed_mps,
                    position: path.position_at(distance_m),
                    heading_rad: path.heading_at(distance_m),
                    body_length_m: population.vehicle_length_m,
                    body_width_m: population.vehicle_width_m,
                });
            }
        }

        Ok(Self {
            scenario,
            config,
            tick: 0,
            agents,
            events: Vec::new(),
            population_announced: false,
            spawned_total: u64::from(population.vehicle_count),
            despawned_total: 0,
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
                        speed_mps: self.agents.speed_mps[index],
                        path: self.agents.path[index],
                        path_distance_m: self.agents.distance_m[index],
                        body_length_m: self.agents.body_length_m[index],
                        body_width_m: self.agents.body_width_m[index],
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

        for index in 0..self.agents.len() {
            if !self.agents.alive[index] {
                continue;
            }
            let path_id = self.agents.path[index];
            let Some(path) = self.scenario.path(path_id) else {
                continue;
            };

            let travelled = self.agents.distance_m[index] + self.agents.speed_mps[index] * dt;
            let length = path.length();
            let distance_m = travelled.min(length);

            self.agents.distance_m[index] = distance_m;
            self.agents.position[index] = path.position_at(distance_m);
            self.agents.heading_rad[index] = path.heading_at(distance_m);

            if travelled >= length {
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
