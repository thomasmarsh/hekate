//! Headless session: the simulation, the shared controller, and the cell
//! backend, with no terminal I/O beyond the backend's [`Write`] sink.
//!
//! [`TuiSession`] is what tests drive: it owns exactly one [`Simulation`] and
//! advances it only in whole fixed steps through the shared
//! [`PresentationController`]. Keyboard or pointer events translate into
//! shared [`ViewCommand`]s, so the terminal backend cannot change which ticks
//! the kernel visits any more than the Bevy backend can.

use std::io::Write;
use std::sync::Arc;

use glam::DVec2;
use hekate_model::CompiledScenario;
use hekate_present::{
    Applied, BackendResult, PresentationController, RendererBackend, RestartMode, SceneFrame,
    SceneGeometry, Speed, ViewCommand, Viewport,
};
use hekate_sim::{AgentId, Event, InitError, RunConfig, Simulation, Snapshot, SnapshotDetail};

use crate::backend::{CellBackend, RunInfo};
use crate::palette::ColorDepth;
use crate::raster::CELL_ASPECT;

/// Default terminal size before the host reports the real one.
const DEFAULT_COLUMNS: u32 = 80;
/// Default terminal height before the host reports the real one.
const DEFAULT_ROWS: u32 = 24;

/// A renderer backend a [`TuiSession`] can drive.
///
/// The shared [`RendererBackend`] contract covers drawing and presenting; a
/// session additionally needs to size the backend to the host terminal, learn
/// the scene extent to fit the viewport, report run totals, and release any
/// terminal resources on shutdown.
pub trait SessionBackend: RendererBackend {
    /// Update the run totals the status line shows.
    fn set_run_info(&mut self, run: RunInfo);

    /// Resize to the host terminal's `columns` by `rows` cells.
    fn resize_terminal(&mut self, columns: u32, rows: u32) -> BackendResult;

    /// The scene area in viewport units, for [`Viewport::fit`].
    fn scene_extent(&self) -> (f64, f64);

    /// Release terminal resources, such as deleting placed graphics images.
    fn shutdown(&mut self) -> BackendResult {
        Ok(())
    }
}

impl<W: Write> SessionBackend for CellBackend<W> {
    fn set_run_info(&mut self, run: RunInfo) {
        CellBackend::set_run_info(self, run);
    }

    fn resize_terminal(&mut self, columns: u32, rows: u32) -> BackendResult {
        self.resize(columns, rows)
    }

    fn scene_extent(&self) -> (f64, f64) {
        let (columns, rows) = self.scene_size();
        (
            f64::from(columns.max(1)),
            f64::from(rows.max(1)) * CELL_ASPECT,
        )
    }
}

impl<W: Write> SessionBackend for crate::kitty::KittyBackend<W> {
    fn set_run_info(&mut self, run: RunInfo) {
        crate::kitty::KittyBackend::set_run_info(self, run);
    }

    fn resize_terminal(&mut self, columns: u32, rows: u32) -> BackendResult {
        self.resize_terminal(columns, rows)
    }

    fn scene_extent(&self) -> (f64, f64) {
        let (columns, rows) = self.scene_size();
        (
            f64::from(columns.max(1)),
            f64::from(rows.max(1)) * CELL_ASPECT,
        )
    }

    fn shutdown(&mut self) -> BackendResult {
        self.shutdown()
    }
}

/// One terminal viewing session.
pub struct TuiSession<B: SessionBackend> {
    scenario: Arc<CompiledScenario>,
    sim: Simulation,
    controller: PresentationController,
    prev: Snapshot,
    curr: Snapshot,
    seed: u64,
    spawned: u64,
    despawned: u64,
    backend: B,
}

impl<W: Write> TuiSession<CellBackend<W>> {
    /// Start `scenario` at `seed`, rendering through the character-cell backend
    /// on `writer` at `depth`.
    pub fn new(
        scenario: Arc<CompiledScenario>,
        seed: u64,
        writer: W,
        depth: ColorDepth,
    ) -> Result<Self, InitError> {
        let backend = CellBackend::new(writer, depth, Arc::clone(&scenario));
        Self::with_backend(scenario, seed, backend)
    }

    /// Consume the session and return the backend sink for assertions.
    pub fn into_writer(self) -> W {
        self.backend.into_writer()
    }
}

