//! `tangle-tui`: a character-cell terminal viewer for Tangle.
//!
//! The terminal is only the presentation host. All playback state lives in the
//! shared presentation controller, and the kernel is stepped exactly as the
//! headless CLI and the Bevy viewer step it, so the terminal cannot change
//! which ticks the simulation visits.
//!
//! The `--backend` flag selects the renderer. `auto` (the default) probes the
//! terminal and only selects the opt-in Kitty backend for a positive reply;
//! `ascii` and `kitty` override detection.

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, Event as TerminalEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use glam::DVec2;
use tangle_model::CompiledScenario;
use tangle_present::{Overlay, RestartMode, Speed, ViewCommand, load_scenario};
use tangle_tui::capability::{
    self, BackendKind, BackendRequest, EnvironmentHints, TerminalResponder,
};
use tangle_tui::terminal::{RealTerminal, TerminalModes};
use tangle_tui::{
    BackendPair, CELL_ASPECT, CellBackend, ColorDepth, KittyBackend, SessionBackend, TuiSession,
};

/// Scenario used when no path is passed on the command line.
const DEFAULT_SCENARIO: &str = "scenarios/walking/walking_guide_v1.json5";
/// Target presentation period, so the terminal does not spin at full speed.
const FRAME_BUDGET: Duration = Duration::from_millis(16);
/// How long the capability probe waits for the terminal's reply.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
/// World cells a single pan key moves the viewport.
const PAN_CELLS: f64 = 4.0;
/// Zoom factor for one zoom-in key press.
const ZOOM_IN: f64 = 0.8;
/// Zoom factor for one zoom-out key press.
const ZOOM_OUT: f64 = 1.25;

/// Command-line help.
const USAGE: &str = "\
usage: tangle-tui [--backend ascii|kitty|auto] [scenario] [seed]

  --backend auto    probe the terminal and prefer Kitty graphics when it
                    answers the support probe (default)
  --backend ascii   force the character-cell backend
  --backend kitty   force the Kitty graphics backend
";

/// Errors from the event loop are boxed so kernel and terminal errors share a
/// single return type.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The concrete session the viewer runs: the character-cell backend, the opt-in
/// Kitty graphics backend, and safe failover between them.
type ViewerSession =
    TuiSession<BackendPair<CellBackend<Stdout>, KittyBackend<Stdout>, RealTerminal>>;

/// Parsed command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    scenario: PathBuf,
    seed: u64,
    backend: BackendRequest,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scenario: PathBuf::from(DEFAULT_SCENARIO),
            seed: 0,
            backend: BackendRequest::Auto,
        }
    }
}

fn main() -> Result<(), BoxError> {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        return Ok(());
    }
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("error: {error}\n{USAGE}");
            std::process::exit(2);
        }
    };

    let scenario: Arc<CompiledScenario> = match load_scenario(&options.scenario) {
        Ok(scenario) => Arc::new(scenario),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let kind = select_backend(options.backend);

    let depth = ColorDepth::detect();
    let cells = CellBackend::new(io::stdout(), depth, Arc::clone(&scenario));
    let kitty = KittyBackend::new(io::stdout(), Arc::clone(&scenario));
    let pair = BackendPair::new(cells, kitty, RealTerminal, kind);
    let mut session = match TuiSession::with_backend(scenario, options.seed, pair) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("error: cannot start simulation: {error}");
            std::process::exit(1);
        }
    };

    let mut terminal = RealTerminal;
    install_panic_hook();
    terminal.enter()?;
    let result = run(&mut session);
    // Delete any placed graphics image before leaving the alternate screen.
    let _ = session.backend_mut().shutdown();
    terminal.restore()?;
    result
}

/// Parse command-line arguments. Flags may appear before or after positional
/// arguments and `--backend` accepts either `--backend VALUE` or
/// `--backend=VALUE`.
fn parse_options(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut positionals = 0;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--backend=") {
            options.backend = parse_backend(value)?;
        } else if arg == "--backend" {
            let value = args.next().ok_or("--backend needs a value")?;
            options.backend = parse_backend(&value)?;
        } else if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option '{arg}'"));
        } else if positionals == 0 {
            options.scenario = PathBuf::from(arg);
            positionals += 1;
        } else if positionals == 1 {
            options.seed = arg.parse().map_err(|_| format!("invalid seed '{arg}'"))?;
            positionals += 1;
        } else {
            return Err(format!("unexpected argument '{arg}'"));
        }
    }
    Ok(options)
}

/// Parse one `--backend` value.
fn parse_backend(value: &str) -> Result<BackendRequest, String> {
    BackendRequest::from_flag(value)
        .ok_or_else(|| format!("unknown backend '{value}'; expected ascii, kitty, or auto"))
}

