//! The character-cell renderer backend.
//!
//! [`CellBackend`] implements [`RendererBackend`] for a terminal: `draw`
//! rasterizes the shared [`SceneFrame`] into a [`CellGrid`] and `present` emits
//! the status line, the grid, and the inspector/help line through a [`Write`]
//! sink. Tests capture that sink instead of a real terminal.

use std::io::{self, Write};
use std::sync::Arc;

use tangle_model::CompiledScenario;
use tangle_present::{BackendCapabilities, BackendResult, RendererBackend, SceneFrame};

use crate::grid::CellGrid;
use crate::hud::Hud;
use crate::palette::{ColorDepth, Rgb};
use crate::raster::{BACKGROUND, Rasterizer};

pub(crate) const HUD_COLOR: Rgb = Rgb::new(224, 237, 255);
pub(crate) const FOOTER_COLOR: Rgb = Rgb::new(158, 173, 199);
pub(crate) const FOOTER_SELECTED_COLOR: Rgb = Rgb::new(242, 217, 140);
/// Status lines above the scene plus the footer below it.
const RESERVED_ROWS: u32 = 3;

/// Run totals the shared [`SceneFrame`] does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunInfo {
    /// Seed the simulation is running under.
    pub seed: u64,
    /// Agents spawned since the run began.
    pub spawned: u64,
    /// Agents despawned since the run began.
    pub despawned: u64,
}

/// A terminal backend that renders to character cells through `W`.
pub struct CellBackend<W: Write> {
    writer: W,
    depth: ColorDepth,
    width: u32,
    height: u32,
    scene_rows: u32,
    hud: Hud,
    pending: Option<CellGrid>,
}

impl<W: Write> CellBackend<W> {
    /// A backend writing to `writer`, rendering `scenario` and initially sized
    /// for an 80x24 terminal.
    pub fn new(writer: W, depth: ColorDepth, scenario: Arc<CompiledScenario>) -> Self {
        Self {
            writer,
            depth,
            width: 80,
            height: 24,
            scene_rows: 21,
            hud: Hud::new(scenario),
            pending: None,
        }
    }

    /// Update the run totals shown on the status line.
    pub fn set_run_info(&mut self, run: RunInfo) {
        self.hud.set_run_info(run);
    }

    /// Current run totals.
    pub const fn run_info(&self) -> RunInfo {
        self.hud.run_info()
    }

    /// Terminal size in cells, including the status and footer rows.
    pub const fn terminal_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Size of the scene area: the terminal minus the status and footer rows.
    pub const fn scene_size(&self) -> (u32, u32) {
        (self.width, self.scene_rows)
    }

    /// Color depth this backend renders at.
    pub const fn depth(&self) -> ColorDepth {
        self.depth
    }

    /// The status lines last computed by [`Self::draw`].
    pub const fn status_lines(&self) -> &[String; 2] {
        self.hud.status_lines()
    }

    /// The character grid produced by the last [`Self::draw`], before it is
    /// serialized to the sink by [`Self::present`].
    pub fn grid(&self) -> Option<&CellGrid> {
        self.pending.as_ref()
    }

    /// The inspector or help line last computed by [`Self::draw`].
    pub fn footer_line(&self) -> &str {
        self.hud.footer_line()
    }

    /// A shared reference to the underlying sink.
    pub const fn writer(&self) -> &W {
        &self.writer
    }

    /// Consume the backend and return its sink, for test assertions.
    pub fn into_writer(self) -> W {
        self.writer
    }
}

impl<W: Write> RendererBackend for CellBackend<W> {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            character_cells: true,
            truecolor: self.depth.is_truecolor(),
            kitty_graphics: false,
            pointer_input: false,
            alternate_screen: true,
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> BackendResult {
        self.width = width.max(1);
        // Three rows are always reserved: two status lines and a footer line.
        self.height = height.max(RESERVED_ROWS);
        self.scene_rows = self.height - RESERVED_ROWS;
        Ok(())
    }

    fn draw(&mut self, frame: &SceneFrame) -> BackendResult {
        let raster = Rasterizer::new(self.width, self.scene_rows);
        self.pending = Some(raster.rasterize(frame));
        self.hud.update(frame);
        Ok(())
    }

    fn present(&mut self) -> BackendResult {
        let Some(grid) = self.pending.take() else {
            return Ok(());
        };
        let footer_color = if self.hud.selected() {
            FOOTER_SELECTED_COLOR
        } else {
            FOOTER_COLOR
        };

        // Home the cursor, then overwrite exactly `height` full-width rows so a
        // frame never scrolls the alternate screen.
        self.writer.write_all(b"\x1b[H")?;
        let status = self.hud.status_lines();
        write_row(
            &mut self.writer,
            &status[0],
            self.width,
            HUD_COLOR,
            BACKGROUND,
            self.depth,
        )?;
        self.writer.write_all(b"\r\n")?;
        write_row(
            &mut self.writer,
            &status[1],
            self.width,
            HUD_COLOR,
            BACKGROUND,
            self.depth,
        )?;
        if self.scene_rows > 0 {
            self.writer.write_all(b"\r\n")?;
            grid.write_ansi(&mut self.writer, self.depth)?;
        }
        self.writer.write_all(b"\r\n")?;
        write_row(
            &mut self.writer,
            self.hud.footer_line(),
            self.width,
            footer_color,
            BACKGROUND,
            self.depth,
        )?;
        self.writer.write_all(b"\x1b[0m")?;
        self.writer.flush()?;
        Ok(())
    }
}

