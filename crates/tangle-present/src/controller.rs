//! The presentation controller: the one place playback state lives.
//!
//! The controller owns the playback clock, the viewport, the selection, and
//! the overlay flags, and it projects kernel snapshots into backend-agnostic
//! [`SceneFrame`]s. Every backend applies the same [`ViewCommand`] stream here,
//! so backend choice cannot change which ticks the kernel visits.

use std::sync::Arc;

use glam::DVec2;
use tangle_sim::{Event, Snapshot};

use crate::clock::PresentationClock;
use crate::command::{RestartMode, ViewCommand};
use crate::safety::{MARKER_LIFETIME_SECONDS, SafetyOverlay};
use crate::scene::{FrameStatus, Overlays, SceneBody, SceneFrame, SceneGeometry, Viewport};

/// Outcome of applying a [`ViewCommand`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The command changed only presentation state.
    None,
    /// The host must rebuild the simulation for this restart mode.
    Restart(RestartMode),
}

/// Owns all presentation state and projects frames for any backend.
#[derive(Debug, Clone)]
pub struct PresentationController {
    geometry: Arc<SceneGeometry>,
    clock: PresentationClock,
    viewport: Viewport,
    selection: Option<usize>,
    overlays: Overlays,
    safety: SafetyOverlay,
}

impl PresentationController {
    /// Create a controller for a fixed step, starting with the default
    /// overlays and no selection.
    pub fn new(geometry: SceneGeometry, step_secs: f64, viewport: Viewport) -> Self {
        Self {
            geometry: Arc::new(geometry),
            clock: PresentationClock::new(step_secs),
            viewport,
            selection: None,
            overlays: Overlays::default(),
            safety: SafetyOverlay::new(marker_lifetime_ticks(step_secs)),
        }
    }

    /// Static scenario geometry shared by every projected frame.
    pub fn geometry(&self) -> &SceneGeometry {
        &self.geometry
    }

    /// Current viewport.
    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Replace the viewport, for example after a window resize or initial fit.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    /// Current playback speed.
    pub const fn speed(&self) -> crate::clock::Speed {
        self.clock.speed()
    }

    /// Whether playback is paused.
    pub const fn paused(&self) -> bool {
        self.clock.is_paused()
    }

    /// Currently selected agent, if any.
    pub const fn selection(&self) -> Option<usize> {
        self.selection
    }

    /// Which overlays are enabled.
    pub const fn overlays(&self) -> Overlays {
        self.overlays
    }

    /// The safety records and folded states the next frame projects.
    pub const fn safety(&self) -> &SafetyOverlay {
        &self.safety
    }

    /// Fold one completed step's records into the projected safety overlays.
    ///
    /// Call once per kernel step, with the tick that step produced, before
    /// projecting the frame for it. The fold is deterministic and reads only
    /// the records it is given, so the same run always projects the same
    /// markers, emphasis, and occupancy.
    pub fn observe_events(&mut self, tick: u64, events: &[Event]) {
        self.safety.observe(tick, events);
    }

    /// Consume frame time and return the whole steps to take now.
    pub fn advance(&mut self, frame_secs: f64) -> u64 {
        self.clock.advance(frame_secs)
    }

