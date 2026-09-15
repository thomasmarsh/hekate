//! Explicit, replaceable controller interfaces, and the index of the two
//! initial model cards.
//!
//! # The seam
//!
//! The kernel never calls a concrete motion model. [`crate::Simulation`] drives
//! each mode through [`ControllerModels`], a set of boxed trait objects:
//!
//! - [`VehicleController`] answers the vehicle longitudinal question: given a
//!   sampled profile, the vehicle's speed, and the constraints ahead, what
//!   acceleration does the vehicle command for this step? Its initial
//!   implementation is [`IdmController`], the documented Intelligent Driver
//!   Model in [`crate::control`].
//! - [`crate::narrow::NarrowWheeledController`] answers the same longitudinal
//!   question for the narrow wheeled family (a capsule that steers): given a
//!   sampled narrow profile, the agent's speed, and the constraints ahead, what
//!   acceleration does it command? Its initial implementation is
//!   [`crate::narrow::IdmNarrowWheeledController`], the shared IDM law under a
//!   narrow profile, documented by the bicycle and scooter cards in
//!   [`crate::narrow`].
//! - [`PedestrianController`] answers the pedestrian question: given a sampled
//!   profile, the pedestrian's world state, and the nearby bodies, which
//!   heading and speed target does it command, and how far may this step's
//!   speed advance? Its initial implementation is [`WaypointController`], the
//!   documented waypoint controller with local collision avoidance in
//!   [`crate::pedestrian`].
//!
//! The kernel keeps every interaction decision: which constraints exist, leader
//! and neighbour selection, stop-line and crossing-yield state, crossing
//! occupancy, waypoint planning, conflict collection, the safety position and
//! spacing caps, and the emergency counters. A model only maps a context to a
//! command, so replacing one — or adding a model for a deferred mode — never
//! edits the kernel's interaction logic. That is the boundary `VISION.md`
//! requires ("No single behavioral model should be hardwired into the kernel",
//! and "behavior and motion models are replaceable behind narrow interfaces")
//! and `PHASE_1_PLAN.md` Increment 3 delivers.
//!
//! The seam is crate-internal in Phase 1, exactly like the shared spatial index
//! ([`crate::index`]): the models are chosen in one place, [`ControllerModels::initial`],
//! and there is no external controller plugin surface until a mode needs one.
//! [`ControllerModelNames`] is the small public provenance view of the choice,
//! so a run can record which models it drove and a test can assert them.
//!
//! # Model cards
//!
//! The two initial models each carry a full model card in the module that
//! documents their equations and constants:
//!
//! | Model | Family | Card |
//! |---|---|---|
//! | [`IdmController`] | Intelligent Driver Model, Treiber, Hennecke & Helbing (2000) | [`crate::control`] |
//! | [`crate::narrow::IdmNarrowWheeledController`] | Intelligent Driver Model under a narrow wheeled profile | [`crate::narrow`] |
//! | [`WaypointController`] | pure-pursuit waypoint seeking, Coulter (1992), with a bounded social-force-style repulsion, Helbing & Molnár (1995) | [`crate::pedestrian`] |
//!
//! Each card follows the checked-in template
//! (`docs/model-card-template.md`), which is the inventory of record. In that
//! order a card states its state variables; its parameters (sampled) and
//! constants (model properties); its decision inputs; its bounds; its
//! tie-breaks; its emergency backstop outside those bounds; and then its
//! assumptions, parameter sources, validated ranges, known failure modes, and
//! incompatible fidelity settings, with its equations or steering law in
//! between; the pedestrian card additionally states its waypoints and its
//! determinism. The narrow wheeled family carries one card per mode
//! (bicycle and scooter) in the module that implements the shared model.
//!
//! # Scope
//!
//! This seam indexes the models the kernel holds. The narrow wheeled model is
//! held here for the family the spawn path dispatches on; wiring narrow-mode
//! spawning and the family dispatch through the four stages is the tracked
//! follow-up, so no agent reaches the narrow model yet.

use crate::control::{self, Constraint};
use crate::narrow::{self, NarrowWheeledController};
use crate::pedestrian::{self, Conflict, PedestrianState, Steering};
use crate::profile::{PedestrianProfile, VehicleProfile};

