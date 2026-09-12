//! The renderer-backend contract.
//!
//! Every backend consumes one [`SceneFrame`] and produces no
//! simulation-affecting state. Capabilities let a host choose a backend and
//! decide which features are safe to use.

use crate::scene::SceneFrame;

/// Failure reported by a renderer backend.
pub type BackendResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Features a backend can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackendCapabilities {
    /// Renders to a character grid rather than pixels.
    pub character_cells: bool,
    /// Emits 24-bit color.
    pub truecolor: bool,
    /// Can place Kitty graphics images.
    pub kitty_graphics: bool,
    /// Receives pointer input.
    pub pointer_input: bool,
    /// Uses the terminal alternate screen.
    pub alternate_screen: bool,
}

impl BackendCapabilities {
    /// A GPU-backed graphical backend, as used by the Bevy viewer.
    pub const GPU: Self = Self {
        character_cells: false,
        truecolor: true,
        kitty_graphics: false,
        pointer_input: true,
        alternate_screen: false,
    };

    /// The universal character-cell fallback backend.
    pub const CHARACTER_CELLS: Self = Self {
        character_cells: true,
        truecolor: false,
        kitty_graphics: false,
        pointer_input: true,
        alternate_screen: true,
    };
}

/// One renderer backend.
///
/// A backend may hold GPU resources or terminal state, but it never owns the
/// playback clock and never mutates the simulation directly. The presentation
/// controller hands it a fully projected [`SceneFrame`].
pub trait RendererBackend {
    /// Declare the features this backend supports.
    fn capabilities(&self) -> BackendCapabilities;

    /// Resize the render target to `width` by `height` units (pixels for a
    /// graphical backend, cells for a terminal backend).
    fn resize(&mut self, width: u32, height: u32) -> BackendResult;

    /// Consume one projected frame.
    fn draw(&mut self, frame: &SceneFrame) -> BackendResult;

    /// Submit the most recently drawn frame for display.
    fn present(&mut self) -> BackendResult;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use glam::DVec2;
    use tangle_model::{CompiledScenario, parse_scenario_source};
    use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

    use crate::clock::Speed;
    use crate::scene::{FrameStatus, Overlays, SceneGeometry, Viewport};

    /// A minimal backend that records what it was asked to draw.
    #[derive(Default)]
    struct RecordingBackend {
        resized: Option<(u32, u32)>,
        drawn: usize,
        presented: usize,
    }

    impl RendererBackend for RecordingBackend {
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::CHARACTER_CELLS
        }

        fn resize(&mut self, width: u32, height: u32) -> BackendResult {
            self.resized = Some((width, height));
            Ok(())
        }

        fn draw(&mut self, _frame: &SceneFrame) -> BackendResult {
            self.drawn += 1;
            Ok(())
        }

        fn present(&mut self) -> BackendResult {
            self.presented += 1;
            Ok(())
        }
    }

    fn frame() -> SceneFrame {
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
            time_seconds: 0.0,
            tick: 0,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Real,
                paused: true,
                selection: None,
            },
            viewport: Viewport::new(DVec2::ZERO, 1.0),
            geometry: Arc::new(SceneGeometry::from_scenario(&scenario)),
            bodies: Vec::new(),
            overlays: Overlays::default(),
        }
    }

    #[test]
    fn a_backend_consumes_a_scene_frame_without_kernel_state() {
        let mut backend = RecordingBackend::default();
        assert_eq!(backend.capabilities(), BackendCapabilities::CHARACTER_CELLS);
        backend.resize(80, 24).expect("resize succeeds");
        backend.draw(&frame()).expect("draw succeeds");
        backend.present().expect("present succeeds");
        assert_eq!(backend.resized, Some((80, 24)));
        assert_eq!(backend.drawn, 1);
        assert_eq!(backend.presented, 1);
    }
}
