//! Safe failover from an opt-in backend to character cells.
//!
//! The opt-in Kitty backend can fail partway through a run: a terminal that
//! answered the support probe `OK` may still abort or reject a transmission.
//! When that happens the viewer must restore the terminal and continue in
//! character cells without losing its place.
//!
//! [`BackendFallback`] is that switch. Playback position and selection live in
//! the presentation controller, outside this type, so the same [`SceneFrame`]
//! is replayed to the fallback and the viewer's state is preserved.

use crate::backend::RunInfo;
use crate::capability::BackendKind;
use crate::session::SessionBackend;
use crate::terminal::TerminalModes;
use hekate_present::{BackendCapabilities, BackendResult, RendererBackend, SceneFrame};

/// Tracks which backend is active while a run may fall back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendFallback {
    active: BackendKind,
    fell_back: bool,
}

impl BackendFallback {
    /// Start a run on `active`.
    pub const fn new(active: BackendKind) -> Self {
        Self {
            active,
            fell_back: false,
        }
    }

    /// The backend currently in use.
    pub const fn active(&self) -> BackendKind {
        self.active
    }

    /// Whether the run has switched away from its starting backend.
    pub const fn fell_back(&self) -> bool {
        self.fell_back
    }

    /// Draw and present `frame` through the active backend, switching to
    /// `fallback` when `primary` fails.
    ///
    /// The switch restores the terminal before the fallback draws, so the
    /// fallback starts from a clean alternate screen, cursor, and input mode
    /// rather than inheriting a half-written frame. The primary's error is
    /// intentionally not propagated: the fallback is expected to recover.
    pub fn render<P, F>(
        &mut self,
        frame: &SceneFrame,
        primary: &mut P,
        fallback: &mut F,
        terminal: &mut dyn TerminalModes,
    ) -> BackendResult
    where
        P: RendererBackend,
        F: RendererBackend,
    {
        if self.active == BackendKind::Ascii {
            fallback.draw(frame)?;
            return fallback.present();
        }
        match primary.draw(frame).and_then(|()| primary.present()) {
            Ok(()) => Ok(()),
            Err(_) => {
                terminal.restore()?;
                self.active = BackendKind::Ascii;
                self.fell_back = true;
                fallback.draw(frame)?;
                fallback.present()
            }
        }
    }
}

/// A terminal session's live backend: an opt-in primary plus the character-cell
/// fallback it can switch to.
///
/// The pair is the single [`SessionBackend`] a [`crate::session::TuiSession`]
/// owns. It buffers the frame from `draw` and runs it through
/// [`BackendFallback::render`] in `present`, so a primitive backend failure
/// restores the terminal and replays the same frame in cells without the
/// session, the controller, or the kernel noticing.
pub struct BackendPair<C, K, T> {
    cells: C,
    kitty: K,
    fallback: BackendFallback,
    terminal: T,
    pending: Option<SceneFrame>,
}

impl<C: SessionBackend, K: SessionBackend, T: TerminalModes> BackendPair<C, K, T> {
    /// Pair a character-cell backend with an opt-in backend, starting on
    /// `active`.
    pub fn new(cells: C, kitty: K, terminal: T, active: BackendKind) -> Self {
        Self {
            cells,
            kitty,
            fallback: BackendFallback::new(active),
            terminal,
            pending: None,
        }
    }

    /// The backend currently rendering.
    pub const fn active(&self) -> BackendKind {
        self.fallback.active()
    }

    /// Whether the run has switched away from its starting backend.
    pub const fn fell_back(&self) -> bool {
        self.fallback.fell_back()
    }
}