impl<B: SessionBackend> TuiSession<B> {
    /// Start `scenario` at `seed`, rendering through `backend`.
    pub fn with_backend(
        scenario: Arc<CompiledScenario>,
        seed: u64,
        backend: B,
    ) -> Result<Self, InitError> {
        let sim = Simulation::new((*scenario).clone(), RunConfig::new(seed))?;
        let step_secs = sim.config().step().as_secs();
        let initial = sim.snapshot(SnapshotDetail::Full);
        let geometry = SceneGeometry::from_scenario(&scenario);
        let controller =
            PresentationController::new(geometry, step_secs, Viewport::new(DVec2::ZERO, 1.0));

        let mut session = Self {
            scenario,
            sim,
            controller,
            prev: initial.clone(),
            curr: initial,
            seed,
            spawned: 0,
            despawned: 0,
            backend,
        };
        session.resize(DEFAULT_COLUMNS, DEFAULT_ROWS);
        Ok(session)
    }

    /// React to a terminal resize: reserve the status and footer rows, then
    /// refit the viewport to the new scene area with aspect correction.
    pub fn resize(&mut self, columns: u32, rows: u32) {
        let _ = self.backend.resize_terminal(columns, rows);
        let screen = self.backend.scene_extent();
        let bounds = self
            .controller
            .geometry()
            .bounds()
            .unwrap_or((DVec2::splat(-20.0), DVec2::splat(20.0)));
        self.controller
            .set_viewport(Viewport::fit(bounds, screen, 1.25));
    }

    /// Seed the run is on.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Completed kernel ticks.
    pub fn tick(&self) -> u64 {
        self.curr.time().tick()
    }

    /// Simulated seconds at the projected tick.
    pub fn time_seconds(&self) -> f64 {
        self.curr.time().seconds()
    }

    /// Current playback speed.
    pub const fn speed(&self) -> Speed {
        self.controller.speed()
    }

    /// Whether playback is paused.
    pub const fn paused(&self) -> bool {
        self.controller.paused()
    }

    /// Currently selected agent, if any.
    pub const fn selection(&self) -> Option<usize> {
        self.controller.selection()
    }

    /// Current viewport.
    pub const fn viewport(&self) -> Viewport {
        self.controller.viewport()
    }

    /// The live kernel, for parity assertions.
    pub const fn simulation(&self) -> &Simulation {
        &self.sim
    }

    /// The rendering backend.
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// The rendering backend, mutably, for shutdown and resource release.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Apply one shared view command, restarting the kernel if it asks.
    pub fn apply(&mut self, command: ViewCommand) {
        let frame = self.project();
        if let Applied::Restart(mode) = self.controller.apply(&command, &frame) {
            let seed = match mode {
                RestartMode::SameSeed => self.seed,
                RestartMode::NextSeed => self.seed.wrapping_add(1),
            };
            self.restart(seed);
        }
    }

    /// Select the next live agent, wrapping around, or clear when none remain.
    pub fn select_next(&mut self) {
        let frame = self.project();
        let Some(next) = next_selection(&frame) else {
            self.controller.apply(&ViewCommand::ClearSelection, &frame);
            return;
        };
        self.controller
            .apply(&ViewCommand::SelectNearest(next), &frame);
    }

    /// Ask the kernel to record a wrong-way entry request for the selected
    /// agent, so the wrong-way rule state is reachable in a normal run.
    ///
    /// The kernel owns the decision: it records the request only for a live
    /// agent that carries route state on a connected opposing traversal, and
    /// its wrong-way pass evaluates the scenario's policy at the start of the
    /// next step. A checked-in scenario authors no request, so this is the host
    /// seam a viewer supplies. Returns whether a request was recorded.
    pub fn request_wrong_way_entry(&mut self) -> bool {
        let frame = self.project();
        let Some(id) = frame.status.selection else {
            return false;
        };
        if frame.body(id).and_then(|body| body.route_state).is_none() {
            return false;
        }
        self.sim.request_wrong_way_entry(AgentId::from_index(id))
    }

    /// Consume `frame_secs` of wall time, stepping the kernel in whole fixed
    /// steps. Returns the number of ticks taken.
    pub fn advance(&mut self, frame_secs: f64) -> u64 {
        let ticks = self.controller.advance(frame_secs);
        for _ in 0..ticks {
            self.step_once();
        }
        ticks
    }

    /// Project and draw one frame through the backend without presenting it.
    ///
    /// This is the draw half of [`Self::render`], exposed so a test can inspect
    /// the backend's pending output (for example a character grid) before it is
    /// serialized to the sink.
    pub fn draw(&mut self) -> BackendResult {
        self.backend.set_run_info(RunInfo {
            seed: self.seed,
            spawned: self.spawned,
            despawned: self.despawned,
        });
        let frame = self.project();
        self.backend.draw(&frame)
    }