    /// Rebuild the clock after a restart, preserving speed and pause state.
    ///
    /// The event stream belongs to the run, so a restart also starts the
    /// safety window and its folded states over; the marker lifetime follows
    /// the new step.
    pub fn reset_clock(&mut self, step_secs: f64) {
        let speed = self.clock.speed();
        let paused = self.clock.is_paused();
        self.clock = PresentationClock::new(step_secs);
        self.clock.set_speed(speed);
        self.clock.set_paused(paused);
        self.safety = SafetyOverlay::new(marker_lifetime_ticks(step_secs));
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Apply one normalized command. Restarts are reported to the host, which
    /// owns the simulation and the seed.
    pub fn apply(&mut self, command: &ViewCommand, frame: &SceneFrame) -> Applied {
        match *command {
            ViewCommand::TogglePause => self.clock.toggle_paused(),
            ViewCommand::SetPaused(paused) => self.clock.set_paused(paused),
            ViewCommand::SingleStep => self.clock.request_single_tick(),
            ViewCommand::SetSpeed(speed) => self.clock.set_speed(speed),
            ViewCommand::Restart(mode) => return Applied::Restart(mode),
            ViewCommand::Pan(delta) => self.viewport.pan(delta),
            ViewCommand::Zoom(factor) => self.viewport.zoom(factor),
            ViewCommand::SelectNearest(point) => {
                self.selection = nearest_body(frame, point).map(|body| body.id);
            }
            ViewCommand::ClearSelection => self.selection = None,
            ViewCommand::ToggleOverlay(overlay) => self.overlays.toggle(overlay),
        }
        Applied::None
    }

    /// Project one frame from the previous and current kernel snapshots,
    /// interpolating bodies by the clock's current sub-step progress.
    ///
    /// The safety overlays come from the records the host folded in with
    /// [`Self::observe_events`], windowed to this frame's tick, so the frame
    /// alone describes everything a backend draws.
    pub fn project(&self, previous: &Snapshot, current: &Snapshot) -> SceneFrame {
        let alpha = self.clock.alpha();
        let bodies: Vec<SceneBody> = current
            .agents()
            .iter()
            .map(|sample| SceneBody::project(previous.agents(), sample, alpha))
            .collect();

        SceneFrame {
            scenario_id: current.scenario_id().to_owned(),
            time_seconds: current.time().seconds(),
            tick: current.time().tick(),
            status: FrameStatus {
                agents: bodies.len(),
                speed: self.clock.speed(),
                paused: self.clock.is_paused(),
                selection: self.selection,
            },
            viewport: self.viewport,
            geometry: Arc::clone(&self.geometry),
            bodies,
            overlays: self.overlays,
            safety: self.safety.windowed_at(current.time().tick()),
        }
    }
}

/// Whole ticks a safety record stays in the marker window at `step_secs`.
///
/// One tick is the floor, so a step longer than the lifetime still shows a
/// marker for the tick that produced it.
fn marker_lifetime_ticks(step_secs: f64) -> u64 {
    if !step_secs.is_finite() || step_secs <= 0.0 {
        return 1;
    }
    (MARKER_LIFETIME_SECONDS / step_secs).ceil().max(1.0) as u64
}

/// The nearest body to `point` within the viewport's selection radius.
fn nearest_body(frame: &SceneFrame, point: DVec2) -> Option<&SceneBody> {
    let radius = frame.viewport.select_radius();
    frame
        .bodies
        .iter()
        .map(|body| (body, body.position.distance(point)))
        .filter(|(_, distance)| *distance <= radius)
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(body, _)| body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ViewCommand;
    use crate::safety::BodyEmphasis;
    use crate::scene::Overlay;
    use tangle_model::CompiledScenario;
    use tangle_sim::{AgentId, Event, RunConfig, Simulation, SnapshotDetail, ViolationKind};

    const STEP: f64 = 0.05;

    fn scenario() -> CompiledScenario {
        let source = tangle_model::parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 60, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 3.5 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 3.5 } ], \
             population: { vehicle_count: 3, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        CompiledScenario::compile(source).expect("scenario compiles")
    }

    fn controller() -> PresentationController {
        let geometry = SceneGeometry::from_scenario(&scenario());
        PresentationController::new(geometry, STEP, Viewport::new(DVec2::ZERO, 1.0))
    }

    #[test]
    fn project_interpolates_and_carries_scenario_and_status() {
        let mut sim = Simulation::new(scenario(), RunConfig::new(0)).expect("builds");
        let previous = sim.snapshot(SnapshotDetail::Full);
        sim.step();
        let current = sim.snapshot(SnapshotDetail::Full);

        let mut controller = controller();
        controller.advance(STEP / 2.0);
        let frame = controller.project(&previous, &current);

        assert_eq!(frame.scenario_id, "walking");
        assert_eq!(frame.tick, 1);
        assert_eq!(frame.status.agents, 3);
        assert_eq!(frame.bodies.len(), 3);
        // Half a step interpolates halfway between the two snapshots.
        let prior = previous.agents()[0].position;
        let now = current.agents()[0].position;
        assert!((frame.bodies[0].position - (prior + now) * 0.5).length() < 1e-9);
    }

