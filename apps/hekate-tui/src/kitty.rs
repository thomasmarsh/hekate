//! The opt-in Kitty graphics terminal backend.
//!
//! [`KittyBackend`] places a pixel image of the shared [`SceneFrame`] on a
//! supporting terminal through the Kitty graphics protocol. It is selected only
//! by [`crate::capability`] and always follows the bounded lifecycle mandated
//! by `DEC-002-terminal-backend-strategy`:
//!
//! - transmit-only `a=t`, never transmit-and-display `a=T`, so image data is
//!   never replaced by a same-id re-display;
//! - a stable placement `a=p,i=<id>,p=<placement>` with `C=1`, so the placement
//!   replaces rather than stacks and cursor advance cannot push it into
//!   scrollback;
//! - an explicit `a=d,d=i,i=<id>` whenever the image is dropped or the raw
//!   transmit budget is reset, and again on [`KittyBackend::shutdown`];
//! - a bounded raw transmit budget, so at most one image is ever live and the
//!   backend never buffers frames without limit.
//!
//! The backend renders the same geometry and colors as
//! [`crate::backend::CellBackend`] through [`crate::pixel::PixelRasterizer`],
//! and shares the HUD text through [`crate::hud::Hud`].

use std::io::{self, Write};
use std::sync::Arc;

use hekate_model::CompiledScenario;
use hekate_present::{BackendCapabilities, BackendResult, RendererBackend, SceneFrame};

use crate::backend::{FOOTER_COLOR, FOOTER_SELECTED_COLOR, HUD_COLOR, RunInfo, write_row};
use crate::hud::Hud;
use crate::palette::ColorDepth;
use crate::pixel::{PixelRasterizer, RgbaImage};

/// Maximum base64 payload bytes per escape sequence.
pub const MAX_CHUNK: usize = 4096;
/// Raw frame bytes transmitted before the live image is deleted and the
/// accounting restarts, bounding how much image data can be outstanding.
pub const MAX_RAW_BYTES: u64 = 32 * 1024 * 1024;
/// GNU screen's DCS passthrough string limit, conservatively; a graphics
/// sequence larger than this causes the backend to decline rather than emit a
/// string the multiplexer would truncate.
pub const SCREEN_MAX_SEQUENCE: usize = 512;
/// Status lines plus footer, matching the character backend.
const RESERVED_ROWS: u32 = 3;
/// Transmit-only action. The `T` form is deliberately never emitted.
const TRANSMIT_ACTION: &str = "a=t";

/// The RFC 4648 base64 alphabet.
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, implemented locally so the backend adds no
/// dependency and its framing is fully testable.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Build one APC graphics command: `ESC _ G <control> ; <payload> ESC \`.
///
/// An empty payload omits the `;` separator, as the protocol requires.
pub fn apc(control: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(control.len() + payload.len() + 8);
    out.extend_from_slice(b"\x1b_G");
    out.extend_from_slice(control.as_bytes());
    if !payload.is_empty() {
        out.push(b';');
        out.extend_from_slice(payload);
    }
    out.extend_from_slice(b"\x1b\\");
    out
}

/// One encoded, ready-to-write frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    /// One escape sequence per protocol chunk.
    pub sequences: Vec<Vec<u8>>,
    /// Payload bytes before base64 (the raw image data).
    pub data_bytes: usize,
    /// Base64 text bytes carried across all chunks.
    pub base64_bytes: usize,
}

/// Encode `data` for `control`, chunked at [`MAX_CHUNK`] base64 bytes.
///
/// Only the first chunk carries the control data; later chunks carry `m` and
/// nothing else, per the protocol's chunked-transmission rules. Base64 output
/// is always a multiple of four bytes and [`MAX_CHUNK`] is too, so every
/// non-final chunk satisfies the "multiple of 4" rule.
pub fn encode(control: &str, data: &[u8]) -> EncodedFrame {
    let encoded = base64_encode(data);
    let mut sequences = Vec::new();
    if encoded.is_empty() {
        sequences.push(apc(&format!("{control},m=0"), b""));
    } else {
        let mut start = 0;
        while start < encoded.len() {
            let end = (start + MAX_CHUNK).min(encoded.len());
            let last = end == encoded.len();
            let chunk_control = if sequences.is_empty() {
                format!("{control},m={}", u8::from(!last))
            } else {
                format!("m={}", u8::from(!last))
            };
            sequences.push(apc(&chunk_control, &encoded.as_bytes()[start..end]));
            start = end;
        }
    }
    EncodedFrame {
        sequences,
        data_bytes: data.len(),
        base64_bytes: encoded.len(),
    }
}

