//! `tangle-tui`: a character-cell terminal viewer for Tangle.
//!
//! The terminal is only the presentation host. All playback state lives in the
//! shared presentation controller, and the kernel is stepped exactly as the
//! headless CLI and the Bevy viewer step it, so the terminal cannot change
//! which ticks the simulation visits.

use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::cursor;
use crossterm::event::{
    self, Event as TerminalEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use glam::DVec2;
use tangle_model::CompiledScenario;
use tangle_present::{Overlay, RestartMode, Speed, ViewCommand, load_scenario};
use tangle_tui::{CELL_ASPECT, ColorDepth, TuiSession};

/// Scenario used when no path is passed on the command line.
const DEFAULT_SCENARIO: &str = "scenarios/walking/walking_guide_v1.json5";
/// Target presentation period, so the terminal does not spin at full speed.
const FRAME_BUDGET: Duration = Duration::from_millis(16);
/// World cells a single pan key moves the viewport.
const PAN_CELLS: f64 = 4.0;
/// Zoom factor for one zoom-in key press.
const ZOOM_IN: f64 = 0.8;
/// Zoom factor for one zoom-out key press.
const ZOOM_OUT: f64 = 1.25;

/// Errors from the event loop are boxed so kernel and terminal errors share a
/// single return type.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

fn main() -> Result<(), BoxError> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_SCENARIO.to_owned()));
    let seed: u64 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    let scenario: Arc<CompiledScenario> = match load_scenario(&path) {
        Ok(scenario) => Arc::new(scenario),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let depth = ColorDepth::detect();
    let mut session = match TuiSession::new(scenario, seed, io::stdout(), depth) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("error: cannot start simulation: {error}");
            std::process::exit(1);
        }
    };

    install_panic_hook();
    enter_terminal()?;
    let result = run(&mut session);
    leave_terminal()?;
    result
}

/// Drive the session from terminal events until the user quits.
fn run(session: &mut TuiSession<Stdout>) -> Result<(), BoxError> {
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
fn handle_key(session: &mut TuiSession<Stdout>, key: KeyEvent) -> bool {
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
fn pan(session: &mut TuiSession<Stdout>, x: f64, y: f64) {
    let scale = session.viewport().scale();
    session.apply(ViewCommand::Pan(DVec2::new(
        x * PAN_CELLS * scale,
        y * PAN_CELLS * scale * CELL_ASPECT,
    )));
}

/// Enter the alternate screen with raw input and no autowrap.
fn enter_terminal() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    // Autowrap off, so writing the bottom-right cell cannot scroll the screen.
    stdout.write_all(b"\x1b[?7l")?;
    stdout.flush()
}

/// Restore the primary screen and cooked input.
fn leave_terminal() -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.write_all(b"\x1b[?7h")?;
    execute!(stdout, LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()
}

/// Restore the terminal before the default panic output, so a panic is legible.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = leave_terminal();
        default_hook(info);
    }));
}