/// Resolve the requested backend against the terminal's proven capabilities.
///
/// `ascii` and `kitty` are explicit overrides and never probe. `auto` consults
/// environment hints and, when they do not already rule graphics out, the
/// terminal's own query; it selects Kitty only for the one positive verdict,
/// and treats every ambiguous or missing reply as unsupported.
fn select_backend(request: BackendRequest) -> BackendKind {
    match request {
        BackendRequest::Ascii => BackendKind::Ascii,
        BackendRequest::Kitty => BackendKind::Kitty,
        BackendRequest::Auto => {
            let hints = EnvironmentHints::capture();
            match capability::detect(&hints, &mut TerminalResponder, PROBE_TIMEOUT) {
                Ok(detection) => BackendKind::resolve(request, detection.verdict),
                Err(error) => {
                    eprintln!(
                        "note: terminal capability probe failed ({error}); using character cells"
                    );
                    BackendKind::Ascii
                }
            }
        }
    }
}

/// Drive the session from terminal events until the user quits.
fn run(session: &mut ViewerSession) -> Result<(), BoxError> {
    let (columns, rows) = terminal::size()?;
    session.resize(u32::from(columns), u32::from(rows));

    let mut last = Instant::now();
    loop {
        let elapsed = last.elapsed().as_secs_f64();
        last = Instant::now();
        session.advance_and_render(elapsed)?;

        let timeout = FRAME_BUDGET.saturating_sub(last.elapsed());
        if !event::poll(timeout)? {
            continue;
        }
        match event::read()? {
            TerminalEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if !handle_key(session, key) {
                    break;
                }
            }
            TerminalEvent::Resize(columns, rows) => {
                session.resize(u32::from(columns), u32::from(rows));
                execute!(io::stdout(), Clear(ClearType::All))?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Translate one key press into shared view commands. Returns `false` to quit.
fn handle_key(session: &mut ViewerSession, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') => return false,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return false,
        KeyCode::Char(' ') => session.apply(ViewCommand::TogglePause),
        KeyCode::Char('.') => session.apply(ViewCommand::SingleStep),
        KeyCode::Char('1') => session.apply(ViewCommand::SetSpeed(Speed::Real)),
        KeyCode::Char('2') => session.apply(ViewCommand::SetSpeed(Speed::Fast)),
        KeyCode::Char('3') => session.apply(ViewCommand::SetSpeed(Speed::Maximum)),
        KeyCode::Char('r') => session.apply(ViewCommand::Restart(RestartMode::SameSeed)),
        KeyCode::Char('n') => session.apply(ViewCommand::Restart(RestartMode::NextSeed)),
        KeyCode::Char('g') => session.apply(ViewCommand::ToggleOverlay(Overlay::Geometry)),
        KeyCode::Char('v') => session.apply(ViewCommand::ToggleOverlay(Overlay::Vectors)),
        KeyCode::Char('b') => session.apply(ViewCommand::ToggleOverlay(Overlay::Safety)),
        KeyCode::Tab => session.select_next(),
        KeyCode::Esc => session.apply(ViewCommand::ClearSelection),
        KeyCode::Char('+' | '=') => session.apply(ViewCommand::Zoom(ZOOM_IN)),
        KeyCode::Char('-' | '_') => session.apply(ViewCommand::Zoom(ZOOM_OUT)),
        KeyCode::Char('w') | KeyCode::Up => pan(session, 0.0, 1.0),
        KeyCode::Char('s') | KeyCode::Down => pan(session, 0.0, -1.0),
        KeyCode::Char('a') | KeyCode::Left => pan(session, -1.0, 0.0),
        KeyCode::Char('d') | KeyCode::Right => pan(session, 1.0, 0.0),
        _ => {}
    }
    true
}

/// Move the viewport by `PAN_CELLS` cells in each direction, in world units.
fn pan(session: &mut ViewerSession, x: f64, y: f64) {
    let scale = session.viewport().scale();
    session.apply(ViewCommand::Pan(DVec2::new(
        x * PAN_CELLS * scale,
        y * PAN_CELLS * scale * CELL_ASPECT,
    )));
}

/// Restore the terminal before the default panic output, so a panic is legible.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut terminal = RealTerminal;
        let _ = terminal.restore();
        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        parse_options(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn defaults_to_auto_backend_and_the_walking_scenario() {
        let options = parse(&[]).expect("parses");
        assert_eq!(options.backend, BackendRequest::Auto);
        assert_eq!(options.scenario, PathBuf::from(DEFAULT_SCENARIO));
        assert_eq!(options.seed, 0);
    }

    #[test]
    fn parses_backend_flag_before_and_after_positionals() {
        let options = parse(&["--backend", "ascii", "scenarios/x.json5", "9"]).expect("parses");
        assert_eq!(options.backend, BackendRequest::Ascii);
        assert_eq!(options.scenario, PathBuf::from("scenarios/x.json5"));
        assert_eq!(options.seed, 9);

        let options = parse(&["scenarios/x.json5", "--backend=kitty"]).expect("parses");
        assert_eq!(options.backend, BackendRequest::Kitty);
        assert_eq!(options.scenario, PathBuf::from("scenarios/x.json5"));
    }

    #[test]
    fn rejects_unknown_backends_and_flags() {
        assert!(parse(&["--backend", "pixels"]).is_err());
        assert!(parse(&["--backend"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
        assert!(parse(&["a", "1", "extra"]).is_err());
    }
}