    /// Project, draw, and present one frame through the backend sink.
    pub fn render(&mut self) -> BackendResult {
        self.draw()?;
        self.backend.present()
    }

    /// Advance by `frame_secs` and render the resulting frame.
    pub fn advance_and_render(&mut self, frame_secs: f64) -> BackendResult {
        self.advance(frame_secs);
        self.render()
    }

    /// Consume the session and return its backend.
    pub fn into_backend(self) -> B {
        self.backend
    }

    fn project(&self) -> SceneFrame {
        self.controller.project(&self.prev, &self.curr)
    }

    fn step_once(&mut self) {
        self.prev = self.curr.clone();
        let output = self.sim.step();
        self.controller
            .observe_events(output.time().tick(), output.events());
        for event in output.events() {
            match event {
                Event::Spawned { .. } => self.spawned += 1,
                Event::Despawned { .. } => self.despawned += 1,
                // Safety and control records change no population counter.
                Event::Yielded { .. }
                | Event::Collision { .. }
                | Event::NearMiss { .. }
                | Event::Violation { .. }
                | Event::Entry { .. }
                | Event::Exit { .. }
                | Event::Queue { .. }
                | Event::ControlTransition { .. }
                | Event::Maneuver { .. }
                | Event::FacilityTransition { .. }
                | Event::OpposingTraversal { .. }
                | Event::ClosePass { .. }
                | Event::ArticulationLimitExceeded { .. } => {}
            }
        }
        self.curr = self.sim.snapshot(SnapshotDetail::Full);
    }

    fn restart(&mut self, seed: u64) {
        let Ok(sim) = Simulation::new((*self.scenario).clone(), RunConfig::new(seed)) else {
            return;
        };
        let snapshot = sim.snapshot(SnapshotDetail::Full);

        self.controller.reset_clock(sim.config().step().as_secs());
        self.controller.clear_selection();

        self.seed = seed;
        self.sim = sim;
        self.prev = snapshot.clone();
        self.curr = snapshot;
        self.spawned = 0;
        self.despawned = 0;
    }
}