/// Delete one image and all of its placements by id.
pub fn delete_image(id: u32) -> Vec<u8> {
    apc(&format!("a=d,d=i,i={id},q=2"), b"")
}

/// Double every ESC byte, the escaping a multiplexer's DCS passthrough expects.
pub fn double_esc(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() * 2);
    for &byte in seq {
        out.push(byte);
        if byte == 0x1b {
            out.push(0x1b);
        }
    }
    out
}

/// Wrap one escape sequence for tmux's `allow-passthrough` DCS channel.
pub fn tmux_wrap(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() * 2 + 8);
    out.extend_from_slice(b"\x1bPtmux;");
    out.extend_from_slice(&double_esc(seq));
    out.extend_from_slice(b"\x1b\\");
    out
}

/// Wrap one escape sequence for GNU screen's passthrough DCS channel.
pub fn screen_wrap(seq: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(seq.len() * 2 + 4);
    out.extend_from_slice(b"\x1bP");
    out.extend_from_slice(&double_esc(seq));
    out.extend_from_slice(b"\x1b\\");
    out
}

/// A terminal multiplexer the backend may be running under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Multiplexer {
    /// A direct terminal, or a multiplexer that needs no wrapping.
    #[default]
    None,
    /// GNU screen.
    Screen,
    /// tmux.
    Tmux,
}

impl Multiplexer {
    /// Detect the multiplexer from the standard environment variables.
    pub fn detect() -> Self {
        if std::env::var_os("TMUX").is_some() {
            Self::Tmux
        } else if std::env::var_os("STY").is_some() {
            Self::Screen
        } else {
            Self::None
        }
    }

    /// Wrap one graphics sequence for this multiplexer.
    pub fn wrap(self, seq: &[u8]) -> Vec<u8> {
        match self {
            Self::None => seq.to_vec(),
            Self::Tmux => tmux_wrap(seq),
            Self::Screen => screen_wrap(seq),
        }
    }
}

/// A Kitty graphics backend that renders through `W`.
pub struct KittyBackend<W: Write> {
    writer: W,
    mux: Multiplexer,
    columns: u32,
    rows: u32,
    scene_rows: u32,
    hud: Hud,
    pending: Option<RgbaImage>,
    image_id: u32,
    placement_id: u32,
    raw_bytes_sent: u64,
    wire_bytes_sent: u64,
    frames_transmitted: u64,
    image_live: bool,
    declined: Option<String>,
}

impl<W: Write> KittyBackend<W> {
    /// A backend writing to `writer`, detecting any multiplexer from the
    /// environment.
    pub fn new(writer: W, scenario: Arc<CompiledScenario>) -> Self {
        Self::with_mux(writer, scenario, Multiplexer::detect())
    }