/// Write one padded, colored, full-width row.
pub(crate) fn write_row<W: Write>(
    out: &mut W,
    text: &str,
    width: u32,
    fg: Rgb,
    bg: Rgb,
    depth: ColorDepth,
) -> io::Result<()> {
    write!(
        out,
        "\x1b[{};{}m",
        depth.foreground(fg),
        depth.background(bg)
    )?;
    let mut written = 0;
    for ch in text.chars() {
        if written >= width {
            break;
        }
        let mut buffer = [0u8; 4];
        out.write_all(ch.encode_utf8(&mut buffer).as_bytes())?;
        written += 1;
    }
    for _ in written..width {
        out.write_all(b" ")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use tangle_model::parse_scenario_source;
    use tangle_present::{FrameStatus, Overlays, SceneBody, SceneGeometry, Speed, Viewport};
    use tangle_sim::{RunConfig, Simulation, SnapshotDetail};

    fn scenario() -> Arc<CompiledScenario> {
        let source = parse_scenario_source(
            "{ schema_version: 1, id: 'walking', \
             coordinate_system: { x: 'east_m', y: 'north_m' }, \
             paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 120, y: 0 } ] } ], \
             portals: [ { id: 'west', path: 'guide', end: 'start', width_m: 4.0 }, \
             { id: 'east', path: 'guide', end: 'end', width_m: 4.0 } ], \
             population: { vehicle_count: 1, vehicle_speed_mps: 12.0, vehicle_spacing_m: 20.0, \
             vehicle_length_m: 4.5, vehicle_width_m: 1.8 } }",
        )
        .expect("scenario parses");
        Arc::new(CompiledScenario::compile(source).expect("scenario compiles"))
    }

    fn frame(selection: Option<usize>) -> SceneFrame {
        let compiled = scenario();
        let sim = Simulation::new((*compiled).clone(), RunConfig::new(4)).expect("builds");
        let snapshot = sim.snapshot(SnapshotDetail::Full);
        SceneFrame {
            scenario_id: compiled.id().to_owned(),
            time_seconds: 1.25,
            tick: 25,
            status: FrameStatus {
                agents: snapshot.agents().len(),
                speed: Speed::Fast,
                paused: true,
                selection,
            },
            viewport: Viewport::new(DVec2::ZERO, 0.5),
            geometry: Arc::new(SceneGeometry::from_scenario(&compiled)),
            bodies: snapshot
                .agents()
                .iter()
                .map(|sample| SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
        }
    }

    fn backend() -> CellBackend<Vec<u8>> {
        let mut backend = CellBackend::new(Vec::new(), ColorDepth::Truecolor, scenario());
        backend.resize(60, 12).expect("resize succeeds");
        backend.set_run_info(RunInfo {
            seed: 4,
            spawned: 3,
            despawned: 1,
        });
        backend
    }

    #[test]
    fn capabilities_report_character_cells_and_depth() {
        let backend = backend();
        let capabilities = backend.capabilities();
        assert!(capabilities.character_cells);
        assert!(capabilities.truecolor);
        assert!(!capabilities.kitty_graphics);
        assert!(capabilities.alternate_screen);
    }

    #[test]
    fn draw_and_present_write_status_grid_and_footer_through_the_sink() {
        let mut backend = backend();
        assert_eq!(backend.scene_size(), (60, 9));
        backend.draw(&frame(None)).expect("draw succeeds");
        backend.present().expect("present succeeds");
        let bytes = backend.into_writer();
        let text = String::from_utf8(bytes).expect("UTF-8");

        assert!(text.starts_with("\x1b[H"));
        assert!(text.contains("Tangle walking"));
        assert!(text.contains("tick 25"));
        assert!(text.contains("seed 4"));
        assert!(text.contains("speed 4x"));
        assert!(text.contains("agents 1"));
        assert!(text.contains("spawned 3"));
        assert!(text.contains("space pause"));
        assert!(text.contains("\x1b[38;2;"));
        // Exactly `height` rows: one fewer line break than rows, no trailing one.
        assert_eq!(text.matches("\r\n").count(), 11);
        assert!(!text.ends_with("\r\n"));
    }

    #[test]
    fn the_inspector_reports_the_same_fields_as_the_bevy_viewer() {
        let mut backend = backend();
        backend.draw(&frame(Some(0))).expect("draw succeeds");
        let footer = backend.footer_line().to_owned();
        assert!(footer.starts_with("Agent #0"));
        assert!(footer.contains("position ("));
        assert!(footer.contains("heading"));
        assert!(footer.contains("speed 12.00 m/s"));
        assert!(footer.contains("path guide"));
        assert!(footer.contains("distance"));
        assert!(footer.contains("body 4.50 x 1.80 m"));
        assert!(footer.contains("decision none (movement is not signal-controlled)"));
    }

    #[test]
    fn a_lower_depth_backend_reports_and_emits_no_truecolor() {
        let mut backend = CellBackend::new(Vec::new(), ColorDepth::Ansi16, scenario());
        backend.resize(20, 6).expect("resize succeeds");
        assert!(!backend.capabilities().truecolor);
        backend.draw(&frame(None)).expect("draw succeeds");
        backend.present().expect("present succeeds");
        let text = String::from_utf8(backend.into_writer()).expect("UTF-8");
        assert!(!text.contains("38;2;"));
        assert!(!text.contains("38;5;"));
    }

    #[test]
    fn present_before_draw_is_a_no_op() {
        let mut backend = backend();
        backend.present().expect("present succeeds");
        assert!(backend.into_writer().is_empty());
    }
}