/// World position of the next agent after the current selection, or the first
/// agent when nothing is selected.
fn next_selection(frame: &SceneFrame) -> Option<DVec2> {
    let bodies = &frame.bodies;
    let index = match frame.status.selection {
        Some(current) => bodies
            .iter()
            .position(|body| body.id == current)
            .map_or(0, |index| (index + 1) % bodies.len()),
        None => 0,
    };
    bodies.get(index).map(|body| body.position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hekate_model::PathId;
    use hekate_present::{Overlay, load_scenario};

    fn scenario() -> Arc<CompiledScenario> {
        let source = hekate_model::parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
             population: { vehicle_count: 3, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        Arc::new(CompiledScenario::compile(source).expect("scenario compiles"))
    }

    fn session() -> TuiSession<CellBackend<Vec<u8>>> {
        TuiSession::new(scenario(), 0, Vec::new(), ColorDepth::Truecolor).expect("session starts")
    }

    #[test]
    fn a_fresh_session_fits_the_viewport_to_the_scene_area() {
        let session = session();
        assert_eq!(session.tick(), 0);
        assert_eq!(session.seed(), 0);
        assert!(!session.paused());
        assert_eq!(session.speed(), Speed::Real);
        assert!(session.viewport().scale() > 0.0);
    }

    #[test]
    fn commands_mutate_only_presentation_state() {
        let mut session = session();
        session.apply(ViewCommand::TogglePause);
        assert!(session.paused());
        session.apply(ViewCommand::SetSpeed(Speed::Fast));
        assert_eq!(session.speed(), Speed::Fast);
        session.apply(ViewCommand::ToggleOverlay(Overlay::Vectors));
        assert!(session.controller.overlays().vectors);

        let before = session.viewport().center();
        session.apply(ViewCommand::Pan(DVec2::new(2.0, -1.0)));
        assert_ne!(session.viewport().center(), before);
        assert_eq!(session.tick(), 0);
    }

    #[test]
    fn restarting_with_the_next_seed_advances_the_seed_and_clears_the_clock() {
        let mut session = session();
        session.advance(0.5);
        assert!(session.tick() > 0);
        session.apply(ViewCommand::Restart(RestartMode::NextSeed));
        assert_eq!(session.seed(), 1);
        assert_eq!(session.tick(), 0);
        session.apply(ViewCommand::Restart(RestartMode::SameSeed));
        assert_eq!(session.seed(), 1);
        assert_eq!(session.tick(), 0);
    }

    #[test]
    fn selecting_cycles_through_live_agents() {
        let mut session = session();
        // Spawn every agent before selecting.
        session.advance(0.05);
        session.select_next();
        assert!(session.selection().is_some());
        let first = session.selection();
        session.select_next();
        let second = session.selection();
        assert_ne!(first, second);
    }

    /// The wrong-way trigger names nothing without a selection, and refuses an
    /// agent that carries no route state.
    #[test]
    fn the_wrong_way_trigger_needs_a_selected_agent_with_route_state() {
        let mut session = session();
        assert!(
            !session.request_wrong_way_entry(),
            "an empty selection asks the kernel for nothing"
        );
        session.advance(0.05);
        session.select_next();
        assert!(session.selection().is_some());
        assert!(
            !session.request_wrong_way_entry(),
            "a Phase 1 body carries no route state to enter against"
        );
    }

    /// The wrong-way trigger records the kernel request for the selected agent,
    /// and the kernel's next step then opens the opposing traversal the
    /// wrong-way overlay draws.
    #[test]
    fn the_wrong_way_trigger_opens_the_selected_agents_opposing_traversal() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scenarios/phase2/inc2/narrow_wrong_way_v2.json5");
        let scenario = Arc::new(load_scenario(&path).expect("the fixture loads"));
        assert_eq!(
            scenario.id_map().path_name(PathId::from_index(0)),
            Some("guide_a"),
            "corridor a's guide path is the fixture's first path"
        );
        let mut session =
            TuiSession::new(scenario, 0, Vec::new(), ColorDepth::Truecolor).expect("starts");
        assert!(
            !session.request_wrong_way_entry(),
            "an empty selection asks the kernel for nothing"
        );

        // Step until corridor a holds its lone rider inside the window the
        // Increment 2 suite records that corridor's entry from, so the policy
        // the fixture authors selects the opposing option.
        let mut found: Option<(usize, DVec2)> = None;
        for _ in 0..400 {
            session.advance(0.5);
            let snapshot = session.simulation().snapshot(SnapshotDetail::Full);
            let on_corridor: Vec<&hekate_sim::AgentSample> = snapshot
                .agents()
                .iter()
                .filter(|sample| {
                    sample
                        .motion
                        .as_ref()
                        .is_some_and(|motion| motion.path == PathId::from_index(0))
                })
                .collect();
            if on_corridor.len() == 1 {
                let sample = on_corridor[0];
                let in_window = sample
                    .motion
                    .as_ref()
                    .is_some_and(|motion| (5.0..=20.0).contains(&motion.path_distance_m));
                if in_window {
                    found = Some((sample.id.index(), sample.position));
                    break;
                }
            }
        }
        let (id, position) = found.expect("corridor a admits a lone rider into the entry window");

        session.apply(ViewCommand::SelectNearest(position));
        assert_eq!(
            session.selection(),
            Some(id),
            "the corridor's rider is the selected agent"
        );
        assert!(
            session.request_wrong_way_entry(),
            "the rider's route state admits a wrong-way entry"
        );

        // The kernel's wrong-way pass runs at the start of the next step, so
        // the inspector reports the open interval within the following ticks.
        session.advance(0.5);
        session.draw().expect("the session draws");
        let footer = session.backend().footer_line();
        assert!(
            footer.starts_with(&format!("Agent #{id}")),
            "the inspector still describes the turned rider: {footer}"
        );
        assert!(
            footer.contains("opposing facility"),
            "the inspector must report the open opposing traversal: {footer}"
        );
    }

    #[test]
    fn advancing_one_second_takes_twenty_fixed_steps() {
        let mut session = session();
        let ticks = session.advance(1.0);
        assert_eq!(ticks, 20);
        assert_eq!(session.tick(), 20);
        assert!((session.time_seconds() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn command_clock_partitioning_does_not_change_final_ticks() {
        let mut one = session();
        one.advance(1.0);

        let mut many = session();
        for _ in 0..100 {
            many.advance(0.01);
        }
        assert_eq!(one.tick(), many.tick());
        assert_eq!(
            format!("{:?}", one.simulation().snapshot(SnapshotDetail::Full)),
            format!("{:?}", many.simulation().snapshot(SnapshotDetail::Full)),
        );
    }
}