    #[test]
    fn observed_records_reach_the_frame_and_a_restart_clears_them() {
        let mut controller = controller();
        let mut sim = Simulation::new(scenario(), RunConfig::new(0)).expect("builds");
        let initial = sim.snapshot(SnapshotDetail::Full);
        let empty = controller.project(&initial, &initial);
        assert!(empty.safety_markers().is_empty());
        assert!(empty.body_emphasis().is_empty());

        let events = [
            Event::Queue {
                agent: AgentId::from_index(0),
                joined: true,
            },
            Event::Violation {
                agent: AgentId::from_index(1),
                kind: ViolationKind::RanRedLight,
            },
        ];
        controller.observe_events(0, &events);
        let frame = controller.project(&initial, &initial);
        // Both records sit at the frame's own tick and draw a marker.
        assert_eq!(frame.safety_markers().len(), 2);
        assert_eq!(
            frame.body_emphasis(),
            vec![(0, BodyEmphasis::Queue), (1, BodyEmphasis::Violation)]
        );
        assert_eq!(frame.events_involving(1).len(), 1);
        assert_eq!(controller.safety().queued(), &[0]);
        // Projecting is a pure read of the folded state.
        assert_eq!(frame, controller.project(&initial, &initial));

        // A record older than the marker window stops drawing a marker, while
        // the open queue state it opened lasts until its closing edge.
        for _ in 0..(marker_lifetime_ticks(STEP) + 1) {
            sim.step();
        }
        let later = sim.snapshot(SnapshotDetail::Full);
        let frame = controller.project(&later, &later);
        assert!(frame.safety_markers().is_empty());
        assert_eq!(frame.body_emphasis(), vec![(0, BodyEmphasis::Queue)]);

        // A restart starts the run's records over with it.
        controller.reset_clock(STEP);
        assert!(controller.safety().is_empty());
        let frame = controller.project(&later, &later);
        assert!(frame.safety_markers().is_empty());
        assert!(frame.body_emphasis().is_empty());
    }

    #[test]
    fn commands_mutate_only_presentation_state() {
        let mut controller = controller();
        let frame = controller.project(
            &Simulation::new(scenario(), RunConfig::new(0))
                .expect("builds")
                .snapshot(SnapshotDetail::Full),
            &Simulation::new(scenario(), RunConfig::new(0))
                .expect("builds")
                .snapshot(SnapshotDetail::Full),
        );

        assert_eq!(controller.speed(), crate::clock::Speed::Real);
        controller.apply(&ViewCommand::SetSpeed(crate::clock::Speed::Fast), &frame);
        assert_eq!(controller.speed(), crate::clock::Speed::Fast);

        controller.apply(&ViewCommand::TogglePause, &frame);
        assert!(controller.paused());

        controller.apply(&ViewCommand::ToggleOverlay(Overlay::Vectors), &frame);
        assert!(controller.overlays().vectors);

        controller.apply(&ViewCommand::Pan(DVec2::new(1.0, 2.0)), &frame);
        controller.apply(&ViewCommand::Zoom(2.0), &frame);
        assert_eq!(controller.viewport().center(), DVec2::new(1.0, 2.0));
        assert!((controller.viewport().scale() - 2.0).abs() < 1e-12);

        assert_eq!(
            controller.apply(&ViewCommand::Restart(RestartMode::NextSeed), &frame),
            Applied::Restart(RestartMode::NextSeed)
        );
        // A restart request does not itself change presentation state.
        assert!(controller.paused());
    }

    #[test]
    fn select_nearest_respects_the_radius_and_clears_on_empty_space() {
        let sim = Simulation::new(scenario(), RunConfig::new(0)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        let target = snapshot.agents()[0].position;

        let mut controller = controller();
        let frame = controller.project(&snapshot, &snapshot);
        controller.apply(&ViewCommand::SelectNearest(target), &frame);
        assert_eq!(controller.selection(), Some(0));

        // Far beyond the selection radius clears the selection.
        let far = target + DVec2::splat(1_000.0);
        controller.apply(&ViewCommand::SelectNearest(far), &frame);
        assert_eq!(controller.selection(), None);
    }

    #[test]
    fn reset_clock_preserves_speed_and_pause_state() {
        let mut controller = controller();
        let frame = controller.project(
            &Simulation::new(scenario(), RunConfig::new(0))
                .expect("builds")
                .snapshot(SnapshotDetail::Full),
            &Simulation::new(scenario(), RunConfig::new(0))
                .expect("builds")
                .snapshot(SnapshotDetail::Full),
        );
        controller.apply(&ViewCommand::SetSpeed(crate::clock::Speed::Maximum), &frame);
        controller.apply(&ViewCommand::TogglePause, &frame);
        controller.reset_clock(STEP);
        assert_eq!(controller.speed(), crate::clock::Speed::Maximum);
        assert!(controller.paused());
    }
}
