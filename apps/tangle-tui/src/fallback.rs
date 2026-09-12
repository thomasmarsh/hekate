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

use crate::capability::BackendKind;
use crate::terminal::TerminalModes;
use tangle_present::{BackendResult, RendererBackend, SceneFrame};

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

#[cfg(test)]
mod tests {
    use super::*;

    use std::io;
    use std::sync::Arc;

    use glam::DVec2;
    use tangle_model::{CompiledScenario, parse_scenario_source};
    use tangle_present::{
        BackendCapabilities, FrameStatus, Overlays, SceneGeometry, Speed, Viewport,
    };
    use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

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
}