/// One replaceable vehicle longitudinal model.
///
/// Implementations must be pure in their inputs and must not draw from any
/// random stream, so swapping a model cannot reorder a named stream. A model
/// commands an acceleration only; the kernel clamps speed to `[0, v0]`,
/// applies the safety position caps, and counts the steps where a cap brakes
/// harder than the command's own bound.
///
/// `Send + Sync`: a whole [`crate::Simulation`] stays shareable, so the viewer
/// can hold one as a resource and batch replication can move one between
/// threads. A model must therefore own its state, not borrow it.
pub(crate) trait VehicleController: std::fmt::Debug + Send + Sync {
    /// Stable model name, for provenance and diagnostics.
    fn name(&self) -> &'static str;

    /// Commanded longitudinal acceleration in m/s² for one step.
    ///
    /// `constraints` is the list of constraints ahead the kernel selected — a
    /// leader, a required stop line, an occupied crossing the vehicle yields
    /// to — or an empty slice for free-flow driving.
    fn desired_acceleration(
        &self,
        profile: &VehicleProfile,
        speed_mps: f64,
        constraints: &[Constraint],
    ) -> f64;
}

/// One replaceable pedestrian model: steering plus the per-step speed bound.
///
/// As with [`VehicleController`], implementations must be pure in their inputs
/// and must not draw from any random stream. The kernel owns waypoint planning,
/// conflict collection, conflict ordering, integration, and the despawn test;
/// a model owns only the commanded heading, the speed target, and how far the
/// speed may advance this step. `Send + Sync` for the same reason as
/// [`VehicleController`].
pub(crate) trait PedestrianController: std::fmt::Debug + Send + Sync {
    /// Stable model name, for provenance and diagnostics.
    fn name(&self) -> &'static str;

    /// Commanded heading, speed target, and hard spacing cap for one step.
    ///
    /// `state` carries the pedestrian's own identifier, world position,
    /// heading, speed, and current waypoint target; `conflicts` are the nearby
    /// bodies the kernel reported, in ascending [`crate::AgentId`] order.
    fn steer(
        &self,
        profile: &PedestrianProfile,
        state: &PedestrianState,
        conflicts: &[Conflict],
        dt: f64,
    ) -> Steering;

    /// The speed after one step of the model's acceleration bounds and spacing
    /// cap, and whether the cap forced braking beyond those bounds.
    ///
    /// The returned flag is the model's own emergency exception; the kernel
    /// counts the steps where it is set in [`crate::Simulation::pedestrian_cap_steps`].
    fn advance_speed(
        &self,
        speed_mps: f64,
        target_speed_mps: f64,
        spacing_cap_mps: Option<f64>,
        dt: f64,
    ) -> (f64, bool);
}

/// The initial vehicle longitudinal model: the documented IDM controller.
///
/// The equations, parameters, bounds, and backstops are the model card in
/// [`crate::control`]; this type is only the seam binding.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IdmController;

impl VehicleController for IdmController {
    fn name(&self) -> &'static str {
        "idm"
    }

    fn desired_acceleration(
        &self,
        profile: &VehicleProfile,
        speed_mps: f64,
        constraints: &[Constraint],
    ) -> f64 {
        control::desired_acceleration(profile, speed_mps, constraints)
    }
}

/// The initial pedestrian model: the documented waypoint controller with local
/// collision avoidance.
///
/// The equations, waypoints, bounds, and spacing cap are the model card in
/// [`crate::pedestrian`]; this type is only the seam binding.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WaypointController;

impl PedestrianController for WaypointController {
    fn name(&self) -> &'static str {
        "waypoint"
    }

    fn steer(
        &self,
        profile: &PedestrianProfile,
        state: &PedestrianState,
        conflicts: &[Conflict],
        dt: f64,
    ) -> Steering {
        pedestrian::steer(profile, state, conflicts, dt)
    }

    fn advance_speed(
        &self,
        speed_mps: f64,
        target_speed_mps: f64,
        spacing_cap_mps: Option<f64>,
        dt: f64,
    ) -> (f64, bool) {
        pedestrian::advance_speed(speed_mps, target_speed_mps, spacing_cap_mps, dt)
    }
}

/// The motion models the kernel drives its interaction logic through.
///
/// [`Simulation`](crate::Simulation) holds one of these, so each family is
/// reachable through an interface rather than through a module function call.
#[derive(Debug)]
pub(crate) struct ControllerModels {
    /// The vehicle longitudinal model.
    pub(crate) vehicle: Box<dyn VehicleController>,
    /// The narrow wheeled longitudinal model.
    pub(crate) narrow: Box<dyn NarrowWheeledController>,
    /// The pedestrian model.
    pub(crate) pedestrian: Box<dyn PedestrianController>,
}