    /// A backend writing to `writer` under an explicit multiplexer.
    pub fn with_mux(writer: W, scenario: Arc<CompiledScenario>, mux: Multiplexer) -> Self {
        Self {
            writer,
            mux,
            columns: 80,
            rows: 24,
            scene_rows: 21,
            hud: Hud::new(scenario),
            pending: None,
            image_id: 1,
            placement_id: 1,
            raw_bytes_sent: 0,
            wire_bytes_sent: 0,
            frames_transmitted: 0,
            image_live: false,
            declined: None,
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

    /// The multiplexer this backend wraps for.
    pub const fn multiplexer(&self) -> Multiplexer {
        self.mux
    }

    /// Terminal size the backend was last resized to.
    pub const fn terminal_size(&self) -> (u32, u32) {
        (self.columns, self.rows)
    }

    /// Scene area in cells.
    pub const fn scene_size(&self) -> (u32, u32) {
        (self.columns, self.scene_rows)
    }

    /// Pixel dimensions of the frame the backend transmits.
    pub fn image_size(&self) -> (u32, u32) {
        PixelRasterizer::new(self.columns, self.scene_rows).image_size()
    }

    /// Raw image bytes transmitted since the last image delete.
    pub const fn raw_bytes_sent(&self) -> u64 {
        self.raw_bytes_sent
    }

    /// Escape-sequence bytes written to the terminal, including base64 and
    /// framing overhead.
    pub const fn wire_bytes_sent(&self) -> u64 {
        self.wire_bytes_sent
    }

    /// Frames transmitted since the backend was created.
    pub const fn frames_transmitted(&self) -> u64 {
        self.frames_transmitted
    }

    /// The status lines last computed by [`Self::draw`].
    pub const fn status_lines(&self) -> &[String; 2] {
        self.hud.status_lines()
    }

    /// The inspector or help line last computed by [`Self::draw`].
    pub fn footer_line(&self) -> &str {
        self.hud.footer_line()
    }

    /// A shared reference to the underlying sink.
    pub const fn writer(&self) -> &W {
        &self.writer
    }

    /// Consume the backend and return its sink.
    pub fn into_writer(self) -> W {
        self.writer
    }

    /// Resize the render target, deleting the old placement first because it is
    /// anchored at the old cell rectangle.
    pub fn resize_terminal(&mut self, columns: u32, rows: u32) -> BackendResult {
        let columns = columns.max(1);
        let rows = rows.max(RESERVED_ROWS);
        if columns == self.columns && rows == self.rows {
            return Ok(());
        }
        self.delete_live_image()?;
        self.columns = columns;
        self.rows = rows;
        self.scene_rows = rows - RESERVED_ROWS;
        Ok(())
    }

    /// Delete any live image so nothing is left on the terminal.
    pub fn shutdown(&mut self) -> BackendResult {
        self.delete_live_image()
    }

    fn delete_live_image(&mut self) -> BackendResult {
        if !self.image_live {
            return Ok(());
        }
        let sequence = delete_image(self.image_id);
        let wrapped = self.mux.wrap(&sequence);
        self.wire_bytes_sent += wrapped.len() as u64;
        self.writer.write_all(&wrapped)?;
        self.writer.flush()?;
        self.raw_bytes_sent = 0;
        self.image_live = false;
        Ok(())
    }

    /// The tail of a present: the status lines and footer under the image.
    fn write_hud(&self, out: &mut Vec<u8>) -> io::Result<()> {
        let footer_color = if self.hud.selected() {
            FOOTER_SELECTED_COLOR
        } else {
            FOOTER_COLOR
        };
        let status = self.hud.status_lines();
        let first_row = self.scene_rows + 1;
        write!(out, "\x1b[{first_row};1H")?;
        write_row(
            out,
            &status[0],
            self.columns,
            HUD_COLOR,
            crate::raster::BACKGROUND,
            ColorDepth::Truecolor,
        )?;
        out.write_all(b"\r\n")?;
        write_row(
            out,
            &status[1],
            self.columns,
            HUD_COLOR,
            crate::raster::BACKGROUND,
            ColorDepth::Truecolor,
        )?;
        out.write_all(b"\r\n")?;
        write_row(
            out,
            self.hud.footer_line(),
            self.columns,
            footer_color,
            crate::raster::BACKGROUND,
            ColorDepth::Truecolor,
        )?;
        Ok(())
    }
}

impl<W: Write> RendererBackend for KittyBackend<W> {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            character_cells: false,
            truecolor: true,
            kitty_graphics: true,
            pointer_input: false,
            alternate_screen: true,
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> BackendResult {
        self.resize_terminal(width, height)
    }

    fn draw(&mut self, frame: &SceneFrame) -> BackendResult {
        let raster = PixelRasterizer::new(self.columns, self.scene_rows);
        self.pending = Some(raster.rasterize(frame));
        self.hud.update(frame);
        Ok(())
    }

    fn present(&mut self) -> BackendResult {
        if let Some(reason) = &self.declined {
            return Err(io::Error::other(reason.clone()).into());
        }
        let Some(image) = self.pending.take() else {
            return Ok(());
        };
        let frame_bytes = image.rgba().len() as u64;

        // Bound outstanding image data: if this frame would exceed the raw
        // budget, drop the live image and its accounting before transmitting.
        if self.image_live && self.raw_bytes_sent.saturating_add(frame_bytes) > MAX_RAW_BYTES {
            self.delete_live_image()?;
        }

        let control = format!(
            "{TRANSMIT_ACTION},f=32,s={width},v={height},i={id},q=2",
            width = image.width(),
            height = image.height(),
            id = self.image_id,
        );
        let encoded = encode(&control, image.rgba());
        let mut sequences = encoded.sequences;
        // Stable placement: re-placing the same id and placement replaces the
        // previous placement instead of stacking a new one.
        sequences.push(apc(
            &format!(
                "a=p,i={id},p={placement},C=1,c={columns},r={scene_rows},q=2",
                id = self.image_id,
                placement = self.placement_id,
                columns = self.columns,
                scene_rows = self.scene_rows,
            ),
            b"",
        ));

        // GNU screen's passthrough cannot carry a graphics string this large;
        // decline so the caller can fall back instead of emitting a truncated
        // image.
        if self.mux == Multiplexer::Screen
            && sequences
                .iter()
                .any(|seq| screen_wrap(seq).len() > SCREEN_MAX_SEQUENCE)
        {
            let reason = format!(
                "Kitty graphics frame needs {} bytes per string; GNU screen passthrough \
                 cannot carry it",
                encoded.base64_bytes
            );
            self.declined = Some(reason.clone());
            return Err(io::Error::other(reason).into());
        }

        let mut out = Vec::new();
        // The placement anchors at the current cursor position.
        out.extend_from_slice(b"\x1b[H");
        for sequence in &sequences {
            out.extend_from_slice(&self.mux.wrap(sequence));
        }
        self.write_hud(&mut out)?;
        out.extend_from_slice(b"\x1b[0m");
        self.writer.write_all(&out)?;
        self.writer.flush()?;

        self.wire_bytes_sent += out.len() as u64;
        self.raw_bytes_sent += frame_bytes;
        self.frames_transmitted += 1;
        self.image_live = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use glam::DVec2;
    use hekate_model::parse_scenario_source;
    use hekate_present::{
        FrameStatus, Overlays, SafetyOverlay, SceneGeometry, Speed, TacticalOverlay, Viewport,
    };
    use hekate_sim::{RunConfig, Simulation, SnapshotDetail};

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
                .map(|sample| hekate_present::SceneBody::project(&[], sample, 0.0))
                .collect(),
            overlays: Overlays::default(),
            safety: SafetyOverlay::default(),
            tactical: TacticalOverlay::default(),
        }
    }

    fn backend() -> KittyBackend<Vec<u8>> {
        let mut backend = KittyBackend::with_mux(Vec::new(), scenario(), Multiplexer::None);
        backend.resize_terminal(40, 12).expect("resize succeeds");
        backend
    }

    /// The concatenated output as a lossy string, for substring assertions.
    fn output(backend: KittyBackend<Vec<u8>>) -> String {
        String::from_utf8_lossy(&backend.into_writer()).into_owned()
    }

    #[test]
    fn base64_matches_the_rfc_4648_examples() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn apc_omits_the_separator_without_a_payload() {
        assert_eq!(apc("a=d,d=A", b""), b"\x1b_Ga=d,d=A\x1b\\");
    }

    #[test]
    fn encode_chunks_large_frames_with_control_only_on_the_first() {
        let data = vec![0xabu8; 12 * 1024];
        let frame = encode("a=t,f=32,s=64,v=48,i=9", &data);
        assert_eq!(frame.sequences.len(), 4);
        assert!(
            frame.sequences[0].starts_with(b"\x1b_Ga=t,f=32,s=64,v=48,i=9,m=1;"),
            "first chunk carries the control data"
        );
        for sequence in &frame.sequences[1..frame.sequences.len() - 1] {
            assert!(sequence.starts_with(b"\x1b_Gm=1;"));
        }
        assert!(frame.sequences.last().unwrap().starts_with(b"\x1b_Gm=0;"));
        for sequence in &frame.sequences[..frame.sequences.len() - 1] {
            let body = between(sequence, 3, b"\x1b\\").expect("APC body");
            let payload = &body[body.iter().position(|&b| b == b';').unwrap() + 1..];
            assert_eq!(payload.len() % 4, 0);
            assert!(payload.len() <= MAX_CHUNK);
        }
    }

    #[test]
    fn transmit_is_never_the_display_action() {
        let mut backend = backend();
        backend.draw(&frame(None)).expect("draw");
        backend.present().expect("present");
        let text = output(backend);
        assert!(text.contains("a=t,"));
        assert!(!text.contains("a=T,"));
    }

    #[test]
    fn a_frame_transmits_places_and_writes_the_hud() {
        let mut backend = backend();
        backend.set_run_info(RunInfo {
            seed: 4,
            spawned: 3,
            despawned: 1,
        });
        backend.draw(&frame(Some(0))).expect("draw");
        backend.present().expect("present");
        let text = output(backend);
        // Transmit and stable placement with the cursor held.
        assert!(text.contains("f=32,s="));
        assert!(text.contains("a=p,i=1,p=1,C=1"));
        // The HUD is the same text as the character backend.
        assert!(text.contains("Hekate walking"));
        assert!(text.contains("tick 25"));
        assert!(text.contains("Agent #0"));
    }

    #[test]
    fn the_raw_transmit_budget_deletes_and_restarts_the_image() {
        let mut backend = KittyBackend::with_mux(Vec::new(), scenario(), Multiplexer::None);
        backend.resize_terminal(40, 12).expect("resize");
        backend.draw(&frame(None)).expect("draw");
        backend.present().expect("present");
        // Pretend most of the budget is already spent, then transmit again.
        backend.raw_bytes_sent = MAX_RAW_BYTES;
        backend.draw(&frame(None)).expect("draw");
        backend.present().expect("present");
        let text = output(backend);
        assert!(
            text.contains("a=d,d=i,i=1"),
            "exceeding the raw budget must delete the live image"
        );
    }

    #[test]
    fn shutdown_deletes_the_live_image() {
        let mut backend = backend();
        backend.draw(&frame(None)).expect("draw");
        backend.present().expect("present");
        backend.shutdown().expect("shutdown");
        let text = output(backend);
        assert!(text.contains("a=d,d=i,i=1"));
    }

    #[test]
    fn tmux_output_is_wrapped_and_escapes_doubled() {
        let mut backend = KittyBackend::with_mux(Vec::new(), scenario(), Multiplexer::Tmux);
        backend.resize_terminal(40, 12).expect("resize");
        backend.draw(&frame(None)).expect("draw");
        backend.present().expect("present");
        let text = String::from_utf8(backend.into_writer()).expect("UTF-8");
        assert!(text.contains("\x1bPtmux;"));
    }

    #[test]
    fn screen_declines_a_frame_it_cannot_carry() {
        let mut backend = KittyBackend::with_mux(Vec::new(), scenario(), Multiplexer::Screen);
        backend.resize_terminal(40, 12).expect("resize");
        backend.draw(&frame(None)).expect("draw");
        let error = backend.present().expect_err("screen must decline");
        assert!(error.to_string().contains("screen"));
        // Once declined, the backend keeps declining rather than emitting a
        // truncated image.
        assert!(backend.present().is_err());
    }

    #[test]
    fn resize_deletes_the_old_placement() {
        let mut backend = backend();
        backend.draw(&frame(None)).expect("draw");
        backend.present().expect("present");
        backend.resize_terminal(50, 20).expect("resize");
        assert_eq!(backend.scene_size(), (50, 17));
        let text = output(backend);
        assert!(text.contains("a=d,d=i,i=1"));
    }

    #[test]
    fn capabilities_advertise_kitty_graphics() {
        let backend = backend();
        let capabilities = backend.capabilities();
        assert!(capabilities.kitty_graphics);
        assert!(!capabilities.character_cells);
        assert!(capabilities.truecolor);
    }

    /// Find `needle` in `haystack` after `start`, returning the enclosed body.
    fn between<'a>(haystack: &'a [u8], start: usize, needle: &[u8]) -> Option<&'a [u8]> {
        haystack[start..]
            .windows(needle.len())
            .position(|window| window == needle)
            .map(|offset| &haystack[start..start + offset])
    }
}