impl<C: SessionBackend, K: SessionBackend, T: TerminalModes> RendererBackend
    for BackendPair<C, K, T>
{
    fn capabilities(&self) -> BackendCapabilities {
        match self.fallback.active() {
            BackendKind::Kitty => self.kitty.capabilities(),
            BackendKind::Ascii => self.cells.capabilities(),
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> BackendResult {
        self.cells.resize(width, height)?;
        self.kitty.resize(width, height)
    }

    fn draw(&mut self, frame: &SceneFrame) -> BackendResult {
        self.pending = Some(frame.clone());
        Ok(())
    }

    fn present(&mut self) -> BackendResult {
        let Some(frame) = self.pending.take() else {
            return Ok(());
        };
        self.fallback
            .render(&frame, &mut self.kitty, &mut self.cells, &mut self.terminal)
    }
}

impl<C: SessionBackend, K: SessionBackend, T: TerminalModes> SessionBackend
    for BackendPair<C, K, T>
{
    fn set_run_info(&mut self, run: RunInfo) {
        self.cells.set_run_info(run);
        self.kitty.set_run_info(run);
    }

    fn resize_terminal(&mut self, columns: u32, rows: u32) -> BackendResult {
        self.cells.resize_terminal(columns, rows)?;
        self.kitty.resize_terminal(columns, rows)
    }

    fn scene_extent(&self) -> (f64, f64) {
        match self.fallback.active() {
            BackendKind::Kitty => self.kitty.scene_extent(),
            BackendKind::Ascii => self.cells.scene_extent(),
        }
    }

    fn shutdown(&mut self) -> BackendResult {
        self.kitty.shutdown()?;
        self.cells.shutdown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io;
    use std::sync::Arc;

    use glam::DVec2;
    use hekate_model::{CompiledScenario, parse_scenario_source};
    use hekate_present::{
        BackendCapabilities, FrameStatus, Overlays, SafetyOverlay, SceneGeometry, Speed, Viewport,
    };
    use hekate_sim::{RunConfig, Simulation, SnapshotDetail};

    use crate::backend::CellBackend;
    use crate::backend::RunInfo;
    use crate::kitty::{KittyBackend, Multiplexer};
    use crate::palette::ColorDepth;
    use crate::session::TuiSession;
    use hekate_present::ViewCommand;

    /// A backend that records the ticks and selections it sees and can be told
    /// to fail at either stage.
    #[derive(Default)]
    struct FakeBackend {
        capabilities: BackendCapabilities,
        ticks: Vec<u64>,
        selections: Vec<Option<usize>>,
        presented: usize,
        fail_draw: bool,
        fail_present: bool,
    }

    impl FakeBackend {
        fn kitty() -> Self {
            Self {
                capabilities: BackendCapabilities {
                    kitty_graphics: true,
                    ..BackendCapabilities::CHARACTER_CELLS
                },
                ..Self::default()
            }
        }

        fn cells() -> Self {
            Self {
                capabilities: BackendCapabilities::CHARACTER_CELLS,
                ..Self::default()
            }
        }

        fn failing() -> Self {
            Self {
                fail_draw: true,
                ..Self::kitty()
            }
        }
    }

    impl RendererBackend for FakeBackend {
        fn capabilities(&self) -> BackendCapabilities {
            self.capabilities
        }

        fn resize(&mut self, _width: u32, _height: u32) -> BackendResult {
            Ok(())
        }

        fn draw(&mut self, frame: &SceneFrame) -> BackendResult {
            if self.fail_draw {
                return Err(io::Error::other("primary draw failed").into());
            }
            self.ticks.push(frame.tick);
            self.selections.push(frame.status.selection);
            Ok(())
        }

        fn present(&mut self) -> BackendResult {
            if self.fail_present {
                return Err(io::Error::other("primary present failed").into());
            }
            self.presented += 1;
            Ok(())
        }
    }

    impl SessionBackend for FakeBackend {
        fn set_run_info(&mut self, _run: RunInfo) {}

        fn resize_terminal(&mut self, _columns: u32, _rows: u32) -> BackendResult {
            Ok(())
        }

        fn scene_extent(&self) -> (f64, f64) {
            (80.0, 48.0)
        }
    }

    /// A terminal that records how often it was entered and restored.
    #[derive(Default)]
    struct FakeTerminal {
        enters: usize,
        restores: usize,
    }

    impl TerminalModes for FakeTerminal {
        fn enter(&mut self) -> io::Result<()> {
            self.enters += 1;
            Ok(())
        }

        fn restore(&mut self) -> io::Result<()> {
            self.restores += 1;
            Ok(())
        }
    }

    fn frame(tick: u64, selection: Option<usize>) -> SceneFrame {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] } ], \
             portals: [ { id: 'a', path: 'guide', end: 'start', width_m: 3.0 }, \
             { id: 'b', path: 'guide', end: 'end', width_m: 3.0 } ], \
             population: { vehicle_count: 1, vehicle_speed_mps: 5.0, vehicle_spacing_m: 5.0, \
             vehicle_length_m: 4.0, vehicle_width_m: 2.0 } }",
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let sim = Simulation::new(scenario.clone(), RunConfig::new(0)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        SceneFrame {
            scenario_id: scenario.id().to_owned(),
            time_seconds: tick as f64 / 20.0,
            tick,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Real,
                paused: false,
                selection,
            },
            viewport: Viewport::new(DVec2::ZERO, 1.0),
            geometry: Arc::new(SceneGeometry::from_scenario(&scenario)),
            bodies: Vec::new(),
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
        }
    }

    #[test]
    fn a_healthy_primary_never_touches_the_fallback() {
        let mut fallback = BackendFallback::new(BackendKind::Kitty);
        let mut primary = FakeBackend::kitty();
        let mut cells = FakeBackend::cells();
        let mut terminal = FakeTerminal::default();

        fallback
            .render(&frame(7, Some(2)), &mut primary, &mut cells, &mut terminal)
            .expect("primary renders");

        assert_eq!(fallback.active(), BackendKind::Kitty);
        assert!(!fallback.fell_back());
        assert_eq!(primary.presented, 1);
        assert!(cells.ticks.is_empty());
        assert_eq!(terminal.restores, 0);
    }

    #[test]
    fn a_failing_primary_restores_the_terminal_and_renders_the_same_frame() {
        let mut fallback = BackendFallback::new(BackendKind::Kitty);
        let mut primary = FakeBackend::failing();
        let mut cells = FakeBackend::cells();
        let mut terminal = FakeTerminal::default();

        fallback
            .render(&frame(42, Some(3)), &mut primary, &mut cells, &mut terminal)
            .expect("fallback recovers");

        assert_eq!(fallback.active(), BackendKind::Ascii);
        assert!(fallback.fell_back());
        assert_eq!(terminal.restores, 1, "terminal must be restored first");
        // Playback position and selection survive the switch.
        assert_eq!(cells.ticks, vec![42]);
        assert_eq!(cells.selections, vec![Some(3)]);
        assert_eq!(cells.presented, 1);
    }

    #[test]
    fn a_failing_present_also_falls_back() {
        let mut fallback = BackendFallback::new(BackendKind::Kitty);
        let mut primary = FakeBackend {
            fail_present: true,
            ..FakeBackend::kitty()
        };
        let mut cells = FakeBackend::cells();
        let mut terminal = FakeTerminal::default();

        fallback
            .render(&frame(1, None), &mut primary, &mut cells, &mut terminal)
            .expect("fallback recovers");

        assert!(fallback.fell_back());
        assert_eq!(terminal.restores, 1);
        assert_eq!(cells.presented, 1);
    }

    #[test]
    fn once_fallen_back_the_primary_is_not_retried() {
        let mut fallback = BackendFallback::new(BackendKind::Kitty);
        let mut primary = FakeBackend::failing();
        let mut cells = FakeBackend::cells();
        let mut terminal = FakeTerminal::default();

        fallback
            .render(&frame(1, None), &mut primary, &mut cells, &mut terminal)
            .expect("fallback recovers");
        fallback
            .render(&frame(2, None), &mut primary, &mut cells, &mut terminal)
            .expect("fallback continues");

        assert_eq!(terminal.restores, 1, "only the first switch restores");
        assert_eq!(cells.ticks, vec![1, 2]);
    }

    #[test]
    fn a_run_that_starts_in_cells_never_touches_the_primary() {
        let mut fallback = BackendFallback::new(BackendKind::Ascii);
        let mut primary = FakeBackend::failing();
        let mut cells = FakeBackend::cells();
        let mut terminal = FakeTerminal::default();

        fallback
            .render(&frame(9, None), &mut primary, &mut cells, &mut terminal)
            .expect("cells render");

        assert!(!fallback.fell_back());
        assert_eq!(terminal.restores, 0);
        assert_eq!(cells.ticks, vec![9]);
    }

    #[test]
    fn a_pair_replays_the_same_frame_in_cells_after_a_kitty_failure() {
        let mut pair = BackendPair::new(
            FakeBackend::cells(),
            FakeBackend::failing(),
            FakeTerminal::default(),
            BackendKind::Kitty,
        );
        pair.draw(&frame(5, Some(1)))
            .expect("draw buffers the frame");
        pair.present().expect("pair falls back");

        assert_eq!(pair.active(), BackendKind::Ascii);
        assert!(pair.fell_back());
        assert!(pair.capabilities().character_cells);
    }

    #[test]
    fn a_healthy_pair_stays_on_kitty() {
        let mut pair = BackendPair::new(
            FakeBackend::cells(),
            FakeBackend::kitty(),
            FakeTerminal::default(),
            BackendKind::Kitty,
        );
        pair.draw(&frame(1, None)).expect("draw buffers the frame");
        pair.present().expect("kitty renders");

        assert_eq!(pair.active(), BackendKind::Kitty);
        assert!(!pair.fell_back());
        assert!(pair.capabilities().kitty_graphics);
    }

    fn walking_scenario() -> Arc<CompiledScenario> {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
             population: { vehicle_count: 4, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("parses");
        Arc::new(CompiledScenario::compile(source).expect("compiles"))
    }

    #[test]
    fn a_session_keeps_playback_state_across_a_real_capability_fallback() {
        // A GNU screen multiplexer cannot carry a Kitty graphics frame, so the
        // real Kitty backend declines and the pair must switch to cells.
        let scenario = walking_scenario();
        let cells = CellBackend::new(Vec::new(), ColorDepth::Truecolor, Arc::clone(&scenario));
        let kitty = KittyBackend::with_mux(Vec::new(), Arc::clone(&scenario), Multiplexer::Screen);
        let pair = BackendPair::new(cells, kitty, FakeTerminal::default(), BackendKind::Kitty);
        let mut session =
            TuiSession::with_backend(Arc::clone(&scenario), 0, pair).expect("session starts");
        session.resize(80, 24);

        session.advance(0.25);
        session.apply(ViewCommand::SetSpeed(Speed::Fast));
        session.apply(ViewCommand::TogglePause);
        session.select_next();
        let tick = session.tick();
        let selection = session.selection();
        assert!(selection.is_some(), "an agent must be selectable");

        session
            .advance_and_render(0.1)
            .expect("the pair falls back to cells");

        assert!(session.backend().fell_back());
        assert_eq!(session.backend().active(), BackendKind::Ascii);
        assert_eq!(session.tick(), tick, "fallback must not move the clock");
        assert_eq!(session.speed(), Speed::Fast);
        assert!(session.paused());
        assert_eq!(session.selection(), selection);
    }
}