impl ControllerModels {
    /// The initial models: IDM for vehicles, the shared IDM law under a narrow
    /// profile for the narrow wheeled family, and the waypoint controller for
    /// pedestrians.
    pub(crate) fn initial() -> Self {
        Self {
            vehicle: Box::new(IdmController),
            narrow: Box::new(narrow::IdmNarrowWheeledController),
            pedestrian: Box::new(WaypointController),
        }
    }

    /// The names of the models in place, for provenance and diagnostics.
    pub(crate) fn names(&self) -> ControllerModelNames {
        ControllerModelNames {
            vehicle: self.vehicle.name(),
            narrow: self.narrow.name(),
            pedestrian: self.pedestrian.name(),
        }
    }
}

/// Names of the motion models a run drives.
///
/// A run manifest records which models produced its trace, so a later model
/// swap cannot be mistaken for a behavior change. This is the only public
/// surface of the controller seam in Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerModelNames {
    /// The vehicle longitudinal model in use.
    pub vehicle: &'static str,
    /// The narrow wheeled longitudinal model in use.
    pub narrow: &'static str,
    /// The pedestrian model in use.
    pub pedestrian: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::narrow::NarrowProfile;
    use crate::sim::Simulation;
    use crate::{RunConfig, SnapshotDetail};
    use glam::DVec2;
    use hekate_model::{CompiledScenario, parse_scenario_source};

    /// A 200 m road and a separate 40 m walking path, each with demand, so both
    /// modes are present without any interaction between them.
    const TWO_MODES: &str = r#"
    {
      schema_version: 1,
      id: 'controller_seam_contract',
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

    /// One frame of observations: identifier, mode, speed, world position.
    type Frame = Vec<(u32, AgentMode, f64, DVec2)>;

    fn sim(seed: u64) -> Simulation {
        let source = parse_scenario_source(TWO_MODES).expect("scenario parses");
        let scenario = CompiledScenario::compile(source).expect("scenario compiles");
        Simulation::new(scenario, RunConfig::new(seed)).expect("simulation builds")
    }

    /// Run `ticks` steps and return the final observation frame.
    fn run(sim: &mut Simulation, ticks: u64) -> Frame {
        let mut last = Frame::new();
        for _ in 0..ticks {
            sim.step();
            last = sim
                .snapshot(SnapshotDetail::Full)
                .agents()
                .iter()
                .map(|sample| {
                    let motion = sample.motion.as_ref().expect("full detail");
                    (
                        sample.id.get(),
                        motion.mode,
                        motion.speed_mps,
                        sample.position,
                    )
                })
                .collect();
        }
        last
    }

    /// A vehicle model that ignores its constraints and brakes at exactly
    /// 1 m/s², within any sampled comfortable braking, so it never trips the
    /// kernel's emergency counter.
    #[derive(Debug)]
    struct FixedBrakeController;

    impl VehicleController for FixedBrakeController {
        fn name(&self) -> &'static str {
            "fixed-brake test double"
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

    /// A narrow wheeled model that commands a standstill, so its motion is
    /// unmistakably not the shared IDM law's.
    #[derive(Debug)]
    struct StandstillNarrowController;

    impl NarrowWheeledController for StandstillNarrowController {
        fn name(&self) -> &'static str {
            "standstill narrow test double"
        }

        fn desired_acceleration(
            &self,
            _profile: &NarrowProfile,
            _speed_mps: f64,
            _constraints: &[Constraint],
        ) -> f64 {
            0.0
        }
    }

    /// A pedestrian model that commands a standstill and no spacing cap, so its
    /// motion is unmistakably not the waypoint controller's.
    #[derive(Debug)]
    struct StandstillController;

    impl PedestrianController for StandstillController {
        fn name(&self) -> &'static str {
            "standstill test double"
        }

        fn steer(
            &self,
            _profile: &PedestrianProfile,
            state: &PedestrianState,
            _conflicts: &[Conflict],
            _dt: f64,
        ) -> Steering {
            Steering {
                heading_rad: state.heading_rad,
                speed_target_mps: 0.0,
                spacing_cap_mps: None,
            }
        }

        fn advance_speed(
            &self,
            _speed_mps: f64,
            target_speed_mps: f64,
            _spacing_cap_mps: Option<f64>,
            _dt: f64,
        ) -> (f64, bool) {
            (target_speed_mps.max(0.0), false)
        }
    }

    /// The stub pair a test installs.
    fn stubs() -> ControllerModels {
        ControllerModels {
            vehicle: Box::new(FixedBrakeController),
            narrow: Box::new(StandstillNarrowController),
            pedestrian: Box::new(StandstillController),
        }
    }

    const STUB_NAMES: ControllerModelNames = ControllerModelNames {
        vehicle: "fixed-brake test double",
        narrow: "standstill narrow test double",
        pedestrian: "standstill test double",
    };

    #[test]
    fn the_initial_models_are_idm_the_narrow_idm_and_the_waypoint_controller() {
        assert_eq!(
            sim(3).controller_models(),
            ControllerModelNames {
                vehicle: "idm",
                narrow: "idm-narrow",
                pedestrian: "waypoint",
            }
        );
    }

    #[test]
    fn the_kernel_reaches_both_modes_only_through_the_interfaces() {
        // 1500 ticks is 75 s of demand: an arrival every 3 s on the 200 m road
        // keeps several vehicles live, and an arrival every 4 s on the 40 m
        // walking path keeps several pedestrians live, with no queueing in
        // either mode. Each body's motion is then a pure function of its own
        // model.
        let mut stubbed_sim = sim(3);
        stubbed_sim.set_controller_models(stubs());
        let stubbed = run(&mut stubbed_sim, 1500);
        assert_eq!(stubbed_sim.controller_models(), STUB_NAMES);

        let mut initial_sim = sim(3);
        let initial = run(&mut initial_sim, 1500);
        assert_eq!(
            initial_sim.controller_models(),
            ControllerModelNames {
                vehicle: "idm",
                narrow: "idm-narrow",
                pedestrian: "waypoint",
            }
        );

        // Both modes are live in each run and share one observation frame.
        for frame in [&stubbed, &initial] {
            assert!(
                frame
                    .iter()
                    .any(|(_, mode, _, _)| *mode == AgentMode::Vehicle)
            );
            assert!(
                frame
                    .iter()
                    .any(|(_, mode, _, _)| *mode == AgentMode::Pedestrian)
            );
        }

        let vehicles_of = |frame: &Frame| {
            frame
                .iter()
                .filter(|(_, mode, _, _)| *mode == AgentMode::Vehicle)
                .map(|(_, _, speed_mps, _)| *speed_mps)
                .collect::<Vec<f64>>()
        };
        let pedestrians_of = |frame: &Frame| {
            frame
                .iter()
                .filter(|(_, mode, _, _)| *mode == AgentMode::Pedestrian)
                .map(|(_, _, speed_mps, position)| (*speed_mps, *position))
                .collect::<Vec<(f64, DVec2)>>()
        };

        // The initial models: IDM keeps every vehicle near its sampled desired
        // speed of 10 m/s on the empty road, and the waypoint controller walks
        // every pedestrian on at its sampled 1.25 m/s.
        assert!(vehicles_of(&initial).iter().all(|speed| *speed > 1.0));
        assert!(
            vehicles_of(&initial)
                .iter()
                .all(|speed| *speed <= 10.0 + 1e-9)
        );
        assert!(
            pedestrians_of(&initial)
                .iter()
                .all(|(speed, _)| *speed > 1.0)
        );
        assert!(
            pedestrians_of(&initial)
                .iter()
                .any(|(_, position)| position.y > 41.0),
            "the waypoint controller must walk beyond the entry"
        );

        // The stubs: the constant 1 m/s² brake brings the earliest vehicle to
        // rest on the road, and the standstill model holds every pedestrian at
        // its entry point. Neither is reachable through IDM or the waypoint
        // controller on this scenario.
        assert!(vehicles_of(&stubbed).contains(&0.0));
        assert!(
            vehicles_of(&stubbed)
                .iter()
                .all(|speed| *speed <= 10.0 + 1e-9)
        );
        assert!(
            pedestrians_of(&stubbed)
                .iter()
                .all(|(speed, position)| { *speed == 0.0 && *position == DVec2::new(0.0, 40.0) })
        );
    }
}
