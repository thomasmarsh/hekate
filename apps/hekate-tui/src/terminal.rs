//! Terminal mode ownership.
//!
//! Entering and restoring the terminal is a separate concern from any backend,
//! so a backend switch (for example, falling back from Kitty graphics to
//! character cells) can restore the alternate screen, cursor, and input mode
//! before the fallback draws its first frame.

use std::io::{self, Write};

use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};

/// The terminal modes a run must be able to undo on exit or fallback.
pub trait TerminalModes {
    /// Enter the alternate screen with raw input, no cursor, and autowrap
    /// disabled.
    fn enter(&mut self) -> io::Result<()>;

    /// Restore the primary screen, the visible cursor, and cooked input.
    fn restore(&mut self) -> io::Result<()>;
}

/// The process's real terminal.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealTerminal;

impl TerminalModes for RealTerminal {
    fn enter(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
        // Autowrap off, so writing the bottom-right cell cannot scroll the
        // screen.
        stdout.write_all(b"\x1b[?7l")?;
        stdout.flush()
    }

    fn restore(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(b"\x1b[?7h")?;
        execute!(stdout, LeaveAlternateScreen, cursor::Show)?;
        terminal::disable_raw_mode()
    }
}
